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
    fn IsValidSecurityDescriptor(descriptor: WinHandle) -> i32;
    fn GetSecurityDescriptorLength(descriptor: WinHandle) -> u32;
    fn GetSecurityDescriptorDacl(
        descriptor: WinHandle,
        dacl_present: *mut i32,
        dacl: *mut WinHandle,
        dacl_defaulted: *mut i32,
    ) -> i32;
    fn GetSecurityDescriptorSacl(
        descriptor: WinHandle,
        sacl_present: *mut i32,
        sacl: *mut WinHandle,
        sacl_defaulted: *mut i32,
    ) -> i32;
    fn GetSecurityDescriptorOwner(
        descriptor: WinHandle,
        owner: *mut WinHandle,
        owner_defaulted: *mut i32,
    ) -> i32;
    fn GetSecurityDescriptorGroup(
        descriptor: WinHandle,
        group: *mut WinHandle,
        group_defaulted: *mut i32,
    ) -> i32;
    fn GetSecurityDescriptorControl(
        descriptor: WinHandle,
        control: *mut u16,
        revision: *mut u32,
    ) -> i32;
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
    pub(crate) dacl_protected_flag: bool,
    pub(crate) dacl_auto_inherited_flag: bool,
    pub(crate) dacl_defaulted_flag: bool,
    pub(crate) descriptor_bytes: u32,
}

/// Reject a NULL, empty, inconsistent or oversized DACL before any ACE is
/// dereferenced. This does NOT interpret trustee SIDs, effective group rights,
/// deny ordering, inheritance semantics, or current-token permissions. A
/// syntactically valid ACL may still grant Everyone full control.
/// A present-DACL bit and supported descriptor revision are structural
/// prerequisites only, not permission or inheritance decisions.
/// The DACL must reside fully inside the owned self-relative descriptor,
/// outside its fixed header. This only checks layout, never effective access.
fn validate_descriptor_dacl_extent(
    control: u16,
    descriptor_base: usize,
    descriptor_bytes: u32,
    dacl_base: usize,
    dacl_bytes: u32,
) -> Result<(), PersistenceError> {
    const SE_SELF_RELATIVE: u16 = 0x8000;
    const DESCRIPTOR_HEADER_BYTES: usize = 20;
    const MAX_DESCRIPTOR_BYTES: u32 = 262_144;

    if control & SE_SELF_RELATIVE == 0
        || descriptor_bytes < DESCRIPTOR_HEADER_BYTES as u32
        || descriptor_bytes > MAX_DESCRIPTOR_BYTES
        || dacl_bytes < 8
        || dacl_bytes > 65_535
    {
        return Err(denied("descriptor format or declared byte sizes invalid"));
    }
    let header_end = descriptor_base
        .checked_add(DESCRIPTOR_HEADER_BYTES)
        .ok_or_else(|| denied("descriptor header address overflow"))?;
    let descriptor_end = descriptor_base
        .checked_add(descriptor_bytes as usize)
        .ok_or_else(|| denied("descriptor address overflow"))?;
    let dacl_end = dacl_base
        .checked_add(dacl_bytes as usize)
        .ok_or_else(|| denied("descriptor DACL address overflow"))?;
    if dacl_base % 4 != 0 || dacl_base < header_end || dacl_end > descriptor_end {
        return Err(denied("DACL outside owned descriptor byte extent"));
    }
    Ok(())
}

/// Cross-check the self-relative header's DACL DWORD offset against the
/// separately queried DACL pointer. Only already-bounded, copied bytes
/// are inspected; this is not an effective-permission decision.
fn validate_self_relative_dacl_offset(
    header: &[u8],
    control: u16,
    revision: u32,
    descriptor_base: usize,
    descriptor_bytes: u32,
    dacl_address: usize,
) -> Result<(), PersistenceError> {
    const SECURITY_DESCRIPTOR_HEADER: usize = 20;
    if header.len() != SECURITY_DESCRIPTOR_HEADER
        || header[0] != 1
        || revision != 1
        || header[1] != 0
        || u16::from_le_bytes([header[2], header[3]]) != control
    {
        return Err(denied("self-relative descriptor header inconsistent"));
    }
    let offset = u32::from_le_bytes([header[16], header[17], header[18], header[19]]);
    if offset < SECURITY_DESCRIPTOR_HEADER as u32 || offset % 4 != 0 || offset > descriptor_bytes {
        return Err(denied("self-relative DACL offset invalid"));
    }
    let actual_address = descriptor_base
        .checked_add(offset as usize)
        .ok_or_else(|| denied("self-relative DACL offset address overflow"))?;
    if actual_address != dacl_address {
        return Err(denied(
            "self-relative DACL offset disagrees with native pointer",
        ));
    }
    Ok(())
}

/// Validate only the bounded locations of optional owner, group and SACL
/// sections in the copied self-relative descriptor header. Neither SID
/// identity nor audit permissions are interpreted here.
fn validate_optional_descriptor_offsets(
    header: &[u8],
    descriptor_bytes: u32,
) -> Result<(), PersistenceError> {
    const HEADER_BYTES: u32 = 20;
    const MAX_DESCRIPTOR_BYTES: u32 = 262_144;
    const MIN_SECTION_BYTES: u32 = 8;
    const SE_SELF_RELATIVE: u16 = 0x8000;
    const SE_SACL_PRESENT: u16 = 0x0010;

    if header.len() != HEADER_BYTES as usize
        || !(HEADER_BYTES..=MAX_DESCRIPTOR_BYTES).contains(&descriptor_bytes)
    {
        return Err(denied("optional descriptor section bounds unavailable"));
    }

    let control = u16::from_le_bytes([header[2], header[3]]);
    if header[0] != 1 || header[1] != 0 || control & SE_SELF_RELATIVE == 0 {
        return Err(denied("optional descriptor section header invalid"));
    }

    // SECURITY_DESCRIPTOR_RELATIVE stores Owner, Group and Sacl as
    // little-endian DWORD offsets at byte positions 4, 8 and 12.
    // Zero means absent. A present but NULL SACL is also allowed.
    for position in [4usize, 8, 12] {
        let offset = u32::from_le_bytes([
            header[position],
            header[position + 1],
            header[position + 2],
            header[position + 3],
        ]);
        if position == 12 && offset != 0 && control & SE_SACL_PRESENT == 0 {
            return Err(denied("SACL offset without SACL-present flag"));
        }
        if offset == 0 {
            continue;
        }
        let end = offset
            .checked_add(MIN_SECTION_BYTES)
            .ok_or_else(|| denied("optional descriptor section offset overflow"))?;
        if offset < HEADER_BYTES || offset % 4 != 0 || end > descriptor_bytes {
            return Err(denied("optional descriptor section outside descriptor"));
        }
    }
    Ok(())
}

/// Only decode SID byte boundaries in an owned descriptor snapshot.
/// A valid shape alone does not identify or authorize any principal.
fn optional_sid_byte_range(
    header: &[u8],
    descriptor: &[u8],
    field: usize,
) -> Result<Option<std::ops::Range<usize>>, PersistenceError> {
    if header.len() != 20 || (field != 4 && field != 8) {
        return Err(denied("optional SID offset field invalid"));
    }
    let offset = u32::from_le_bytes([
        header[field],
        header[field + 1],
        header[field + 2],
        header[field + 3],
    ]) as usize;
    if offset == 0 {
        return Ok(None);
    }
    let prefix_end = offset
        .checked_add(8)
        .ok_or_else(|| denied("optional SID prefix overflow"))?;
    if offset < 20 || offset % 4 != 0 || prefix_end > descriptor.len() {
        return Err(denied("optional SID prefix outside descriptor"));
    }
    let revision = descriptor[offset];
    let count = descriptor[offset + 1];
    if revision != 1 || count > 15 {
        return Err(denied("optional SID revision or count invalid"));
    }
    let end = prefix_end
        .checked_add(usize::from(count) * 4)
        .ok_or_else(|| denied("optional SID byte length overflow"))?;
    if end > descriptor.len() {
        return Err(denied("optional SID extends beyond descriptor"));
    }
    Ok(Some(offset..end))
}

fn validate_optional_sid_shapes(header: &[u8], descriptor: &[u8]) -> Result<(), PersistenceError> {
    optional_sid_byte_range(header, descriptor, 4)?;
    optional_sid_byte_range(header, descriptor, 8)?;
    Ok(())
}

/// Native Windows SID validity is checked on already bounded owned bytes;
/// it never establishes the SID's role, ownership or effective permissions.
fn validate_optional_sid_native(header: &[u8], descriptor: &[u8]) -> Result<(), PersistenceError> {
    for field in [4usize, 8] {
        if let Some(range) = optional_sid_byte_range(header, descriptor, field)? {
            let sid = &descriptor[range];
            if unsafe { IsValidSid(sid.as_ptr() as WinHandle) } == 0 {
                return Err(denied("Windows rejected optional descriptor SID"));
            }
        }
    }
    Ok(())
}

