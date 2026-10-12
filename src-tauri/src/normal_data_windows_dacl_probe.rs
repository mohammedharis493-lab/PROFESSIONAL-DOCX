//! Windows DACL structural inspection for an already opened READ_CONTROL
//! directory handle. This is a read-only negative prerequisite, never a
//! protected-storage permission decision or access-control capability.
#![cfg(windows)]

use crate::persistence::PersistenceError;
use std::{ffi::c_void, fs::File, os::windows::io::AsRawHandle, ptr};

type WinHandle = *mut c_void;

#[link(name = "advapi32")]
unsafe extern "system" {
    fn GetSecurityInfo(
        handle: WinHandle,
        object_type: u32,
        information: u32,
        owner: *mut WinHandle,
        group: *mut WinHandle,
        dacl: *mut WinHandle,
        sacl: *mut WinHandle,
        descriptor: *mut WinHandle,
    ) -> u32;
    fn IsValidAcl(acl: WinHandle) -> i32;
    fn IsValidSid(sid: WinHandle) -> i32;
    fn GetAclInformation(
        acl: WinHandle,
        information: WinHandle,
        information_length: u32,
        information_class: u32,
    ) -> i32;
    fn GetAce(acl: WinHandle, index: u32, ace: *mut WinHandle) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LocalFree(memory: WinHandle) -> WinHandle;
    fn GetCurrentProcess() -> WinHandle;
    fn ReadProcessMemory(
        process: WinHandle,
        base_address: *const c_void,
        buffer: WinHandle,
        size: usize,
        bytes_read: *mut usize,
    ) -> i32;
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct AclSizeInformation {
    ace_count: u32,
    acl_bytes_in_use: u32,
    acl_bytes_free: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct AceHeader {
    _ace_type: u8,
    _ace_flags: u8,
    ace_size: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DaclBounds {
    ace_count: u32,
    bytes_in_use: u32,
}

struct SecurityDescriptor(WinHandle);

impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // DACL points into this descriptor; it must not outlive this owner.
            unsafe { LocalFree(self.0) };
        }
    }
}

