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
#[derive(Default)]
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

fn inspect_ace_layout(dacl: WinHandle, bounds: DaclBounds) -> Result<u32, PersistenceError> {
    const ACL_HEADER_BYTES: u32 = 8;
    let dacl_address = dacl as usize;
    let mut offset = ACL_HEADER_BYTES;

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
        offset = validate_ace_bounds(
            dacl_address,
            bounds.bytes_in_use,
            offset,
            ace as usize,
            header,
        )?;
    }
    if offset != bounds.bytes_in_use {
        return Err(denied("DACL ACE sizes do not account for bytes in use"));
    }
    Ok(offset - ACL_HEADER_BYTES)
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
    let ace_bytes = inspect_ace_layout(dacl, bounds)?;
    Ok(WindowsDaclShape {
        ace_count: bounds.ace_count,
        bytes_in_use: bounds.bytes_in_use,
        ace_bytes,
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
