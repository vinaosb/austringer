//! LowBox (AppContainer) spawn for the Windows backend (todo 26).

use std::io;
use std::path::Path;

use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Security::SECURITY_CAPABILITIES;
use windows_sys::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT,
    GetExitCodeProcess, InitializeProcThreadAttributeList,
    PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, PROCESS_INFORMATION, STARTUPINFOEXW,
    UpdateProcThreadAttribute, WaitForSingleObject,
};

use crate::backends::win_util::{free_sid, sid_from_sddl, wide};

const DETACHED_PROCESS: u32 = 0x0000_0008;
const WAIT_MS: u32 = 120_000;

/// A freshly spawned AppContainer process (raw handles; caller owns).
pub(crate) struct AcSpawned {
    /// Process handle.
    pub(crate) process: windows_sys::Win32::Foundation::HANDLE,
    /// Initial thread handle.
    pub(crate) thread: windows_sys::Win32::Foundation::HANDLE,
}

// SAFETY: Win32 process/thread handles are valid across threads; this
// struct owns them exclusively and no shared alias exists.
unsafe impl Send for AcSpawned {}

impl AcSpawned {
    /// Block until exit; returns the exit code.
    pub(crate) fn wait_exit(&self) -> io::Result<i32> {
        // SAFETY: handles are owned and valid until closed.
        let wait = unsafe { WaitForSingleObject(self.process, WAIT_MS) };
        if wait != 0 {
            return Err(io::Error::other("sandboxed child did not exit in time"));
        }
        let mut code: u32 = 0;
        // SAFETY: out-pointer to a local u32.
        if unsafe { GetExitCodeProcess(self.process, &mut code) } == 0 {
            return Err(io::Error::other("GetExitCodeProcess failed"));
        }
        Ok(code as i32)
    }

    /// Close both handles.
    pub(crate) fn close(&self) {
        // SAFETY: owned handles.
        unsafe {
            CloseHandle(self.thread);
            CloseHandle(self.process);
        }
    }
}

/// Spawn `program` inside the AppContainer identified by `sid_sddl`.
///
/// Two build-empirical constraints shape this signature (see the
/// task-26 evidence for the full probe matrix):
/// * UNSUSPENDED — `CREATE_SUSPENDED` + the LowBox attribute makes the
///   child never complete initialization on current Windows builds.
/// * INHERITED ENVIRONMENT — a non-NULL `lpEnvironment` together with
///   the `SECURITY_CAPABILITIES` attribute is rejected with
///   ERROR_ENVVAR_NOT_FOUND (203) on this build, so per-spawn env
///   overrides are impossible here and the backend's report names
///   `env_scrub` as a gap instead of claiming enforcement.
pub(crate) fn spawn_in_appcontainer(
    sid_sddl: &str,
    program: &str,
    args: &[String],
    cwd: &Path,
) -> io::Result<AcSpawned> {
    let sid = sid_from_sddl(sid_sddl)?;
    let mut si: STARTUPINFOEXW = unsafe { core::mem::zeroed() };
    si.StartupInfo.cb = core::mem::size_of::<STARTUPINFOEXW>() as u32;
    let mut attr_size: usize = 0;
    // SAFETY: sizing call with null list is the documented pattern.
    unsafe { InitializeProcThreadAttributeList(core::ptr::null_mut(), 1, 0, &mut attr_size) };
    let mut buffer = vec![0u8; attr_size.max(core::mem::size_of::<usize>() * 4)];
    let attr_list = buffer.as_mut_ptr().cast::<core::ffi::c_void>();
    // SAFETY: buffer sized by the sizing call above.
    if unsafe { InitializeProcThreadAttributeList(attr_list, 1, 0, &mut attr_size) } == 0 {
        free_sid(sid);
        return Err(io::Error::last_os_error());
    }
    let caps = SECURITY_CAPABILITIES {
        AppContainerSid: sid,
        Capabilities: core::ptr::null_mut(),
        CapabilityCount: 0,
        Reserved: 0,
    };
    // SAFETY: caps lives across the CreateProcessW call below.
    let ok = unsafe {
        UpdateProcThreadAttribute(
            attr_list,
            0,
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
            &caps as *const _ as *const core::ffi::c_void,
            core::mem::size_of::<SECURITY_CAPABILITIES>(),
            core::ptr::null_mut(),
            core::ptr::null(),
        )
    };
    si.lpAttributeList = attr_list;
    let mut result = Err(io::Error::other("UpdateProcThreadAttribute failed"));
    if ok != 0 {
        let mut cmdline = quote_program(program);
        for arg in args {
            cmdline.push(' ');
            cmdline.push_str(arg);
        }
        let mut cmdw = wide(&cmdline);
        let wcwd = wide(&cwd.to_string_lossy());
        let mut pi: PROCESS_INFORMATION = unsafe { core::mem::zeroed() };
        // SAFETY: all strings NUL-terminated; environment inherited
        // (NULL) per the build constraint documented above.
        let created = unsafe {
            CreateProcessW(
                core::ptr::null(),
                cmdw.as_mut_ptr(),
                core::ptr::null(),
                core::ptr::null(),
                0,
                DETACHED_PROCESS | EXTENDED_STARTUPINFO_PRESENT,
                core::ptr::null(),
                wcwd.as_ptr(),
                &si.StartupInfo,
                &mut pi,
            )
        };
        result = if created == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(AcSpawned {
                process: pi.hProcess,
                thread: pi.hThread,
            })
        };
    }
    // SAFETY: attribute list initialized above.
    unsafe { DeleteProcThreadAttributeList(attr_list) };
    free_sid(sid);
    result
}

fn quote_program(program: &str) -> String {
    if program.contains(' ') {
        format!("\"{program}\"")
    } else {
        program.to_string()
    }
}
