//! Raw Win32 surface for the Windows sandbox backend (todo 26):
//! AppContainer profile management and DACL grant/deny merges for the
//! package SID. Spawn lives in [`crate::backends::win_spawn`].

use std::io;

use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Authorization::{
    DENY_ACCESS, EXPLICIT_ACCESS_W, GRANT_ACCESS, GetNamedSecurityInfoW, SE_FILE_OBJECT,
    SetEntriesInAclW, SetNamedSecurityInfoW, TRUSTEE_IS_SID, TRUSTEE_W,
};
use windows_sys::Win32::Security::Isolation::{
    CreateAppContainerProfile, DeleteAppContainerProfile, DeriveAppContainerSidFromAppContainerName,
};
use windows_sys::Win32::Security::{
    ACE_HEADER, ACL, ACL_REVISION, ACL_SIZE_INFORMATION, AclSizeInformation, AddAce, EqualSid,
    FreeSid, GetAce, GetAclInformation, InitializeAcl,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_GENERIC_EXECUTE, FILE_GENERIC_READ, FILE_GENERIC_WRITE, WRITE_DAC, WRITE_OWNER,
};

use crate::backends::win_util::{free_sid, sddl_of, sid_from_sddl, wide};

const DACL_SECURITY_INFORMATION: u32 = 0x0000_0004;
/// PROTECTED_DACL (0x8000_0000): strips inherited ACEs and blocks all
/// future inheritance. For protected paths this is the actual
/// enforcement — AppContainer's dual-principal check only requires a
/// package-SID grant and IGNORES deny ACEs for the package portion, so
/// removing the inherited writable-root grant is what denies access;
/// the deny ACE is kept as belt-and-suspenders.
const PROTECTED_DACL_SECURITY_INFORMATION: u32 = 0x8000_0000;
const SUB_CONTAINERS_AND_OBJECTS_INHERIT: u32 = 0x0000_0003;
const DELETE_RIGHT: u32 = 0x0001_0000;

pub(crate) const GRANT_WRITABLE: u32 =
    FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE | DELETE_RIGHT;
pub(crate) const GRANT_READABLE: u32 = FILE_GENERIC_READ | FILE_GENERIC_EXECUTE;
pub(crate) const DENY_WRITE: u32 = FILE_GENERIC_WRITE | DELETE_RIGHT | WRITE_DAC | WRITE_OWNER;

/// An AppContainer profile: name plus its SDDL string SID (Send + Sync;
/// the raw PSID never crosses threads).
pub(crate) struct AcProfile {
    /// Profile (moniker) name.
    pub(crate) name: String,
    /// SDDL form of the package SID.
    pub(crate) sid_sddl: String,
}

impl AcProfile {
    /// Create (or re-derive) the profile and return its SDDL SID.
    pub(crate) fn create(name: &str) -> io::Result<Self> {
        let wname = wide(name);
        let mut sid: *mut core::ffi::c_void = core::ptr::null_mut();
        // SAFETY: name is NUL-terminated; ppSid out-param owned below.
        let hr = unsafe {
            CreateAppContainerProfile(
                wname.as_ptr(),
                wname.as_ptr(),
                wname.as_ptr(),
                core::ptr::null(),
                0,
                &mut sid,
            )
        };
        if hr != 0 {
            // SAFETY: derive from the same NUL-terminated name.
            let hr2 =
                unsafe { DeriveAppContainerSidFromAppContainerName(wname.as_ptr(), &mut sid) };
            if hr2 != 0 {
                return Err(io::Error::other(format!(
                    "AppContainer profile unavailable (create {hr:#x}, derive {hr2:#x})"
                )));
            }
        }
        let sddl = sddl_of(sid);
        // SAFETY: profile SIDs are freed with FreeSid (see the Microsoft
        // SandboxSecurityTools sample); LocalFree would corrupt the heap.
        unsafe { FreeSid(sid.cast()) };
        match sddl {
            Some(s) => Ok(Self {
                name: name.to_string(),
                sid_sddl: s,
            }),
            None => Err(io::Error::other("AppContainer SID not renderable")),
        }
    }

