//! Read-only Windows owner SID inspection for a held directory handle.
//! Caller must acquire READ_CONTROL on a no-follow handle and separately bind
//! it to the intended directory identity. Not DACL enforcement or a principal.
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
    fn OpenProcessToken(process: WinHandle, access: u32, token: *mut WinHandle) -> i32;
    fn GetTokenInformation(
        token: WinHandle,
        info_class: u32,
        buffer: WinHandle,
        size: u32,
        returned: *mut u32,
    ) -> i32;
    fn EqualSid(first: WinHandle, second: WinHandle) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> WinHandle;
    fn CloseHandle(handle: WinHandle) -> i32;
    fn LocalFree(memory: WinHandle) -> WinHandle;
}

fn denied(why: &str) -> PersistenceError {
    PersistenceError::Configuration(format!(
        "Normal Data Windows directory owner unavailable: {why}"
    ))
}

struct SecurityDescriptor(WinHandle);

impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // Descriptor allocated by GetSecurityInfo; SID points inside it.
            unsafe { LocalFree(self.0) };
        }
    }
}

struct Token(WinHandle);

impl Drop for Token {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// Check that the owner SID of this READ_CONTROL directory handle equals the
/// actual *process token user* SID. No caller supplied UUID/role is trusted.
/// An owner match is only a narrow prerequisite: an owner may grant broad DACL
/// access, and hostile same-user writers or inherited permissions remain.
pub(crate) fn observe_owner_matches_process_user(handle: &File) -> Result<(), PersistenceError> {
    // SE_FILE_OBJECT = 1, OWNER_SECURITY_INFORMATION = 0x00000001.
    const SE_FILE_OBJECT: u32 = 1;
    const OWNER_SECURITY_INFORMATION: u32 = 1;
    // TOKEN_QUERY = 0x0008, TokenUser = 1.
    const TOKEN_QUERY: u32 = 0x0008;
    const TOKEN_USER: u32 = 1;
    const ERROR_INSUFFICIENT_BUFFER: i32 = 122;
    const MAX_TOKEN_USER_BYTES: usize = 64 * 1024;

    let mut owner_sid = ptr::null_mut();
    let mut descriptor = SecurityDescriptor(ptr::null_mut());
    let status = unsafe {
        GetSecurityInfo(
            handle.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &mut owner_sid,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &mut descriptor.0,
        )
    };
    if status != 0 || owner_sid.is_null() || descriptor.0.is_null() {
        return Err(denied("GetSecurityInfo did not return an owner SID"));
    }

    let mut raw_token = ptr::null_mut();
    let opened = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw_token) };
    let token = Token(raw_token);
    if opened == 0 || token.0.is_null() {
        return Err(denied("current process token could not be queried"));
    }
    let mut required_bytes = 0u32;
    let first_call = unsafe {
        GetTokenInformation(token.0, TOKEN_USER, ptr::null_mut(), 0, &mut required_bytes)
    };
    if first_call != 0
        || std::io::Error::last_os_error().raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER)
        || required_bytes as usize > MAX_TOKEN_USER_BYTES
        || (required_bytes as usize) < std::mem::size_of::<usize>()
    {
        return Err(denied("process token user size query is invalid"));
    }

    // Vec<u64> has sufficient alignment for TOKEN_USER and SID_AND_ATTRIBUTES.
    // The first field of TOKEN_USER is its user's SID pointer.
    let allocated_words = (required_bytes as usize).div_ceil(8);
    let mut token_buffer = vec![0u64; allocated_words];
    let succeeded = unsafe {
        GetTokenInformation(
            token.0,
            TOKEN_USER,
            token_buffer.as_mut_ptr().cast(),
            required_bytes,
            &mut required_bytes,
        )
    };
    if succeeded == 0 {
        return Err(denied("process token user query failed"));
    }
    let user_sid = unsafe { *(token_buffer.as_ptr() as *const WinHandle) };
    if user_sid.is_null() {
        return Err(denied("process token has no user SID"));
    }
    if unsafe { EqualSid(owner_sid, user_sid) } == 0 {
        return Err(denied(
            "directory owner is not the current process token user",
        ));
    }
    Ok(())
}