fn denied(reason: &str) -> PersistenceError {
    PersistenceError::Configuration(format!(
        "Normal Data Windows root DACL structure unavailable: {reason}"
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowsDaclShape {
    pub(crate) ace_count: u32,
    pub(crate) bytes_in_use: u32,
    pub(crate) ace_bytes: u32,
    pub(crate) basic_trustee_aces: u32,
    pub(crate) unexamined_ace_types: u32,
}

/// Reject a NULL, empty, inconsistent or oversized DACL before any ACE is
/// dereferenced. This does NOT interpret trustee SIDs, effective group rights,
/// deny ordering, inheritance semantics, or current-token permissions. A
/// syntactically valid ACL may still grant Everyone full control.
fn validate_dacl_shape(
    has_dacl: bool,
    size: AclSizeInformation,
) -> Result<DaclBounds, PersistenceError> {
    const ACL_HEADER_BYTES: u32 = 8;
    const MIN_ACE_HEADER_BYTES: u32 = 4;
    const MAX_ACL_BYTES: u32 = 65535;

    if !has_dacl {
        return Err(denied("NULL DACL grants unrestricted access"));
    }
    // Empty ACLs can deny all access and are not usable for a writable store.
    if size.ace_count == 0 {
        return Err(denied("DACL contains no access-control entries"));
    }
    let total = size
        .acl_bytes_in_use
        .checked_add(size.acl_bytes_free)
        .ok_or_else(|| denied("DACL size arithmetic overflow"))?;
    if total > MAX_ACL_BYTES
        || size.acl_bytes_in_use < ACL_HEADER_BYTES
        || size.ace_count > (size.acl_bytes_in_use - ACL_HEADER_BYTES) / MIN_ACE_HEADER_BYTES
    {
        return Err(denied("DACL size or entry count is inconsistent"));
    }
    Ok(DaclBounds {
        ace_count: size.ace_count,
        bytes_in_use: size.acl_bytes_in_use,
    })
}

/// Cross-check the owned ACL header against independent Win32 size metadata.
/// ACL_REVISION (2) and ACL_REVISION_DS (4) are recognized structurally.
/// This does not determine whether any entry confers a protected right.
fn validate_acl_header(bytes: &[u8], size: &AclSizeInformation) -> Result<(), PersistenceError> {
    if bytes.len() != 8 || (bytes[0] != 2 && bytes[0] != 4) || bytes[1] != 0 {
        return Err(denied("ACL header revision or reserved byte invalid"));
    }
    let declared_size = u32::from(u16::from_le_bytes([bytes[2], bytes[3]]));
    let declared_count = u32::from(u16::from_le_bytes([bytes[4], bytes[5]]));
    if bytes[6] != 0 || bytes[7] != 0 {
        return Err(denied("ACL header reserved bytes invalid"));
    }
    let total = size
        .acl_bytes_in_use
        .checked_add(size.acl_bytes_free)
        .ok_or_else(|| denied("ACL metadata byte count overflow"))?;
    if declared_size != total || declared_count != size.ace_count {
        return Err(denied("ACL header disagrees with queried metadata"));
    }
    Ok(())
}

fn acl_header_snapshots_match(before: &[u8], after: &[u8]) -> bool {
    before.len() == 8 && after.len() == 8 && before == after
}

/// Exact bytes only: this is not a live filesystem ACL race guarantee.
fn acl_body_snapshots_match(before: &[u8], after: &[u8], expected_size: u32) -> bool {
    before.len() == expected_size as usize && before == after
}

fn acl_metadata_unchanged(before: &AclSizeInformation, after: &AclSizeInformation) -> bool {
    before.ace_count == after.ace_count
        && before.acl_bytes_in_use == after.acl_bytes_in_use
        && before.acl_bytes_free == after.acl_bytes_free
}

fn validate_ace_bounds(
    dacl_address: usize,
    bytes_in_use: u32,
    expected_offset: u32,
    ace_address: usize,
    header: AceHeader,
) -> Result<u32, PersistenceError> {
    const ACE_HEADER_BYTES: u32 = std::mem::size_of::<AceHeader>() as u32;

    if expected_offset > bytes_in_use
        || bytes_in_use - expected_offset < ACE_HEADER_BYTES
        || dacl_address
            .checked_add(expected_offset as usize)
            .ok_or_else(|| denied("DACL ACE address arithmetic overflow"))?
            != ace_address
    {
        return Err(denied("DACL ACE pointer does not match contiguous layout"));
    }
    let ace_size = u32::from(header.ace_size);
    if ace_size < ACE_HEADER_BYTES || ace_size % 4 != 0 {
        return Err(denied("DACL ACE size is invalid or unaligned"));
    }
    let next_offset = expected_offset
        .checked_add(ace_size)
        .ok_or_else(|| denied("DACL ACE size arithmetic overflow"))?;
    if next_offset > bytes_in_use {
        return Err(denied("DACL ACE extends beyond bytes in use"));
    }
    Ok(next_offset)
}

fn read_ace_header(ace: WinHandle) -> Result<AceHeader, PersistenceError> {
    let mut header = AceHeader::default();
    let mut bytes_read = 0usize;
    let expected = std::mem::size_of::<AceHeader>();
    let ok = unsafe {
        ReadProcessMemory(
            GetCurrentProcess(),
            ace as *const c_void,
            (&mut header as *mut AceHeader).cast(),
            expected,
            &mut bytes_read,
        )
    };
    if ok == 0 || bytes_read != expected {
        return Err(denied("DACL ACE header memory could not be copied safely"));
    }
    Ok(header)
}

/// Only ordinary ACCESS_ALLOWED_ACE (0) and ACCESS_DENIED_ACE (1) have
/// the fixed [header, mask, SID] layout below. Object, callback, audit and
/// other ACE types remain unexamined; none is ever treated as permission proof.
fn validate_basic_ace_bytes(bytes: &[u8]) -> Result<u32, PersistenceError> {
    const PREFIX: usize = 8;
    if bytes.len() < PREFIX + 8 || bytes.len() % 4 != 0 {
        return Err(denied("basic ACE is too short or misaligned"));
    }
    if bytes[0] != 0 && bytes[0] != 1 {
        return Err(denied("not a basic allow/deny ACE"));
    }
    // Ordinary allow/deny ACEs only use documented inheritance flag bits.
    // Audit-success/failure and reserved flags are not interpreted here.
    if bytes[1] & !0x1f != 0 {
        return Err(denied("basic access ACE contains unsupported flag bits"));
    }
    if usize::from(u16::from_le_bytes([bytes[2], bytes[3]])) != bytes.len() {
        return Err(denied("basic ACE header length differs from copied bytes"));
    }
    // SID is revision, subauthority count, 6-byte identifier authority,
    // followed by exactly count DWORD subauthorities.
    let sid = &bytes[PREFIX..];
    if sid[0] != 1 || sid[1] > 15 {
        return Err(denied("basic ACE trustee SID revision/count invalid"));
    }
    let sid_length = 8usize
        .checked_add(usize::from(sid[1]) * 4)
        .ok_or_else(|| denied("basic ACE SID size overflow"))?;
    if sid.len() != sid_length {
        return Err(denied("basic ACE trustee SID size does not match ACE"));
    }
    // SID points exclusively into validated owned memory, never into an
    // unchecked GetAce pointer. Let Windows reject structurally invalid SIDs.
    if unsafe { IsValidSid(sid.as_ptr() as WinHandle) } == 0 {
        return Err(denied("Windows rejected basic ACE trustee SID"));
    }
    Ok(u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]))
}

/// Confirm the copied ACE still has the exact header that was bounded
/// before the copy. This detects a mismatched snapshot, not all races.
fn header_matches_copied_ace(header: AceHeader, bytes: &[u8]) -> bool {
    bytes.len() == usize::from(header.ace_size)
        && bytes.len() >= 4
        && bytes[0] == header._ace_type
        && bytes[1] == header._ace_flags
        && bytes[2..4] == header.ace_size.to_le_bytes()
}

fn copy_ace_bytes(ace: WinHandle, size: usize) -> Result<Vec<u8>, PersistenceError> {
    let mut bytes = vec![0u8; size];
    let mut bytes_read = 0usize;
    let ok = unsafe {
        ReadProcessMemory(
            GetCurrentProcess(),
            ace as *const c_void,
            bytes.as_mut_ptr().cast(),
            size,
            &mut bytes_read,
        )
    };
    if ok == 0 || bytes_read != size {
        return Err(denied("DACL basic ACE memory could not be copied safely"));
    }
    Ok(bytes)
}

fn inspect_ace_layout(
    dacl: WinHandle,
    bounds: DaclBounds,
) -> Result<(u32, u32, u32), PersistenceError> {
    const ACL_HEADER_BYTES: u32 = 8;
    let dacl_address = dacl as usize;
    let mut offset = ACL_HEADER_BYTES;
    let mut basic_trustee_aces = 0u32;
    let mut unexamined_ace_types = 0u32;

    for index in 0..bounds.ace_count {
        let mut ace = ptr::null_mut();
        if unsafe { GetAce(dacl, index, &mut ace) } == 0 || ace.is_null() {
            return Err(denied("DACL ACE could not be retrieved"));
        }
        let expected_address = dacl_address
            .checked_add(offset as usize)
            .ok_or_else(|| denied("DACL ACE address arithmetic overflow"))?;
        if expected_address != ace as usize
            || offset > bounds.bytes_in_use
            || bounds.bytes_in_use - offset < std::mem::size_of::<AceHeader>() as u32
        {
            return Err(denied("DACL ACE pointer does not match contiguous layout"));
        }
        let header = read_ace_header(ace)?;
        let next_offset = validate_ace_bounds(
            dacl_address,
            bounds.bytes_in_use,
            offset,
            ace as usize,
            header,
        )?;
        // All ACE types must be readable and retain their bounded headers.
        // Unknown formats are copied, never decoded as permissions.
        let bytes = copy_ace_bytes(ace, usize::from(header.ace_size))?;
        if !header_matches_copied_ace(header, &bytes) {
            return Err(denied("DACL ACE header changed between checked reads"));
        }
        if header._ace_type == 0 || header._ace_type == 1 {
            let _observed_mask = validate_basic_ace_bytes(&bytes)?;
            basic_trustee_aces += 1;
        } else {
            unexamined_ace_types += 1;
        }
        offset = next_offset;
    }
    if offset != bounds.bytes_in_use {
        return Err(denied("DACL ACE sizes do not account for bytes in use"));
    }
    Ok((
        offset - ACL_HEADER_BYTES,
        basic_trustee_aces,
        unexamined_ace_types,
    ))
}

/// Inspect a directory handle already opened with READ_CONTROL and bound to
/// its expected volume/file identity by the caller. Never returns an approval.
pub(crate) fn observe_dacl_structure(handle: &File) -> Result<WindowsDaclShape, PersistenceError> {
    const SE_FILE_OBJECT: u32 = 1;
    const DACL_SECURITY_INFORMATION: u32 = 0x0000_0004;
    const ACL_SIZE_INFORMATION_CLASS: u32 = 2;

    let mut dacl = ptr::null_mut();
    let mut descriptor = SecurityDescriptor(ptr::null_mut());
    let status = unsafe {
        GetSecurityInfo(
            handle.as_raw_handle(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut dacl,
            ptr::null_mut(),
            &mut descriptor.0,
        )
    };
    if status != 0 || descriptor.0.is_null() {
        return Err(denied("GetSecurityInfo could not retrieve the DACL"));
    }
    if dacl.is_null() {
        return Err(denied("NULL DACL grants unrestricted access"));
    }
    if unsafe { IsValidAcl(dacl) } == 0 {
        return Err(denied("Windows reported an invalid DACL"));
    }

    let mut size = AclSizeInformation::default();
    let ok = unsafe {
        GetAclInformation(
            dacl,
            (&mut size as *mut AclSizeInformation).cast(),
            std::mem::size_of::<AclSizeInformation>() as u32,
            ACL_SIZE_INFORMATION_CLASS,
        )
    };
    if ok == 0 {
        return Err(denied("DACL entry count and bounds could not be inspected"));
    }
    let bounds = validate_dacl_shape(true, size)?;
    let acl_header = copy_ace_bytes(dacl, 8)?;
    validate_acl_header(&acl_header, &size)?;
    // The 64 KiB ACL metadata cap was checked before these owned copies.
    let acl_before = copy_ace_bytes(dacl, bounds.bytes_in_use as usize)?;
    let (ace_bytes, basic_trustee_aces, unexamined_ace_types) = inspect_ace_layout(dacl, bounds)?;
    let acl_after = copy_ace_bytes(dacl, bounds.bytes_in_use as usize)?;
    if !acl_body_snapshots_match(&acl_before, &acl_after, bounds.bytes_in_use) {
        return Err(denied("DACL bytes changed during ACE observation"));
    }
    // A second bounded copy must still match the earlier ACL header. This
    // detects snapshot differences, not every concurrent ACL mutation.
    let repeated_acl_header = copy_ace_bytes(dacl, 8)?;
    if !acl_header_snapshots_match(&acl_header, &repeated_acl_header) {
        return Err(denied("DACL header changed during observation"));
    }
    // Re-query after traversal rather than silently relying on a stale ACL
    // metadata snapshot. This is NOT a concurrent-mutation or race guarantee.
    let mut after = AclSizeInformation::default();
    let repeat_ok = unsafe {
        GetAclInformation(
            dacl,
            (&mut after as *mut AclSizeInformation).cast(),
            std::mem::size_of::<AclSizeInformation>() as u32,
            ACL_SIZE_INFORMATION_CLASS,
        )
    };
    if repeat_ok == 0 || !acl_metadata_unchanged(&size, &after) {
        return Err(denied(
            "DACL metadata changed or became unreadable during observation",
        ));
    }
    Ok(WindowsDaclShape {
        ace_count: bounds.ace_count,
        bytes_in_use: bounds.bytes_in_use,
        ace_bytes,
        basic_trustee_aces,
        unexamined_ace_types,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_empty_and_inconsistent_acl_shapes_fail_closed() {
        assert!(validate_dacl_shape(false, AclSizeInformation::default()).is_err());
        assert!(validate_dacl_shape(true, AclSizeInformation::default()).is_err());
        assert!(validate_dacl_shape(
            true,
            AclSizeInformation {
                ace_count: 3,
                acl_bytes_in_use: 8,
                acl_bytes_free: 0,
            }
        )
        .is_err());
        assert!(validate_dacl_shape(
            true,
            AclSizeInformation {
                ace_count: 1,
                acl_bytes_in_use: u32::MAX,
                acl_bytes_free: 1,
            }
        )
        .is_err());
        assert_eq!(
            validate_dacl_shape(
                true,
                AclSizeInformation {
                    ace_count: 1,
                    acl_bytes_in_use: 24,
                    acl_bytes_free: 8,
                }
            )
            .expect("structurally plausible ACL"),
            DaclBounds {
                ace_count: 1,
                bytes_in_use: 24,
            }
        );
    }

    #[test]
    fn ace_header_copy_uses_checked_process_memory_read() {
        let source = AceHeader {
            _ace_type: 2,
            _ace_flags: 3,
            ace_size: 20,
        };
        let copied = read_ace_header(&source as *const AceHeader as WinHandle)
            .expect("readable local ACE header should copy exactly");
        assert_eq!(copied._ace_type, 2);
        assert_eq!(copied._ace_flags, 3);
        assert_eq!(copied.ace_size, 20);
        assert!(read_ace_header(ptr::null_mut()).is_err());
    }

    #[test]
    fn basic_ace_trustee_sid_and_mask_structure_is_bounded() {
        let mut bytes = vec![0u8; 20];
        bytes[0] = 0; // ACCESS_ALLOWED_ACE_TYPE
        bytes[2..4].copy_from_slice(&20u16.to_le_bytes());
        bytes[4..8].copy_from_slice(&0x001f_01ffu32.to_le_bytes());
        bytes[8] = 1; // SID revision
        bytes[9] = 1; // one subauthority
        bytes[15] = 5; // NT identifier authority
        bytes[16..20].copy_from_slice(&32u32.to_le_bytes());
        assert_eq!(
            validate_basic_ace_bytes(&bytes).expect("bounded allow ACE"),
            0x001f_01ff
        );
        bytes[0] = 1;
        assert!(validate_basic_ace_bytes(&bytes).is_ok());
        bytes[1] = 0x13; // recognized inheritance bits
        assert!(validate_basic_ace_bytes(&bytes).is_ok());
        bytes[1] = 0x40; // audit-success bit
        assert!(validate_basic_ace_bytes(&bytes).is_err());
        bytes[1] = 0x80; // audit-failure bit
        assert!(validate_basic_ace_bytes(&bytes).is_err());
        bytes[1] = 0x20; // reserved bit
        assert!(validate_basic_ace_bytes(&bytes).is_err());
        bytes[1] = 0;
        bytes[0] = 5;
        assert!(validate_basic_ace_bytes(&bytes).is_err());
        bytes[0] = 0;
        bytes[9] = 2;
        assert!(validate_basic_ace_bytes(&bytes).is_err());
        bytes[9] = 1;
        bytes[8] = 2;
        assert!(validate_basic_ace_bytes(&bytes).is_err());
        bytes[8] = 1;
        bytes[2] = 16;
        assert!(validate_basic_ace_bytes(&bytes).is_err());
        assert!(validate_basic_ace_bytes(&bytes[..12]).is_err());
    }

    #[test]
    fn complete_acl_snapshot_must_be_exact_and_bounded() {
        let before = [2u8, 0, 12, 0, 1, 0, 0, 0, 0, 0, 4, 0];
        let mut after = before;
        assert!(acl_body_snapshots_match(&before, &after, 12));
        after[9] ^= 1;
        assert!(!acl_body_snapshots_match(&before, &after, 12));
        assert!(!acl_body_snapshots_match(&before, &before, 13));
        assert!(!acl_body_snapshots_match(&before[..11], &before[..11], 12));
    }

    #[test]
    fn post_inspection_acl_header_must_match_exactly() {
        let before = [2u8, 0, 32, 0, 1, 0, 0, 0];
        let mut after = before;
        assert!(acl_header_snapshots_match(&before, &after));
        after[4] = 2;
        assert!(!acl_header_snapshots_match(&before, &after));
        after = before;
        after[6] = 1;
        assert!(!acl_header_snapshots_match(&before, &after));
        assert!(!acl_header_snapshots_match(&before[..7], &before));
        assert!(!acl_header_snapshots_match(&before, &before[..7]));
    }

    #[test]
    fn post_inspection_acl_metadata_must_be_identical() {
        let initial = AclSizeInformation {
            ace_count: 2,
            acl_bytes_in_use: 48,
            acl_bytes_free: 8,
        };
        let mut next = AclSizeInformation {
            ace_count: 2,
            acl_bytes_in_use: 48,
            acl_bytes_free: 8,
        };
        assert!(acl_metadata_unchanged(&initial, &next));
        next.ace_count = 3;
        assert!(!acl_metadata_unchanged(&initial, &next));
        next.ace_count = 2;
        next.acl_bytes_in_use = 44;
        assert!(!acl_metadata_unchanged(&initial, &next));
        next.acl_bytes_in_use = 48;
        next.acl_bytes_free = 12;
        assert!(!acl_metadata_unchanged(&initial, &next));
    }

    #[test]
    fn acl_header_must_match_independent_size_metadata() {
        let metadata = AclSizeInformation {
            ace_count: 1,
            acl_bytes_in_use: 28,
            acl_bytes_free: 4,
        };
        let mut header = [2u8, 0, 32, 0, 1, 0, 0, 0];
        assert!(validate_acl_header(&header, &metadata).is_ok());
        header[0] = 4;
        assert!(validate_acl_header(&header, &metadata).is_ok());
        header[0] = 3;
        assert!(validate_acl_header(&header, &metadata).is_err());
        header[0] = 2;
        header[1] = 1;
        assert!(validate_acl_header(&header, &metadata).is_err());
        header[1] = 0;
        header[2] = 28;
        assert!(validate_acl_header(&header, &metadata).is_err());
        header[2] = 32;
        header[4] = 2;
        assert!(validate_acl_header(&header, &metadata).is_err());
        header[4] = 1;
        header[7] = 1;
        assert!(validate_acl_header(&header, &metadata).is_err());
        assert!(validate_acl_header(&header[..7], &metadata).is_err());
    }

    #[test]
    fn copied_ace_header_must_equal_checked_header() {
        let header = AceHeader {
            _ace_type: 0,
            _ace_flags: 0x10,
            ace_size: 20,
        };
        let mut bytes = vec![0u8; 20];
        bytes[..4].copy_from_slice(&[0, 0x10, 20, 0]);
        assert!(header_matches_copied_ace(header, &bytes));
        bytes[1] = 0;
        assert!(!header_matches_copied_ace(header, &bytes));
        bytes[1] = 0x10;
        bytes[2] = 16;
        assert!(!header_matches_copied_ace(header, &bytes));
        assert!(!header_matches_copied_ace(header, &bytes[..12]));
    }

    #[test]
    fn opaque_ace_copies_require_consistent_headers() {
        let header = AceHeader {
            _ace_type: 0x11,
            _ace_flags: 0,
            ace_size: 4,
        };
        assert!(header_matches_copied_ace(header, &[0x11, 0, 4, 0]));
        assert!(!header_matches_copied_ace(header, &[0x11, 0, 8, 0]));
        assert!(!header_matches_copied_ace(header, &[0x11, 0, 4]));
    }

    #[test]
    fn basic_ace_copy_refuses_unreadable_memory() {
        assert!(copy_ace_bytes(ptr::null_mut(), 20).is_err());
    }

    #[test]
    fn ace_layout_bounds_fail_closed_on_gaps_bad_sizes_and_overrun() {
        let base = 0x1000usize;
        let valid = AceHeader {
            _ace_type: 0,
            _ace_flags: 0,
            ace_size: 20,
        };
        assert_eq!(
            validate_ace_bounds(base, 48, 8, base + 8, valid).expect("first ACE"),
            28
        );
        assert_eq!(
            validate_ace_bounds(base, 48, 28, base + 28, valid).expect("second ACE"),
            48
        );
        assert!(validate_ace_bounds(base, 48, 8, base + 12, valid).is_err());
        assert!(validate_ace_bounds(
            base,
            48,
            8,
            base + 8,
            AceHeader {
                _ace_type: 0,
                _ace_flags: 0,
                ace_size: 3,
            }
        )
        .is_err());
        assert!(validate_ace_bounds(
            base,
            48,
            8,
            base + 8,
            AceHeader {
                _ace_type: 0,
                _ace_flags: 0,
                ace_size: 6,
            }
        )
        .is_err());
        assert!(validate_ace_bounds(
            base,
            16,
            8,
            base + 8,
            AceHeader {
                _ace_type: 0,
                _ace_flags: 0,
                ace_size: 12,
            }
        )
        .is_err());
        assert!(validate_ace_bounds(usize::MAX - 4, 16, 8, usize::MAX, valid).is_err());
    }
}