/// Inspect only SACL header bytes already inside an owned descriptor.
/// Audit rules or effective rights are never interpreted.
fn optional_sacl_byte_range(
    header: &[u8],
    descriptor: &[u8],
) -> Result<Option<std::ops::Range<usize>>, PersistenceError> {
    if header.len() != 20 {
        return Err(denied("optional SACL descriptor header truncated"));
    }
    let offset = u32::from_le_bytes([header[12], header[13], header[14], header[15]]) as usize;
    if offset == 0 {
        return Ok(None);
    }
    let control = u16::from_le_bytes([header[2], header[3]]);
    if control & 0x0010 == 0 {
        return Err(denied("SACL pointer without SACL-present control"));
    }
    let prefix_end = offset
        .checked_add(8)
        .ok_or_else(|| denied("optional SACL header offset overflow"))?;
    if offset < 20 || offset % 4 != 0 || prefix_end > descriptor.len() {
        return Err(denied("optional SACL header outside owned descriptor"));
    }
    let acl = &descriptor[offset..prefix_end];
    if (acl[0] != 2 && acl[0] != 4) || acl[1] != 0 || acl[6] != 0 || acl[7] != 0 {
        return Err(denied(
            "optional SACL header revision or reserved bytes invalid",
        ));
    }
    let size = usize::from(u16::from_le_bytes([acl[2], acl[3]]));
    let ace_count = usize::from(u16::from_le_bytes([acl[4], acl[5]]));
    let end = offset
        .checked_add(size)
        .ok_or_else(|| denied("optional SACL declared size overflow"))?;
    if size < 8 || size % 4 != 0 || end > descriptor.len() || ace_count > (size - 8) / 4 {
        return Err(denied("optional SACL declared size or ACE count invalid"));
    }
    Ok(Some(offset..end))
}

/// Walk bounded, opaque SACL ACE headers and count the actual used bytes.
/// Never decode audit masks, trustees or effective access.
fn bounded_optional_sacl_ace_usage(
    header: &[u8],
    descriptor: &[u8],
) -> Result<Option<(std::ops::Range<usize>, usize)>, PersistenceError> {
    let Some(range) = optional_sacl_byte_range(header, descriptor)? else {
        return Ok(None);
    };
    let count = usize::from(u16::from_le_bytes([
        descriptor[range.start + 4],
        descriptor[range.start + 5],
    ]));
    let mut cursor = range.start + 8;
    for _ in 0..count {
        let prefix_end = cursor
            .checked_add(4)
            .ok_or_else(|| denied("optional SACL ACE address overflow"))?;
        if prefix_end > range.end {
            return Err(denied("optional SACL ACE header truncated"));
        }
        let ace_size = usize::from(u16::from_le_bytes([
            descriptor[cursor + 2],
            descriptor[cursor + 3],
        ]));
        let next = cursor
            .checked_add(ace_size)
            .ok_or_else(|| denied("optional SACL ACE size overflow"))?;
        if ace_size < 4 || ace_size % 4 != 0 || next > range.end {
            return Err(denied("optional SACL ACE outside declared ACL"));
        }
        cursor = next;
    }
    let used = cursor - range.start;
    Ok(Some((range, used)))
}

fn validate_optional_sacl_ace_chain(
    header: &[u8],
    descriptor: &[u8],
) -> Result<(), PersistenceError> {
    bounded_optional_sacl_ace_usage(header, descriptor).map(|_| ())
}

/// Independently reconcile Win32 ACL size information with the owned ACL
/// header and the bounded ACE chain. Unused trailing ACL space is permitted.
fn validate_optional_sacl_native_metadata(
    acl: &[u8],
    used_bytes: usize,
    native: AclSizeInformation,
) -> Result<(), PersistenceError> {
    if acl.len() < 8 || used_bytes < 8 || used_bytes > acl.len() {
        return Err(denied("optional SACL used-byte bounds invalid"));
    }
    let declared_size = u32::from(u16::from_le_bytes([acl[2], acl[3]]));
    let declared_count = u32::from(u16::from_le_bytes([acl[4], acl[5]]));
    let actual_size =
        u32::try_from(acl.len()).map_err(|_| denied("optional SACL size exceeds native bounds"))?;
    let actual_used = u32::try_from(used_bytes)
        .map_err(|_| denied("optional SACL used bytes exceed native bounds"))?;
    let native_total = native
        .acl_bytes_in_use
        .checked_add(native.acl_bytes_free)
        .ok_or_else(|| denied("optional SACL native byte count overflow"))?;
    if declared_size != actual_size
        || native_total != declared_size
        || native.ace_count != declared_count
        || native.acl_bytes_in_use != actual_used
        || native.acl_bytes_free != actual_size - actual_used
    {
        return Err(denied(
            "optional SACL native metadata disagrees with owned bytes",
        ));
    }
    Ok(())
}

/// Never dereference a GetAce pointer: only compare its integer address
/// against the next known offset in the validated owned ACL buffer.
fn validate_optional_sacl_ace_pointer(
    base: usize,
    offset: usize,
    native_pointer: usize,
) -> Result<(), PersistenceError> {
    let expected = base
        .checked_add(offset)
        .ok_or_else(|| denied("optional SACL ACE pointer arithmetic overflow"))?;
    if native_pointer == 0 || native_pointer != expected {
        return Err(denied("optional SACL native ACE pointer is not contiguous"));
    }
    Ok(())
}

/// Re-enumerate ACEs with native GetAce on the same owned ACL copy.
/// Bounds and IsValidAcl/GetAclInformation must have succeeded first.
/// Audit bits, trustee identities and masks remain opaque.
fn validate_optional_sacl_native_ace_positions(
    acl: &[u8],
    used_bytes: usize,
) -> Result<(), PersistenceError> {
    if acl.len() < 8 || used_bytes < 8 || used_bytes > acl.len() {
        return Err(denied("optional SACL native ACE bounds unavailable"));
    }
    let count = u32::from(u16::from_le_bytes([acl[4], acl[5]]));
    let mut cursor = 8usize;
    for index in 0..count {
        let prefix_end = cursor
            .checked_add(4)
            .ok_or_else(|| denied("optional SACL native ACE offset overflow"))?;
        if prefix_end > used_bytes {
            return Err(denied("optional SACL native ACE header outside used bytes"));
        }
        let ace_size = usize::from(u16::from_le_bytes([acl[cursor + 2], acl[cursor + 3]]));
        let next = cursor
            .checked_add(ace_size)
            .ok_or_else(|| denied("optional SACL native ACE size overflow"))?;
        if ace_size < 4 || ace_size % 4 != 0 || next > used_bytes {
            return Err(denied("optional SACL native ACE exceeds used bytes"));
        }
        let mut native_ace = ptr::null_mut();
        if unsafe { GetAce(acl.as_ptr() as WinHandle, index, &mut native_ace) } == 0 {
            return Err(denied(
                "Windows could not enumerate owned optional SACL ACE",
            ));
        }
        validate_optional_sacl_ace_pointer(acl.as_ptr() as usize, cursor, native_ace as usize)?;
        cursor = next;
    }
    if cursor != used_bytes {
        return Err(denied("optional SACL native ACE chain byte count mismatch"));
    }
    Ok(())
}

/// Native ACL validation, metadata and ACE position inspections use only
/// an already-bounded owned optional SACL. None grants audit or access rights.
fn validate_optional_sacl_native(header: &[u8], descriptor: &[u8]) -> Result<(), PersistenceError> {
    const ACL_SIZE_INFORMATION_CLASS: u32 = 2;
    let Some((range, used_bytes)) = bounded_optional_sacl_ace_usage(header, descriptor)? else {
        return Ok(());
    };
    let acl = &descriptor[range];
    if unsafe { IsValidAcl(acl.as_ptr() as WinHandle) } == 0 {
        return Err(denied("Windows rejected owned optional SACL structure"));
    }
    let mut native = AclSizeInformation::default();
    let ok = unsafe {
        GetAclInformation(
            acl.as_ptr() as WinHandle,
            (&mut native as *mut AclSizeInformation).cast(),
            std::mem::size_of::<AclSizeInformation>() as u32,
            ACL_SIZE_INFORMATION_CLASS,
        )
    };
    if ok == 0 {
        return Err(denied(
            "Windows could not inspect owned optional SACL metadata",
        ));
    }
    validate_optional_sacl_native_metadata(acl, used_bytes, native)?;
    validate_optional_sacl_native_ace_positions(acl, used_bytes)
}

