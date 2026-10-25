//! Shared Win32 string/SID helpers for the Windows backend (todo 26).

use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;

use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Authorization::{ConvertSidToStringSidW, ConvertStringSidToSidW};

pub(crate) fn wide(text: &str) -> Vec<u16> {
    OsStr::new(text).encode_wide().chain([0]).collect()
}

/// Parse an SDDL SID string into an owned LocalAlloc'd PSID; free with
/// [`free_sid`].
pub(crate) fn sid_from_sddl(sddl: &str) -> io::Result<*mut core::ffi::c_void> {
    let ws = wide(sddl);
    let mut sid: *mut core::ffi::c_void = core::ptr::null_mut();
    // SAFETY: ws is NUL-terminated.
    if unsafe { ConvertStringSidToSidW(ws.as_ptr(), &mut sid) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(sid)
}

/// Free a SID returned by [`sid_from_sddl`].
pub(crate) fn free_sid(sid: *mut core::ffi::c_void) {
    // SAFETY: sid came from ConvertStringSidToSidW (LocalAlloc).
    unsafe { LocalFree(sid.cast()) };
}

/// Render a PSID as its SDDL string.
pub(crate) fn sddl_of(sid: *mut core::ffi::c_void) -> Option<String> {
    let mut out: *mut u16 = core::ptr::null_mut();
    // SAFETY: sid comes from the profile APIs; the string out-param is
    // LocalAlloc'd and freed below.
    if unsafe { ConvertSidToStringSidW(sid, &mut out) } == 0 {
        return None;
    }
    let mut count = 0usize;
    // SAFETY: out is NUL-terminated; counting the length first.
    while unsafe { *out.add(count) } != 0 {
        count += 1;
    }
    // SAFETY: count elements before the NUL.
    let text = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(out, count) });
    // SAFETY: LocalAlloc'd string.
    unsafe { LocalFree(out.cast()) };
    Some(text)
}
