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
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LocalFree(memory: WinHandle) -> WinHandle;
}

#[repr(C)]
#[derive(Default)]
struct AclSizeInformation {
    ace_count: u32,
    acl_bytes_in_use: u32,
    acl_bytes_free: u32,
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
}

/// Reject a NULL, empty, inconsistent or oversized DACL. This does NOT
/// interpret any ACE/SID, effective group rights, denies, inheritance, or
/// current-token permissions. A syntactically valid ACL may still grant
/// Everyone full control.
fn validate_dacl_shape(
    has_dacl: bool,
    size: AclSizeInformation,
) -> Result<WindowsDaclShape, PersistenceError> {
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
    Ok(WindowsDaclShape {
        ace_count: size.ace_count,
        bytes_in_use: size.acl_bytes_in_use,
    })
}

/// Inspect a directory handle already opened with READ_CONTROL and bound to
/// its expected volume/file identity by the caller. Never returns an approval.
pub(crate) fn observe_dacl_structure(
    handle: &File,
) -> Result<WindowsDaclShape, PersistenceError> {
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
    validate_dacl_shape(true, size)
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
        ).is_err());
        assert!(validate_dacl_shape(
            true,
            AclSizeInformation {
                ace_count: 1,
                acl_bytes_in_use: u32::MAX,
                acl_bytes_free: 1,
            }
        ).is_err());
        assert_eq!(
            validate_dacl_shape(
                true,
                AclSizeInformation {
                    ace_count: 1,
                    acl_bytes_in_use: 24,
                    acl_bytes_free: 8,
                }
            ).expect("structurally plausible ACL"),
            WindowsDaclShape {
                ace_count: 1,
                bytes_in_use: 24,
            }
        );
    }
}