/// Check native SACL presence, defaulted bit and pointer against the
/// header of the owned self-relative security descriptor. Never inspect the
/// native pointer when SACL-present is false (those outputs are undefined).
fn validate_optional_sacl_native_descriptor_fields(
    header: &[u8],
    descriptor_base: usize,
    owned_sacl_offset: Option<usize>,
    native_present: bool,
    native_sacl_address: usize,
    native_defaulted: bool,
) -> Result<(), PersistenceError> {
    const SE_SACL_PRESENT: u16 = 0x0010;
    const SE_SACL_DEFAULTED: u16 = 0x0020;
    if header.len() != 20 {
        return Err(denied("optional SACL native descriptor header unavailable"));
    }
    let control = u16::from_le_bytes([header[2], header[3]]);
    let expected_present = control & SE_SACL_PRESENT != 0;
    if native_present != expected_present || (!expected_present && owned_sacl_offset.is_some()) {
        return Err(denied("native SACL presence differs from owned descriptor"));
    }
    if !native_present {
        // Windows does not define the other GetSecurityDescriptorSacl outputs.
        return Ok(());
    }
    if native_defaulted != (control & SE_SACL_DEFAULTED != 0) {
        return Err(denied(
            "native SACL defaulted flag differs from owned descriptor",
        ));
    }
    let expected_pointer = if let Some(offset) = owned_sacl_offset {
        if offset < 20 || offset % 4 != 0 {
            return Err(denied("optional SACL native descriptor offset invalid"));
        }
        descriptor_base
            .checked_add(offset)
            .ok_or_else(|| denied("optional SACL native descriptor address overflow"))?
    } else {
        0
    };
    if native_sacl_address != expected_pointer {
        return Err(denied(
            "native SACL pointer differs from owned descriptor offset",
        ));
    }
    Ok(())
}

/// Native GetSecurityDescriptorSacl is queried only on a bounded owned
/// self-relative descriptor snapshot. This does not read the live SACL,
/// interpret audit policy, authorize any principal or grant custody.
fn validate_optional_sacl_native_descriptor(
    header: &[u8],
    descriptor: &[u8],
) -> Result<(), PersistenceError> {
    if header.len() != 20 || descriptor.len() < 20 || !descriptor.starts_with(header) {
        return Err(denied("optional SACL owned descriptor header mismatch"));
    }
    let descriptor_bytes = u32::try_from(descriptor.len())
        .map_err(|_| denied("optional SACL owned descriptor length invalid"))?;
    validate_optional_descriptor_offsets(header, descriptor_bytes)?;
    let owned_sacl_offset = optional_sacl_byte_range(header, descriptor)?.map(|range| range.start);
    let mut native_present = 0i32;
    let mut native_sacl = ptr::null_mut();
    let mut native_defaulted = 0i32;
    let ok = unsafe {
        GetSecurityDescriptorSacl(
            descriptor.as_ptr() as WinHandle,
            &mut native_present,
            &mut native_sacl,
            &mut native_defaulted,
        )
    };
    if ok == 0 {
        return Err(denied(
            "Windows could not inspect owned descriptor SACL identity",
        ));
    }
    // In the absent case Windows leaves pointer/defaulted outputs undefined.
    let (address, defaulted) = if native_present != 0 {
        (native_sacl as usize, native_defaulted != 0)
    } else {
        (0, false)
    };
    validate_optional_sacl_native_descriptor_fields(
        header,
        descriptor.as_ptr() as usize,
        owned_sacl_offset,
        native_present != 0,
        address,
        defaulted,
    )
}

/// Compare the native owner SID pointer with an already bounded owner
/// offset in the owned self-relative descriptor. Without an owner, Windows
/// leaves OwnerDefaulted unset; that output must never influence a decision.
fn validate_optional_owner_native_fields(
    header: &[u8],
    descriptor_base: usize,
    owner_offset: Option<usize>,
    native_owner_address: usize,
    native_defaulted: Option<bool>,
) -> Result<(), PersistenceError> {
    const SE_OWNER_DEFAULTED: u16 = 0x0001;
    if header.len() != 20 {
        return Err(denied("native owner descriptor header unavailable"));
    }
    let Some(offset) = owner_offset else {
        if native_owner_address != 0 {
            return Err(denied("native owner unexpectedly present"));
        }
        return Ok(());
    };
    if offset < 20 || offset % 4 != 0 {
        return Err(denied("native owner offset invalid"));
    }
    let expected = descriptor_base
        .checked_add(offset)
        .ok_or_else(|| denied("native owner address overflow"))?;
    if native_owner_address == 0 || native_owner_address != expected {
        return Err(denied("native owner pointer differs from owned bytes"));
    }
    let control = u16::from_le_bytes([header[2], header[3]]);
    if native_defaulted != Some(control & SE_OWNER_DEFAULTED != 0) {
        return Err(denied(
            "native owner defaulted flag differs from owned bytes",
        ));
    }
    Ok(())
}

/// Query GetSecurityDescriptorOwner on the validated owned copy only.
/// SID byte bounds and IsValidSid have already been checked; no process token,
/// trustee role, owner privilege or protected-root rights are inferred.
fn validate_optional_owner_native_descriptor(
    header: &[u8],
    descriptor: &[u8],
) -> Result<(), PersistenceError> {
    if header.len() != 20 || descriptor.len() < 20 || !descriptor.starts_with(header) {
        return Err(denied("native owner descriptor snapshot inconsistent"));
    }
    let total = u32::try_from(descriptor.len())
        .map_err(|_| denied("native owner descriptor length invalid"))?;
    validate_optional_descriptor_offsets(header, total)?;
    let owner_offset = optional_sid_byte_range(header, descriptor, 4)?.map(|r| r.start);
    let mut native_owner = ptr::null_mut();
    let mut native_defaulted = 0i32;
    let ok = unsafe {
        GetSecurityDescriptorOwner(
            descriptor.as_ptr() as WinHandle,
            &mut native_owner,
            &mut native_defaulted,
        )
    };
    if ok == 0 {
        return Err(denied("Windows could not inspect owned owner SID"));
    }
    let defaulted = if native_owner.is_null() {
        None
    } else {
        Some(native_defaulted != 0)
    };
    validate_optional_owner_native_fields(
        header,
        descriptor.as_ptr() as usize,
        owner_offset,
        native_owner as usize,
        defaulted,
    )
}

/// Compare the native group SID pointer with an already bounded group
/// offset in the owned self-relative descriptor. With no group, Windows
/// does not define GroupDefaulted; it must never influence a decision.
fn validate_optional_group_native_fields(
    header: &[u8],
    descriptor_base: usize,
    group_offset: Option<usize>,
    native_group_address: usize,
    native_defaulted: Option<bool>,
) -> Result<(), PersistenceError> {
    const SE_GROUP_DEFAULTED: u16 = 0x0002;
    if header.len() != 20 {
        return Err(denied("native group descriptor header unavailable"));
    }
    let Some(offset) = group_offset else {
        if native_group_address != 0 {
            return Err(denied("native group unexpectedly present"));
        }
        return Ok(());
    };
    if offset < 20 || offset % 4 != 0 {
        return Err(denied("native group offset invalid"));
    }
    let expected = descriptor_base
        .checked_add(offset)
        .ok_or_else(|| denied("native group address overflow"))?;
    if native_group_address == 0 || native_group_address != expected {
        return Err(denied("native group pointer differs from owned bytes"));
    }
    let control = u16::from_le_bytes([header[2], header[3]]);
    if native_defaulted != Some(control & SE_GROUP_DEFAULTED != 0) {
        return Err(denied(
            "native group defaulted flag differs from owned bytes",
        ));
    }
    Ok(())
}

/// Query GetSecurityDescriptorGroup on the validated owned copy only.
/// SID byte bounds and IsValidSid have already been checked; no process token,
/// trustee role, group membership or protected-root rights are inferred.
fn validate_optional_group_native_descriptor(
    header: &[u8],
    descriptor: &[u8],
) -> Result<(), PersistenceError> {
    if header.len() != 20 || descriptor.len() < 20 || !descriptor.starts_with(header) {
        return Err(denied("native group descriptor snapshot inconsistent"));
    }
    let total = u32::try_from(descriptor.len())
        .map_err(|_| denied("native group descriptor length invalid"))?;
    validate_optional_descriptor_offsets(header, total)?;
    let group_offset = optional_sid_byte_range(header, descriptor, 8)?.map(|r| r.start);
    let mut native_group = ptr::null_mut();
    let mut native_defaulted = 0i32;
    let ok = unsafe {
        GetSecurityDescriptorGroup(
            descriptor.as_ptr() as WinHandle,
            &mut native_group,
            &mut native_defaulted,
        )
    };
    if ok == 0 {
        return Err(denied("Windows could not inspect owned group SID"));
    }
    let defaulted = if native_group.is_null() {
        None
    } else {
        Some(native_defaulted != 0)
    };
    validate_optional_group_native_fields(
        header,
        descriptor.as_ptr() as usize,
        group_offset,
        native_group as usize,
        defaulted,
    )
}