    /// Best-effort profile teardown (profile dirs are reclaimable by the
    /// OS; failure is not fatal and not a sandbox escape).
    pub(crate) fn delete(&self) {
        let wname = wide(&self.name);
        // SAFETY: NUL-terminated name.
        unsafe { DeleteAppContainerProfile(wname.as_ptr()) };
    }
}

/// Grant `mask` (or deny, when `deny`) for the package SID on `path`,
/// merged into the EXISTING DACL. Replacing the DACL would drop the
/// user's own ACEs — AppContainer access checks are dual-principal, so
/// the user portion must survive. When `deny`, the DACL is also marked
/// PROTECTED (inherited grants stripped): see
/// [`PROTECTED_DACL_SECURITY_INFORMATION`] for why that is the load-
/// bearing half.
pub(crate) fn apply_ace(
    sid_sddl: &str,
    path: &std::path::Path,
    mask: u32,
    deny: bool,
) -> io::Result<()> {
    let sid = sid_from_sddl(sid_sddl)?;
    let trustee = TRUSTEE_W {
        pMultipleTrustee: core::ptr::null_mut(),
        MultipleTrusteeOperation: 0,
        TrusteeForm: TRUSTEE_IS_SID,
        TrusteeType: 0,
        ptstrName: sid.cast(),
    };
    let entries = [EXPLICIT_ACCESS_W {
        grfAccessPermissions: mask,
        grfAccessMode: if deny { DENY_ACCESS } else { GRANT_ACCESS },
        grfInheritance: SUB_CONTAINERS_AND_OBJECTS_INHERIT,
        Trustee: trustee,
    }];
    let mut wpath = wide(&path.to_string_lossy());
    let mut old_acl: *mut ACL = core::ptr::null_mut();
    let mut sec_desc: *mut core::ffi::c_void = core::ptr::null_mut();
    // SAFETY: wpath is NUL-terminated; the returned DACL points INTO
    // sec_desc, so sec_desc (not old_acl) owns the allocation and must
    // be freed with LocalFree at the end of this function.
    let rc = unsafe {
        GetNamedSecurityInfoW(
            wpath.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            &mut old_acl,
            core::ptr::null_mut(),
            &mut sec_desc,
        )
    };
    if rc != 0 {
        free_sid(sid);
        return Err(io::Error::other(format!("GetNamedSecurityInfoW rc={rc}")));
    }
    merge_and_set(&entries, old_acl, sec_desc, sid, deny, &mut wpath, path)?;
    free_sid(sid);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn merge_and_set(
    entries: &[EXPLICIT_ACCESS_W; 1],
    old_acl: *mut ACL,
    sec_desc: *mut core::ffi::c_void,
    sid: *mut core::ffi::c_void,
    deny: bool,
    wpath: &mut [u16],
    path: &std::path::Path,
) -> io::Result<()> {
    let mut new_acl: *mut ACL = core::ptr::null_mut();
    // deny path: merge into the STRIPPED DACL (no package-SID ACEs at
    // all); grant path: merge into the existing DACL as-is
    let merge_rc = if deny && !old_acl.is_null() {
        let mut stripped = strip_trustee_aces(old_acl, sid)?;
        // SAFETY: stripped outlives this call; entries is one entry.
        unsafe {
            SetEntriesInAclW(
                1,
                entries.as_ptr(),
                stripped.as_mut_ptr().cast::<ACL>(),
                &mut new_acl,
            )
        }
    } else {
        // SAFETY: entries is a valid one-entry list; OldAcl points into
        // sec_desc, which is still alive here.
        unsafe { SetEntriesInAclW(1, entries.as_ptr(), old_acl, &mut new_acl) }
    };
    if merge_rc != 0 {
        free_sid(sid);
        // SAFETY: sec_desc is LocalAlloc'd by GetNamedSecurityInfoW.
        unsafe { LocalFree(sec_desc.cast()) };
        return Err(io::Error::other(format!(
            "SetEntriesInAclW rc={merge_rc} on {}",
            path.display()
        )));
    }
    // SAFETY: wpath NUL-terminated; new_acl from SetEntriesInAclW. In
    // the deny case the PROTECTED_DACL bit also blocks all future
    // inheritance, so the root grant can never propagate back in.
    let rc = unsafe {
        SetNamedSecurityInfoW(
            wpath.as_mut_ptr(),
            SE_FILE_OBJECT,
            if deny {
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION
            } else {
                DACL_SECURITY_INFORMATION
            },
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            new_acl.cast(),
            core::ptr::null(),
        )
    };
    // SAFETY: new_acl is LocalAlloc'd by SetEntriesInAclW; sec_desc by
    // GetNamedSecurityInfoW. old_acl is an interior pointer and is NOT
    // freed separately.
    unsafe {
        LocalFree(new_acl.cast());
        LocalFree(sec_desc.cast());
    }
    if rc != 0 {
        return Err(io::Error::other(format!(
            "SetNamedSecurityInfoW rc={rc} on {}",
            path.display()
        )));
    }
    Ok(())
}

/// Copy `old_acl` into a fresh ACL, dropping every allowed/denied ACE
/// that names `sid` (the icacls `/inheritance:r` client-side filter:
/// the kernel never strips inherited-flagged ACEs from a provided
/// DACL). Returns the owning buffer.
fn strip_trustee_aces(old_acl: *mut ACL, sid: *mut core::ffi::c_void) -> io::Result<Vec<u8>> {
    // SAFETY: plain repr(C) stats struct, fully overwritten by the call.
    let mut info: ACL_SIZE_INFORMATION = unsafe { core::mem::zeroed() };
    // SAFETY: out-struct sized to its type; read-only walk below.
    if unsafe {
        GetAclInformation(
            old_acl,
            (&mut info as *mut ACL_SIZE_INFORMATION).cast::<core::ffi::c_void>(),
            core::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
            AclSizeInformation,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut kept: Vec<(*mut core::ffi::c_void, u16)> = Vec::new();
    for index in 0..info.AceCount {
        let mut ace: *mut core::ffi::c_void = core::ptr::null_mut();
        // SAFETY: index bounded by AceCount from the query above.
        if unsafe { GetAce(old_acl, index, &mut ace) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: every ACL ACE begins with ACE_HEADER.
        let header = unsafe { *(ace as *const ACE_HEADER) };
        let names_pkg = matches!(header.AceType, 0 | 1)
            && unsafe {
                // SAFETY: allowed/denied ACEs are header + mask(u32) + SID.
                EqualSid(
                    ace.add(core::mem::size_of::<ACE_HEADER>() + core::mem::size_of::<u32>()),
                    sid,
                )
            } != 0;
        if !names_pkg {
            kept.push((ace, header.AceSize));
        }
    }
    let ace_bytes: usize = kept.iter().map(|(_, size)| *size as usize).sum();
    // ACL header plus kept ACEs plus slack for canonicalization
    let total: usize = core::mem::size_of::<ACL>() + ace_bytes + 64;
    let mut buffer = vec![0u8; total];
    // SAFETY: buffer sized above; InitializeAcl writes only the header.
    if unsafe {
        InitializeAcl(
            buffer.as_mut_ptr().cast::<ACL>(),
            total as u32,
            ACL_REVISION,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    for (ace, size) in kept {
        // SAFETY: source ACEs are valid blobs from the old ACL;
        // append (MAXDWORD index) into the fresh, sufficiently sized ACL.
        if unsafe {
            AddAce(
                buffer.as_mut_ptr().cast::<ACL>(),
                ACL_REVISION,
                u32::MAX,
                ace,
                size as u32,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(buffer)
}
