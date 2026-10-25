//! Seccomp deny-all-egress filter for the Linux backend (todo 26):
//! blocks socket(AF_INET/AF_INET6) with EPERM when the egress allowlist
//! is empty. Hand-built cBPF; jump offsets are security-critical (a
//! silent miss = filter that allows). Typecheck-proven for
//! x86_64-unknown-linux-gnu; runtime owned by CI.

pub(crate) fn install_seccomp_net_filter() -> std::io::Result<()> {
    #[repr(C)]
    struct SockFilter {
        code: u16,
        jt: u8,
        jf: u8,
        k: u32,
    }
    #[repr(C)]
    struct SockFprog {
        len: u16,
        filter: *mut SockFilter,
    }
    const BPF_LD: u16 = 0x00;
    const BPF_W: u16 = 0x00;
    const BPF_ABS: u16 = 0x20;
    const BPF_JMP: u16 = 0x05;
    const BPF_JEQ: u16 = 0x10;
    const BPF_RET: u16 = 0x06;
    const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;
    const SECCOMP_RET_ERRNO: u32 = 0x0005_0000;
    const EPERM: u32 = 1;
    const AUDIT_ARCH_X86_64: u32 = 0xc000_003e;
    const SYS_SOCKET: u32 = 41;
    const AF_INET: u32 = 2;
    const AF_INET6: u32 = 10;
    // program: load arch -> must be x86_64; load nr -> socket(41)?
    // load a0 -> AF_INET/AF_INET6? -> ERRNO|EPERM; everything else ALLOW
    let mut prog = [
        SockFilter {
            code: BPF_LD | BPF_W | BPF_ABS,
            jt: 0,
            jf: 0,
            k: 4,
        },
        SockFilter {
            code: BPF_JMP | BPF_JEQ,
            jt: 0,
            jf: 6,
            k: AUDIT_ARCH_X86_64,
        },
        SockFilter {
            code: BPF_LD | BPF_W | BPF_ABS,
            jt: 0,
            jf: 0,
            k: 0,
        },
        SockFilter {
            code: BPF_JMP | BPF_JEQ,
            jt: 0,
            jf: 4,
            k: SYS_SOCKET,
        },
        SockFilter {
            code: BPF_LD | BPF_W | BPF_ABS,
            jt: 0,
            jf: 0,
            k: 16,
        },
        SockFilter {
            code: BPF_JMP | BPF_JEQ,
            jt: 1,
            jf: 0,
            k: AF_INET,
        },
        SockFilter {
            code: BPF_JMP | BPF_JEQ,
            jt: 0,
            jf: 1,
            k: AF_INET6,
        },
        SockFilter {
            code: BPF_RET | BPF_W,
            jt: 0,
            jf: 0,
            k: SECCOMP_RET_ERRNO | EPERM,
        },
        SockFilter {
            code: BPF_RET | BPF_W,
            jt: 0,
            jf: 0,
            k: SECCOMP_RET_ALLOW,
        },
    ];
    let fprog = SockFprog {
        len: prog.len() as u16,
        filter: prog.as_mut_ptr(),
    };
    // SAFETY: fprog references prog for the duration of the call.
    let rc = unsafe {
        libc::prctl(
            libc::PR_SET_SECCOMP,
            libc::SECCOMP_MODE_FILTER as libc::c_ulong,
            &fprog as *const SockFprog as libc::c_ulong,
            0,
            0,
        )
    };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}