/// Decode the mandatory DACL offset entirely from a copied, bounded
/// self-relative descriptor header. A present-but-NULL DACL is never accepted.
fn owned_dacl_header_offset(
    header: &[u8],
    descriptor_bytes: usize,
) -> Result<usize, PersistenceError> {
    const HEADER_BYTES: usize = 20;
    const ACL_HEADER_BYTES: usize = 8;
    const MAX_DESCRIPTOR_BYTES: usize = 262_144;
    const SE_SELF_RELATIVE: u16 = 0x8000;
    const SE_DACL_PRESENT: u16 = 0x0004;

    if header.len() != HEADER_BYTES
        || !(HEADER_BYTES..=MAX_DESCRIPTOR_BYTES).contains(&descriptor_bytes)
        || header[0] != 1
        || header[1] != 0
    {
        return Err(denied("owned DACL descriptor header invalid"));
    }
    let control = u16::from_le_bytes([header[2], header[3]]);
    if control & SE_SELF_RELATIVE == 0 || control & SE_DACL_PRESENT == 0 {
        return Err(denied("owned descriptor lacks mandatory DACL"));
    }
    let offset = u32::from_le_bytes([header[16], header[17], header[18], header[19]]) as usize;
    let end = offset
        .checked_add(ACL_HEADER_BYTES)
        .ok_or_else(|| denied("owned DACL offset arithmetic overflow"))?;
    if offset < HEADER_BYTES || offset % 4 != 0 || end > descriptor_bytes {
        return Err(denied("owned descriptor DACL offset outside copied bytes"));
    }
    Ok(offset)
}

/// Match native DACL presence, pointer and defaulted flag to the bounded
/// owned header, and to the earlier read-only native descriptor query.
/// No effective rights, trustee or permission ordering is inferred.
fn validate_owned_dacl_native_fields(
    header: &[u8],
    descriptor_base: usize,
    descriptor_bytes: usize,
    native_present: bool,
    native_dacl_address: usize,
    native_defaulted: bool,
    original_defaulted: bool,
) -> Result<(), PersistenceError> {
    const SE_DACL_DEFAULTED: u16 = 0x0008;
    let offset = owned_dacl_header_offset(header, descriptor_bytes)?;
    if !native_present {
        // GetSecurityDescriptorDacl does not define the pointer/defaulted
        // outputs when DACL-present is false. Reject without consulting them.
        return Err(denied("native owned descriptor lacks mandatory DACL"));
    }
    let expected = descriptor_base
        .checked_add(offset)
        .ok_or_else(|| denied("owned DACL pointer arithmetic overflow"))?;
    if native_dacl_address == 0 || native_dacl_address != expected {
        return Err(denied("native owned DACL pointer differs from copied offset"));
    }
    let control = u16::from_le_bytes([header[2], header[3]]);
    if native_defaulted != (control & SE_DACL_DEFAULTED != 0)
        || native_defaulted != original_defaulted
    {
        return Err(denied("native owned DACL defaulted flag inconsistent"));
    }
    Ok(())
}

/// Requery DACL identity using only the previously bounded, copied
/// self-relative descriptor, not another live filesystem security query.
/// The original DACL was already non-NULL and independently validated.
fn validate_owned_dacl_native_descriptor(
    header: &[u8],
    descriptor: &[u8],
    original_defaulted: bool,
) -> Result<(), PersistenceError> {
    if header.len() != 20 || descriptor.len() < 20 || !descriptor.starts_with(header) {
        return Err(denied("owned DACL descriptor snapshot mismatch"));
    }
    // Validate the pointer's complete ACL header extent before native access.
    let _offset = owned_dacl_header_offset(header, descriptor.len())?;
    let mut native_present = 0i32;
    let mut native_dacl = ptr::null_mut();
    let mut native_defaulted = 0i32;
    let ok = unsafe {
        GetSecurityDescriptorDacl(
            descriptor.as_ptr() as WinHandle,
            &mut native_present,
            &mut native_dacl,
            &mut native_defaulted,
        )
    };
    if ok == 0 {
        return Err(denied("Windows could not query owned descriptor DACL identity"));
    }
    // With DACL absent, native pointer/defaulted outputs are undefined.
    let (pointer, defaulted) = if native_present != 0 {
        (native_dacl as usize, native_defaulted != 0)
    } else {
        (0, false)
    };
    validate_owned_dacl_native_fields(
        header,
        descriptor.as_ptr() as usize,
        descriptor.len(),
        native_present != 0,
        pointer,
        defaulted,
        original_defaulted,
    )
}

fn validate_descriptor_dacl_identity(
    present: bool,
    descriptor_dacl: usize,
    queried_dacl: usize,
) -> Result<(), PersistenceError> {
    if !present || descriptor_dacl == 0 || descriptor_dacl != queried_dacl {
        return Err(denied(
            "Descriptor DACL identity differs from GetSecurityInfo",
        ));
    }
    Ok(())
}

fn inspect_descriptor_control(
    control: u16,
    revision: u32,
) -> Result<(bool, bool), PersistenceError> {
    const SE_DACL_PRESENT: u16 = 0x0004;
    const SE_DACL_AUTO_INHERITED: u16 = 0x0400;
    const SE_DACL_PROTECTED: u16 = 0x1000;

    if revision != 1 || control & SE_DACL_PRESENT == 0 {
        return Err(denied(
            "Windows descriptor revision or DACL-presence flag invalid",
        ));
    }
    Ok((
        control & SE_DACL_PROTECTED != 0,
        control & SE_DACL_AUTO_INHERITED != 0,
    ))
}

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

/// Observe owned in-memory descriptor consistency, including bytes outside
/// the DACL. Equality does not establish an atomic live filesystem ACL view.
fn descriptor_snapshots_match(
    header: &[u8],
    before: &[u8],
    after: &[u8],
    expected_bytes: u32,
) -> bool {
    const HEADER_BYTES: usize = 20;
    const MAX_DESCRIPTOR_BYTES: usize = 262_144;
    let expected = expected_bytes as usize;
    (HEADER_BYTES..=MAX_DESCRIPTOR_BYTES).contains(&expected)
        && header.len() == HEADER_BYTES
        && before.len() == expected
        && after.len() == expected
        && before.starts_with(header)
        && after.starts_with(header)
        && before == after
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
    let mut descriptor_control = 0u16;
    let mut descriptor_revision = 0u32;
    if unsafe {
        GetSecurityDescriptorControl(
            descriptor.0,
            &mut descriptor_control,
            &mut descriptor_revision,
        )
    } == 0
    {
        return Err(denied("Windows descriptor control unavailable"));
    }
    let (dacl_protected_flag, dacl_auto_inherited_flag) =
        inspect_descriptor_control(descriptor_control, descriptor_revision)?;
    let mut dacl_present = 0i32;
    let mut descriptor_dacl = ptr::null_mut();
    let mut dacl_defaulted = 0i32;
    if unsafe {
        GetSecurityDescriptorDacl(
            descriptor.0,
            &mut dacl_present,
            &mut descriptor_dacl,
            &mut dacl_defaulted,
        )
    } == 0
    {
        return Err(denied("Descriptor DACL could not be inspected"));
    }
    validate_descriptor_dacl_identity(dacl_present != 0, descriptor_dacl as usize, dacl as usize)?;
    let dacl_defaulted_flag = dacl_defaulted != 0;
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
    // Length is meaningful only after Windows confirms descriptor validity.
    if unsafe { IsValidSecurityDescriptor(descriptor.0) } == 0 {
        return Err(denied("Windows rejected security descriptor structure"));
    }
    let descriptor_bytes = unsafe { GetSecurityDescriptorLength(descriptor.0) };
    let dacl_bytes = size
        .acl_bytes_in_use
        .checked_add(size.acl_bytes_free)
        .ok_or_else(|| denied("DACL total size overflow"))?;
    validate_descriptor_dacl_extent(
        descriptor_control,
        descriptor.0 as usize,
        descriptor_bytes,
        dacl as usize,
        dacl_bytes,
    )?;
    // Descriptor length and containment were checked before copying the
    // fixed header into owned bytes. Never dereference an unchecked pointer.
    let descriptor_header = copy_ace_bytes(descriptor.0, 20)?;
    validate_self_relative_dacl_offset(
        &descriptor_header,
        descriptor_control,
        descriptor_revision,
        descriptor.0 as usize,
        descriptor_bytes,
        dacl as usize,
    )?;
    validate_optional_descriptor_offsets(&descriptor_header, descriptor_bytes)?;
    // A bounded full-descriptor snapshot includes its owner/group header
    // fields and any optional native sections, without interpreting them.
    let descriptor_before = copy_ace_bytes(descriptor.0, descriptor_bytes as usize)?;
    validate_optional_sid_shapes(&descriptor_header, &descriptor_before)?;
    validate_optional_sid_native(&descriptor_header, &descriptor_before)?;
    validate_optional_owner_native_descriptor(&descriptor_header, &descriptor_before)?;
    validate_optional_group_native_descriptor(&descriptor_header, &descriptor_before)?;
    validate_owned_dacl_native_descriptor(
        &descriptor_header,
        &descriptor_before,
        dacl_defaulted_flag,
    )?;
    validate_optional_sacl_ace_chain(&descriptor_header, &descriptor_before)?;
    validate_optional_sacl_native(&descriptor_header, &descriptor_before)?;
    validate_optional_sacl_native_descriptor(&descriptor_header, &descriptor_before)?;
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
    let descriptor_after = copy_ace_bytes(descriptor.0, descriptor_bytes as usize)?;
    if !descriptor_snapshots_match(
        &descriptor_header,
        &descriptor_before,
        &descriptor_after,
        descriptor_bytes,
    ) {
        return Err(denied(
            "security descriptor bytes changed during observation",
        ));
    }
    Ok(WindowsDaclShape {
        ace_count: bounds.ace_count,
        bytes_in_use: bounds.bytes_in_use,
        ace_bytes,
        basic_trustee_aces,
        unexamined_ace_types,
        dacl_protected_flag,
        dacl_auto_inherited_flag,
        dacl_defaulted_flag,
        descriptor_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_dacl_native_fields_reject_missing_or_mismatched_identity() {
        let mut header = [0u8; 20];
        header[0] = 1;
        header[2..4].copy_from_slice(&0x8004u16.to_le_bytes());
        header[16..20].copy_from_slice(&24u32.to_le_bytes());

        assert!(validate_owned_dacl_native_fields(
            &header, 100, 48, true, 124, false, false,
        ).is_ok());
        assert!(validate_owned_dacl_native_fields(
            &header, 100, 48, false, 0, false, false,
        ).is_err());
        assert!(validate_owned_dacl_native_fields(
            &header, 100, 48, true, 0, false, false,
        ).is_err());
        assert!(validate_owned_dacl_native_fields(
            &header, 100, 48, true, 125, false, false,
        ).is_err());
        assert!(validate_owned_dacl_native_fields(
            &header, usize::MAX, 48, true, 23, false, false,
        ).is_err());
        assert!(validate_owned_dacl_native_fields(
            &header, 100, 48, true, 124, true, false,
        ).is_err());
        assert!(validate_owned_dacl_native_fields(
            &header, 100, 48, true, 124, false, true,
        ).is_err());
        header[2..4].copy_from_slice(&0x800cu16.to_le_bytes());
        assert!(validate_owned_dacl_native_fields(
            &header, 100, 48, true, 124, true, true,
        ).is_ok());
        assert!(validate_owned_dacl_native_fields(
            &header, 100, 48, true, 124, false, false,
        ).is_err());
        header[2..4].copy_from_slice(&0x8004u16.to_le_bytes());

        header[16..20].copy_from_slice(&0u32.to_le_bytes());
        assert!(validate_owned_dacl_native_fields(
            &header, 100, 48, true, 100, false, false,
        ).is_err());
        header[16..20].copy_from_slice(&22u32.to_le_bytes());
        assert!(validate_owned_dacl_native_fields(
            &header, 100, 48, true, 122, false, false,
        ).is_err());
        header[16..20].copy_from_slice(&44u32.to_le_bytes());
        assert!(validate_owned_dacl_native_fields(
            &header, 100, 48, true, 144, false, false,
        ).is_err());
        header[16..20].copy_from_slice(&24u32.to_le_bytes());
        assert!(validate_owned_dacl_native_fields(
            &header[..19], 100, 48, true, 124, false, false,
        ).is_err());
        assert!(validate_owned_dacl_native_fields(
            &header, 100, 27, true, 124, false, false,
        ).is_err());
        header[2..4].copy_from_slice(&0x8000u16.to_le_bytes());
        assert!(validate_owned_dacl_native_fields(
            &header, 100, 48, false, 0, false, false,
        ).is_err());
    }

    #[test]
    fn owned_dacl_native_descriptor_uses_only_bounded_copy() {
        let mut descriptor = [0u8; 48];
        descriptor[0] = 1;
        descriptor[2..4].copy_from_slice(&0x8004u16.to_le_bytes());
        descriptor[16..20].copy_from_slice(&24u32.to_le_bytes());
        descriptor[24..32].copy_from_slice(&[2, 0, 8, 0, 0, 0, 0, 0]);

        assert!(validate_owned_dacl_native_descriptor(
            &descriptor[..20], &descriptor, false,
        ).is_ok());
        descriptor[2..4].copy_from_slice(&0x800cu16.to_le_bytes());
        assert!(validate_owned_dacl_native_descriptor(
            &descriptor[..20], &descriptor, true,
        ).is_ok());
        assert!(validate_owned_dacl_native_descriptor(
            &descriptor[..20], &descriptor, false,
        ).is_err());
        let stale = descriptor[..20].to_vec();
        descriptor[16..20].copy_from_slice(&40u32.to_le_bytes());
        assert!(validate_owned_dacl_native_descriptor(
            &stale, &descriptor, true,
        ).is_err());
        assert!(validate_owned_dacl_native_descriptor(
            &descriptor[..20], &descriptor, true,
        ).is_err());
        assert!(validate_owned_dacl_native_descriptor(
            &descriptor[..19], &descriptor, true,
        ).is_err());
        descriptor[16..20].copy_from_slice(&0u32.to_le_bytes());
        assert!(validate_owned_dacl_native_descriptor(
            &descriptor[..20], &descriptor, true,
        ).is_err());
        descriptor[2..4].copy_from_slice(&0x8000u16.to_le_bytes());
        assert!(validate_owned_dacl_native_descriptor(
            &descriptor[..20], &descriptor, false,
        ).is_err());
    }

    #[test]
    fn optional_group_native_fields_reject_bad_pointer_and_defaulted_bit() {
        let mut header = [0u8; 20];
        header[0] = 1;
        header[2..4].copy_from_slice(&0x8000u16.to_le_bytes());
        // With no group, the defaulted output has no defined value.
        assert!(validate_optional_group_native_fields(&header, 100, None, 0, None).is_ok());
        assert!(validate_optional_group_native_fields(&header, 100, None, 0, Some(true)).is_ok());
        assert!(validate_optional_group_native_fields(&header, 100, None, 124, None).is_err());
        assert!(validate_optional_group_native_fields(&header, 100, Some(24), 124, None).is_err());
        assert!(validate_optional_group_native_fields(&header, 100, Some(24), 0, None).is_err());
        assert!(validate_optional_group_native_fields(&header, 100, Some(24), 125, None).is_err());
        assert!(validate_optional_group_native_fields(&header, 100, Some(22), 122, None).is_err());
        assert!(
            validate_optional_group_native_fields(&header, usize::MAX, Some(24), 23, None).is_err()
        );
        assert!(validate_optional_group_native_fields(&header, 100, Some(24), 124, None).is_err());
        assert!(
            validate_optional_group_native_fields(&header, 100, Some(24), 124, Some(false)).is_ok()
        );
        assert!(
            validate_optional_group_native_fields(&header, 100, Some(24), 124, Some(true)).is_err()
        );
        header[2..4].copy_from_slice(&0x8002u16.to_le_bytes());
        assert!(
            validate_optional_group_native_fields(&header, 100, Some(24), 124, Some(true)).is_ok()
        );
        assert!(
            validate_optional_group_native_fields(&header, 100, Some(24), 124, Some(false))
                .is_err()
        );
        assert!(validate_optional_group_native_fields(&header[..19], 100, None, 0, None).is_err());
    }

    #[test]
    fn optional_group_native_descriptor_uses_owned_bounded_sid() {
        let mut owned = [0u8; 48];
        owned[0] = 1;
        owned[2..4].copy_from_slice(&0x8000u16.to_le_bytes());
        assert!(validate_optional_group_native_descriptor(&owned[..20], &owned).is_ok());
        owned[8..12].copy_from_slice(&24u32.to_le_bytes());
        owned[24..36].copy_from_slice(&[1, 1, 0, 0, 0, 0, 0, 5, 18, 0, 0, 0]);
        assert!(validate_optional_group_native_descriptor(&owned[..20], &owned).is_ok());
        owned[2..4].copy_from_slice(&0x8002u16.to_le_bytes());
        assert!(validate_optional_group_native_descriptor(&owned[..20], &owned).is_ok());
        let stale = owned[..20].to_vec();
        owned[8..12].copy_from_slice(&44u32.to_le_bytes());
        assert!(validate_optional_group_native_descriptor(&stale, &owned).is_err());
        assert!(validate_optional_group_native_descriptor(&owned[..20], &owned).is_err());
        assert!(validate_optional_group_native_descriptor(&owned[..19], &owned).is_err());
    }

    #[test]
    fn optional_owner_native_fields_reject_bad_pointer_and_defaulted_bit() {
        let mut header = [0u8; 20];
        header[0] = 1;
        header[2..4].copy_from_slice(&0x8000u16.to_le_bytes());
        // With no owner, the defaulted output has no defined value.
        assert!(validate_optional_owner_native_fields(&header, 100, None, 0, None).is_ok());
        assert!(validate_optional_owner_native_fields(&header, 100, None, 0, Some(true)).is_ok());
        assert!(validate_optional_owner_native_fields(&header, 100, None, 124, None).is_err());
        assert!(validate_optional_owner_native_fields(&header, 100, Some(24), 124, None).is_err());
        assert!(validate_optional_owner_native_fields(&header, 100, Some(24), 0, None).is_err());
        assert!(validate_optional_owner_native_fields(&header, 100, Some(24), 125, None).is_err());
        assert!(validate_optional_owner_native_fields(&header, 100, Some(22), 122, None).is_err());
        assert!(
            validate_optional_owner_native_fields(&header, usize::MAX, Some(24), 23, None).is_err()
        );
        assert!(validate_optional_owner_native_fields(&header, 100, Some(24), 124, None).is_err());
        assert!(
            validate_optional_owner_native_fields(&header, 100, Some(24), 124, Some(false)).is_ok()
        );
        assert!(
            validate_optional_owner_native_fields(&header, 100, Some(24), 124, Some(true)).is_err()
        );
        header[2..4].copy_from_slice(&0x8001u16.to_le_bytes());
        assert!(
            validate_optional_owner_native_fields(&header, 100, Some(24), 124, Some(true)).is_ok()
        );
        assert!(
            validate_optional_owner_native_fields(&header, 100, Some(24), 124, Some(false))
                .is_err()
        );
        assert!(validate_optional_owner_native_fields(&header[..19], 100, None, 0, None).is_err());
    }

    #[test]
    fn optional_owner_native_descriptor_uses_owned_bounded_sid() {
        let mut owned = [0u8; 48];
        owned[0] = 1;
        owned[2..4].copy_from_slice(&0x8000u16.to_le_bytes());
        assert!(validate_optional_owner_native_descriptor(&owned[..20], &owned).is_ok());
        owned[4..8].copy_from_slice(&24u32.to_le_bytes());
        owned[24..36].copy_from_slice(&[1, 1, 0, 0, 0, 0, 0, 5, 18, 0, 0, 0]);
        assert!(validate_optional_owner_native_descriptor(&owned[..20], &owned).is_ok());
        owned[2..4].copy_from_slice(&0x8001u16.to_le_bytes());
        assert!(validate_optional_owner_native_descriptor(&owned[..20], &owned).is_ok());
        let stale = owned[..20].to_vec();
        owned[4..8].copy_from_slice(&44u32.to_le_bytes());
        assert!(validate_optional_owner_native_descriptor(&stale, &owned).is_err());
        assert!(validate_optional_owner_native_descriptor(&owned[..20], &owned).is_err());
        assert!(validate_optional_owner_native_descriptor(&owned[..19], &owned).is_err());
    }

    #[test]
    fn optional_sacl_native_descriptor_fields_fail_closed_on_mismatch() {
        let mut header = [0u8; 20];
        header[0] = 1;
        header[2..4].copy_from_slice(&0x8000u16.to_le_bytes());
        // Absent: undefined native pointer/defaulted outputs are not read.
        assert!(validate_optional_sacl_native_descriptor_fields(
            &header, 100, None, false, 999, true,
        )
        .is_ok());
        assert!(validate_optional_sacl_native_descriptor_fields(
            &header, 100, None, true, 0, false,
        )
        .is_err());
        assert!(validate_optional_sacl_native_descriptor_fields(
            &header,
            100,
            Some(24),
            false,
            0,
            false,
        )
        .is_err());
        header[2..4].copy_from_slice(&0x8010u16.to_le_bytes());
        // Present-NULL is distinct from an absent SACL.
        assert!(validate_optional_sacl_native_descriptor_fields(
            &header, 100, None, true, 0, false,
        )
        .is_ok());
        assert!(validate_optional_sacl_native_descriptor_fields(
            &header, 100, None, true, 124, false,
        )
        .is_err());
        assert!(validate_optional_sacl_native_descriptor_fields(
            &header,
            100,
            Some(24),
            true,
            124,
            false,
        )
        .is_ok());
        assert!(validate_optional_sacl_native_descriptor_fields(
            &header,
            100,
            Some(24),
            true,
            125,
            false,
        )
        .is_err());
        assert!(validate_optional_sacl_native_descriptor_fields(
            &header,
            usize::MAX,
            Some(24),
            true,
            23,
            false,
        )
        .is_err());
        assert!(validate_optional_sacl_native_descriptor_fields(
            &header,
            100,
            Some(22),
            true,
            122,
            false,
        )
        .is_err());
        assert!(validate_optional_sacl_native_descriptor_fields(
            &header,
            100,
            Some(24),
            true,
            124,
            true,
        )
        .is_err());
        header[2..4].copy_from_slice(&0x8030u16.to_le_bytes());
        assert!(validate_optional_sacl_native_descriptor_fields(
            &header,
            100,
            Some(24),
            true,
            124,
            true,
        )
        .is_ok());
        assert!(validate_optional_sacl_native_descriptor_fields(
            &header[..19],
            100,
            None,
            false,
            0,
            false,
        )
        .is_err());
    }

    #[test]
    fn optional_sacl_native_descriptor_uses_only_owned_bounded_bytes() {
        let mut owned = [0u8; 48];
        owned[0] = 1;
        owned[2..4].copy_from_slice(&0x8000u16.to_le_bytes());
        assert!(validate_optional_sacl_native_descriptor(&owned[..20], &owned).is_ok());
        owned[2..4].copy_from_slice(&0x8010u16.to_le_bytes());
        assert!(validate_optional_sacl_native_descriptor(&owned[..20], &owned).is_ok());
        owned[12..16].copy_from_slice(&24u32.to_le_bytes());
        owned[24..36].copy_from_slice(&[2, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(validate_optional_sacl_native_descriptor(&owned[..20], &owned).is_ok());
        // Native defaulted output must match the SACL_DEFAULTED control bit.
        owned[2..4].copy_from_slice(&0x8030u16.to_le_bytes());
        assert!(validate_optional_sacl_native_descriptor(&owned[..20], &owned).is_ok());
        let stale = owned[..20].to_vec();
        owned[12..16].copy_from_slice(&40u32.to_le_bytes());
        assert!(validate_optional_sacl_native_descriptor(&stale, &owned).is_err());
        assert!(validate_optional_sacl_native_descriptor(&owned[..20], &owned).is_err());
        assert!(validate_optional_sacl_native_descriptor(&owned[..19], &owned).is_err());
    }

    #[test]
    fn optional_sacl_native_ace_pointer_requires_exact_bounded_address() {
        assert!(validate_optional_sacl_ace_pointer(100, 8, 108).is_ok());
        assert!(validate_optional_sacl_ace_pointer(100, 8, 0).is_err());
        assert!(validate_optional_sacl_ace_pointer(100, 8, 107).is_err());
        assert!(validate_optional_sacl_ace_pointer(usize::MAX, 8, 7).is_err());
    }

    #[test]
    fn optional_sacl_native_ace_positions_are_bounded_and_opaque() {
        // One complete SYSTEM_AUDIT_ACE: header, opaque mask and a valid SID.
        let acl: [u8; 28] = [
            2, 0, 28, 0, 1, 0, 0, 0, // ACL
            2, 0, 20, 0, 0, 0, 0, 0, // ACE header + opaque mask
            1, 1, 0, 0, 0, 0, 0, 5, 18, 0, 0, 0, // SID
        ];
        assert!(validate_optional_sacl_native_ace_positions(&acl, 28).is_ok());
        assert!(validate_optional_sacl_native_ace_positions(&acl, 27).is_err());
        assert!(validate_optional_sacl_native_ace_positions(&acl[..24], 24).is_err());
        let mut bad = acl;
        bad[4..6].copy_from_slice(&2u16.to_le_bytes());
        assert!(validate_optional_sacl_native_ace_positions(&bad, 28).is_err());
        bad = acl;
        bad[10..12].copy_from_slice(&24u16.to_le_bytes());
        assert!(validate_optional_sacl_native_ace_positions(&bad, 28).is_err());
        bad = acl;
        bad[10..12].copy_from_slice(&0u16.to_le_bytes());
        assert!(validate_optional_sacl_native_ace_positions(&bad, 28).is_err());
        let empty_with_slack = [2u8, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert!(validate_optional_sacl_native_ace_positions(&empty_with_slack, 8).is_ok());
        assert!(validate_optional_sacl_native_ace_positions(&empty_with_slack, 12).is_err());
    }

    #[test]
    fn optional_sacl_native_metadata_requires_exact_independent_counts() {
        // One opaque four-byte ACE followed by four legal unused ACL bytes.
        let mut acl = [2u8, 0, 16, 0, 1, 0, 0, 0, 0x11, 0, 4, 0, 0, 0, 0, 0];
        let native = AclSizeInformation {
            ace_count: 1,
            acl_bytes_in_use: 12,
            acl_bytes_free: 4,
        };
        assert!(validate_optional_sacl_native_metadata(&acl, 12, native).is_ok());
        assert!(validate_optional_sacl_native_metadata(&acl, 16, native).is_err());
        assert!(validate_optional_sacl_native_metadata(&acl, 7, native).is_err());
        let mut bad = native;
        bad.ace_count = 2;
        assert!(validate_optional_sacl_native_metadata(&acl, 12, bad).is_err());
        bad = native;
        bad.acl_bytes_in_use = 11;
        assert!(validate_optional_sacl_native_metadata(&acl, 12, bad).is_err());
        bad = native;
        bad.acl_bytes_free = 3;
        assert!(validate_optional_sacl_native_metadata(&acl, 12, bad).is_err());
        bad = native;
        bad.acl_bytes_in_use = u32::MAX;
        assert!(validate_optional_sacl_native_metadata(&acl, 12, bad).is_err());
        acl[2..4].copy_from_slice(&12u16.to_le_bytes());
        assert!(validate_optional_sacl_native_metadata(&acl, 12, native).is_err());
    }

    #[test]
    fn optional_sacl_native_metadata_allows_empty_acl_slack() {
        let acl = [2u8, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let native = AclSizeInformation {
            ace_count: 0,
            acl_bytes_in_use: 8,
            acl_bytes_free: 4,
        };
        assert!(validate_optional_sacl_native_metadata(&acl, 8, native).is_ok());
        assert!(validate_optional_sacl_native_metadata(&[], 8, native).is_err());
    }

    #[test]
    fn optional_sacl_native_reenumerates_nonempty_owned_ace() {
        let mut header = [0u8; 20];
        let mut owned = [0u8; 64];
        header[2..4].copy_from_slice(&0x8014u16.to_le_bytes());
        header[12..16].copy_from_slice(&24u32.to_le_bytes());
        owned[24..52].copy_from_slice(&[
            2, 0, 28, 0, 1, 0, 0, 0, // ACL
            2, 0, 20, 0, 0, 0, 0, 0, // audit ACE header + opaque mask
            1, 1, 0, 0, 0, 0, 0, 5, 18, 0, 0, 0, // SID
        ]);
        assert!(validate_optional_sacl_native(&header, &owned).is_ok());
        owned[34..36].copy_from_slice(&24u16.to_le_bytes());
        assert!(validate_optional_sacl_native(&header, &owned).is_err());
    }

    #[test]
    fn optional_sacl_native_validation_uses_owned_bounded_bytes() {
        let mut header = [0u8; 20];
        let mut owned = vec![0u8; 64];
        assert!(validate_optional_sacl_native(&header, &owned).is_ok());
        header[2..4].copy_from_slice(&0x8014u16.to_le_bytes());
        header[12..16].copy_from_slice(&24u32.to_le_bytes());
        owned[24..32].copy_from_slice(&[2, 0, 8, 0, 0, 0, 0, 0]);
        assert!(validate_optional_sacl_native(&header, &owned).is_ok());
        // Windows must also accept a valid empty SACL with unused capacity.
        owned[26..28].copy_from_slice(&12u16.to_le_bytes());
        assert!(validate_optional_sacl_native(&header, &owned).is_ok());
        owned[26..28].copy_from_slice(&8u16.to_le_bytes());
        owned[24] = 1;
        assert!(validate_optional_sacl_native(&header, &owned).is_err());
        owned[24] = 2;
        owned[26..28].copy_from_slice(&12u16.to_le_bytes());
        owned[28..30].copy_from_slice(&1u16.to_le_bytes());
        assert!(validate_optional_sacl_native(&header, &owned).is_err());
    }

    #[test]
    fn optional_sacl_ace_chain_remains_bounded_and_opaque() {
        let mut header = [0u8; 20];
        let mut owned = [0u8; 64];
        assert!(validate_optional_sacl_ace_chain(&header, &owned).is_ok());
        header[2..4].copy_from_slice(&0x8014u16.to_le_bytes());
        header[12..16].copy_from_slice(&24u32.to_le_bytes());
        owned[24..32].copy_from_slice(&[2, 0, 20, 0, 1, 0, 0, 0]);
        owned[32..36].copy_from_slice(&[0x11, 0, 12, 0]);
        assert!(validate_optional_sacl_ace_chain(&header, &owned).is_ok());
        owned[34..36].copy_from_slice(&0u16.to_le_bytes());
        assert!(validate_optional_sacl_ace_chain(&header, &owned).is_err());
        owned[34..36].copy_from_slice(&3u16.to_le_bytes());
        assert!(validate_optional_sacl_ace_chain(&header, &owned).is_err());
        owned[34..36].copy_from_slice(&16u16.to_le_bytes());
        assert!(validate_optional_sacl_ace_chain(&header, &owned).is_err());
        owned[34..36].copy_from_slice(&12u16.to_le_bytes());
        owned[28..30].copy_from_slice(&2u16.to_le_bytes());
        assert!(validate_optional_sacl_ace_chain(&header, &owned).is_err());
    }

    #[test]
    fn optional_sacl_header_is_bounded_without_audit_permission_decisions() {
        let mut header = [0u8; 20];
        let mut owned = [0u8; 64];
        assert_eq!(optional_sacl_byte_range(&header, &owned).unwrap(), None);
        header[2..4].copy_from_slice(&0x8014u16.to_le_bytes());
        header[12..16].copy_from_slice(&24u32.to_le_bytes());
        owned[24..32].copy_from_slice(&[2, 0, 8, 0, 0, 0, 0, 0]);
        assert_eq!(
            optional_sacl_byte_range(&header, &owned).unwrap(),
            Some(24..32)
        );
        owned[24] = 1;
        assert!(optional_sacl_byte_range(&header, &owned).is_err());
        owned[24] = 2;
        owned[25] = 1;
        assert!(optional_sacl_byte_range(&header, &owned).is_err());
        owned[25] = 0;
        owned[26..28].copy_from_slice(&60u16.to_le_bytes());
        assert!(optional_sacl_byte_range(&header, &owned).is_err());
        owned[26..28].copy_from_slice(&8u16.to_le_bytes());
        owned[28..30].copy_from_slice(&1u16.to_le_bytes());
        assert!(optional_sacl_byte_range(&header, &owned).is_err());
        owned[28..30].fill(0);
        header[2..4].copy_from_slice(&0x8004u16.to_le_bytes());
        assert!(optional_sacl_byte_range(&header, &owned).is_err());
        assert!(optional_sacl_byte_range(&header[..19], &owned).is_err());
    }

    #[test]
    fn optional_sids_are_checked_by_windows_only_after_copy_and_bounds() {
        let mut header = [0u8; 20];
        let mut owned = [0u8; 64];
        assert!(validate_optional_sid_native(&header, &owned).is_ok());
        header[4..8].copy_from_slice(&20u32.to_le_bytes());
        owned[20] = 1;
        owned[21] = 1;
        assert!(validate_optional_sid_native(&header, &owned).is_ok());
        owned[20] = 2;
        assert!(validate_optional_sid_native(&header, &owned).is_err());
        owned[20] = 1;
        header[4..8].copy_from_slice(&60u32.to_le_bytes());
        assert!(validate_optional_sid_native(&header, &owned).is_err());
    }

    #[test]
    fn optional_owner_group_sids_require_bounded_binary_shapes() {
        let mut header = [0u8; 20];
        let mut owned = [0u8; 80];
        assert!(validate_optional_sid_shapes(&header, &owned).is_ok());
        header[4..8].copy_from_slice(&20u32.to_le_bytes());
        header[8..12].copy_from_slice(&40u32.to_le_bytes());
        owned[20] = 1;
        owned[21] = 1;
        owned[40] = 1;
        assert_eq!(
            optional_sid_byte_range(&header, &owned, 4).unwrap(),
            Some(20..32)
        );
        assert_eq!(
            optional_sid_byte_range(&header, &owned, 8).unwrap(),
            Some(40..48)
        );
        assert!(validate_optional_sid_shapes(&header, &owned).is_ok());

        owned[20] = 2;
        assert!(validate_optional_sid_shapes(&header, &owned).is_err());
        owned[20] = 1;
        owned[21] = 16;
        assert!(validate_optional_sid_shapes(&header, &owned).is_err());
        owned[21] = 1;
        header[8..12].copy_from_slice(&76u32.to_le_bytes());
        assert!(validate_optional_sid_shapes(&header, &owned).is_err());
        header[8..12].copy_from_slice(&40u32.to_le_bytes());
        header[4..8].copy_from_slice(&72u32.to_le_bytes());
        owned[72] = 1;
        owned[73] = 1;
        assert!(validate_optional_sid_shapes(&header, &owned).is_err());
        assert!(optional_sid_byte_range(&header[..19], &owned, 4).is_err());
        assert!(optional_sid_byte_range(&header, &owned, 12).is_err());
    }

    #[test]
    fn optional_descriptor_offsets_allow_absence_and_bounded_sections() {
        let mut header = [0u8; 20];
        header[0] = 1;
        header[2..4].copy_from_slice(&0x8004u16.to_le_bytes());
        assert!(validate_optional_descriptor_offsets(&header, 64).is_ok());

        header[4..8].copy_from_slice(&20u32.to_le_bytes());
        header[8..12].copy_from_slice(&28u32.to_le_bytes());
        assert!(validate_optional_descriptor_offsets(&header, 64).is_ok());

        header[2..4].copy_from_slice(&0x8014u16.to_le_bytes());
        header[12..16].copy_from_slice(&36u32.to_le_bytes());
        assert!(validate_optional_descriptor_offsets(&header, 64).is_ok());

        // A present-but-NULL SACL remains a valid structural possibility.
        header[12..16].fill(0);
        assert!(validate_optional_descriptor_offsets(&header, 64).is_ok());
    }

    #[test]
    fn optional_descriptor_offsets_reject_invalid_sections() {
        let mut header = [0u8; 20];
        header[0] = 1;
        header[2..4].copy_from_slice(&0x8014u16.to_le_bytes());

        for position in [4usize, 8, 12] {
            for offset in [4u32, 16, 21, 60, u32::MAX] {
                header[position..position + 4].copy_from_slice(&offset.to_le_bytes());
                assert!(validate_optional_descriptor_offsets(&header, 64).is_err());
            }
            header[position..position + 4].fill(0);
        }

        header[12..16].copy_from_slice(&20u32.to_le_bytes());
        header[2..4].copy_from_slice(&0x8004u16.to_le_bytes());
        assert!(validate_optional_descriptor_offsets(&header, 64).is_err());
        header[12..16].fill(0);
        assert!(validate_optional_descriptor_offsets(&header[..19], 64).is_err());
        assert!(validate_optional_descriptor_offsets(&header, 19).is_err());
        assert!(validate_optional_descriptor_offsets(&header, 262_145).is_err());

        header[2..4].copy_from_slice(&0x0004u16.to_le_bytes());
        assert!(validate_optional_descriptor_offsets(&header, 64).is_err());
        header[2..4].copy_from_slice(&0x8004u16.to_le_bytes());
        header[1] = 1;
        assert!(validate_optional_descriptor_offsets(&header, 64).is_err());
        header[1] = 0;
        header[0] = 2;
        assert!(validate_optional_descriptor_offsets(&header, 64).is_err());
    }

    #[test]
    fn self_relative_dacl_offset_must_match_native_pointer() {
        let mut header = [0u8; 20];
        header[0] = 1;
        header[2..4].copy_from_slice(&0x8004u16.to_le_bytes());
        header[16..20].copy_from_slice(&20u32.to_le_bytes());
        let observe =
            |bytes: &[u8]| validate_self_relative_dacl_offset(bytes, 0x8004, 1, 0x1000, 64, 0x1014);
        assert!(observe(&header).is_ok());
        assert!(observe(&header[..19]).is_err());
        assert!(
            validate_self_relative_dacl_offset(&header, 0x8004, 0, 0x1000, 64, 0x1014).is_err()
        );
        assert!(
            validate_self_relative_dacl_offset(&header, 0x8000, 1, 0x1000, 64, 0x1014).is_err()
        );
        assert!(
            validate_self_relative_dacl_offset(&header, 0x8004, 1, 0x1000, 64, 0x1018).is_err()
        );
        assert!(
            validate_self_relative_dacl_offset(&header, 0x8004, 1, usize::MAX - 8, 64, 0).is_err()
        );
        header[1] = 1;
        assert!(observe(&header).is_err());
        header[1] = 0;
        for offset in [0u32, 16, 21, 68] {
            header[16..20].copy_from_slice(&offset.to_le_bytes());
            assert!(observe(&header).is_err());
        }
        header[16..20].copy_from_slice(&20u32.to_le_bytes());
        header[0] = 2;
        assert!(observe(&header).is_err());
    }

    #[test]
    fn self_relative_descriptor_must_contain_entire_dacl() {
        let good = validate_descriptor_dacl_extent(0x8004, 0x1000, 64, 0x1014, 32);
        assert!(good.is_ok());
        assert!(validate_descriptor_dacl_extent(0x0004, 0x1000, 64, 0x1014, 32).is_err());
        assert!(validate_descriptor_dacl_extent(0x8004, 0x1000, 19, 0x1014, 8).is_err());
        assert!(validate_descriptor_dacl_extent(0x8004, 0x1000, 262_145, 0x1014, 8).is_err());
        assert!(validate_descriptor_dacl_extent(0x8004, 0x1000, 64, 0x1014, 4).is_err());
        assert!(validate_descriptor_dacl_extent(0x8004, 0x1000, 64, 0x1010, 32).is_err());
        assert!(validate_descriptor_dacl_extent(0x8004, 0x1000, 64, 0x1015, 32).is_err());
        assert!(validate_descriptor_dacl_extent(0x8004, 0x1000, 64, 0x1028, 32).is_err());
        assert!(validate_descriptor_dacl_extent(0x8004, usize::MAX - 15, 64, 0x1014, 8).is_err());
        assert!(validate_descriptor_dacl_extent(0x8004, 0x1000, 64, usize::MAX - 3, 8).is_err());
    }

    #[test]
    fn descriptor_dacl_pointer_must_match_queried_dacl() {
        assert!(validate_descriptor_dacl_identity(true, 0x1000, 0x1000).is_ok());
        assert!(validate_descriptor_dacl_identity(false, 0x1000, 0x1000).is_err());
        assert!(validate_descriptor_dacl_identity(true, 0, 0).is_err());
        assert!(validate_descriptor_dacl_identity(true, 0x1000, 0x1004).is_err());
        assert!(validate_descriptor_dacl_identity(true, 0x1000, 0).is_err());
        assert!(validate_descriptor_dacl_identity(true, 0, 0x1000).is_err());
    }

    #[test]
    fn security_descriptor_control_requires_present_dacl() {
        assert!(inspect_descriptor_control(0, 1).is_err());
        assert!(inspect_descriptor_control(0x1400, 1).is_err());
        assert!(inspect_descriptor_control(0x0004, 0).is_err());
        assert_eq!(
            inspect_descriptor_control(0x0004, 1).expect("present DACL"),
            (false, false)
        );
        assert_eq!(
            inspect_descriptor_control(0x1404, 1).expect("flags observed only"),
            (true, true)
        );
    }

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
    fn complete_descriptor_snapshot_must_be_exact_and_bounded() {
        let mut before = vec![0u8; 64];
        before[0] = 1;
        before[2..4].copy_from_slice(&0x8004u16.to_le_bytes());
        before[16..20].copy_from_slice(&20u32.to_le_bytes());
        let header = before[..20].to_vec();
        let mut after = before.clone();
        assert!(descriptor_snapshots_match(&header, &before, &after, 64));

        // A change outside the DACL header still invalidates observation.
        after[52] ^= 1;
        assert!(!descriptor_snapshots_match(&header, &before, &after, 64));
        after = before.clone();
        after[3] ^= 1;
        assert!(!descriptor_snapshots_match(&header, &before, &after, 64));

        assert!(!descriptor_snapshots_match(
            &header[..19],
            &before,
            &before,
            64
        ));
        assert!(!descriptor_snapshots_match(
            &header,
            &before[..63],
            &before,
            64
        ));
        assert!(!descriptor_snapshots_match(
            &header,
            &before,
            &before[..63],
            64
        ));
        assert!(!descriptor_snapshots_match(&header, &before, &before, 65));
        assert!(!descriptor_snapshots_match(&header, &before, &before, 19));
        assert!(!descriptor_snapshots_match(
            &header, &before, &before, 262_145
        ));
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
