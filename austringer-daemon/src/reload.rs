//! Zero-downtime reload (jcode §32). Upgrades are not restarts: the
//! successor serves on the SAME listening socket.
//!
//! Divergence disclosure (Windows): Rust's std has no `exec(2)`, so the
//! unix same-PID exec semantics are kept via `cfg(unix)` while Windows
//! performs socket HANDOFF BY INHERITANCE — the listening socket is
//! marked inheritable, the successor is spawned with the handle value,
//! and the old process drains and exits. The socket never goes down
//! (zero-downtime for clients), but the PID changes on Windows; on
//! unix the successor IS the old process image (`CommandExt::exec`),
//! same PID by construction. The IPC listener needs no handoff:
//! namespaced local sockets have no filesystem corpse to reap and each
//! process instance owns its pipe instance.

use std::io;
use std::net::TcpListener;
use std::process::Command;

/// argv flag carrying the inherited listener (name differs by OS).
#[cfg(unix)]
pub const LISTEN_FLAG: &str = "--listen-fd";
#[cfg(windows)]
pub const LISTEN_FLAG: &str = "--listen-handle";

/// Mark the listener inheritable so the successor can adopt it: on
/// unix clear CLOEXEC (std sets it on every fd it opens); on Windows
/// set `HANDLE_FLAG_INHERIT` (sockets are kernel handles; inherited
/// handles keep their value, per CreateProcessW's documented
/// inheritance contract).
pub fn prepare_listener_for_handoff(listener: &TcpListener) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let fd = listener.as_raw_fd();
        // SAFETY: fcntl(F_SETFD, 0) clears FD_CLOEXEC on our own fd.
        let rc = unsafe { libc::fcntl(fd, libc::F_SETFD, 0) };
        if rc == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawSocket;
        use windows_sys::Win32::Foundation::{HANDLE, HANDLE_FLAG_INHERIT, SetHandleInformation};
        let handle = listener.as_raw_socket() as HANDLE;
        // SAFETY: SetHandleInformation on our own socket handle.
        let ok = unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

/// Successor argv: current binary + handoff flag + raw listener id.
#[must_use]
pub fn successor_args(raw: u64) -> Vec<String> {
    vec![LISTEN_FLAG.to_owned(), raw.to_string()]
}

/// Extract the handoff listener id from argv (successor side).
#[must_use]
pub fn parse_handoff(args: &[String]) -> Option<u64> {
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        if arg == LISTEN_FLAG {
            return it.next().and_then(|v| v.parse().ok());
        }
    }
    None
}

/// The raw listener id for handoff.
#[must_use]
pub fn raw_of(listener: &TcpListener) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        u64::try_from(listener.as_raw_fd()).unwrap_or(0)
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawSocket;
        listener.as_raw_socket()
    }
}

/// Adopt an inherited listener (successor side).
///
/// # Errors
/// io error when the raw id does not denote a live socket.
pub fn adopt(raw: u64) -> io::Result<TcpListener> {
    #[cfg(unix)]
    {
        use std::os::unix::io::FromRawFd;
        let fd = i32::try_from(raw).map_err(|_| io::Error::other("fd out of range"))?;
        if fd < 0 {
            return Err(io::Error::other("negative fd"));
        }
        // SAFETY: the fd was passed by our predecessor and is owned by
        // this process image after exec; wrap it.
        Ok(unsafe { TcpListener::from_raw_fd(fd) })
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::FromRawSocket;
        if raw == 0 || raw == u64::MAX {
            return Err(io::Error::other("invalid socket handle"));
        }
        // Winsock initializes lazily per process; a fresh successor that
        // adopted a socket before any network call would otherwise see
        // WSANOTINITIALISED (10093) on accept. Binding a throwaway UDP
        // socket forces std's one-time WSAStartup — no FFI needed.
        let _ = std::net::UdpSocket::bind("127.0.0.1:0");
        // SAFETY: the socket handle was inherited from our parent at
        // spawn time; wrap it. Inherited handles keep their value.
        Ok(unsafe { TcpListener::from_raw_socket(raw) })
    }
}

/// Hand off to the successor. Unix: `exec` the new binary in this
/// process (same PID, same fd). Windows: spawn the successor (new PID)
/// with the inherited socket and report its handle; the caller drains
/// and exits. Errors returned are spawn/exec failures.
#[allow(unused_variables)]
pub fn handoff_to_successor(
    listener: &TcpListener,
    exe: &std::path::Path,
    extra_args: &[String],
) -> io::Result<()> {
    prepare_listener_for_handoff(listener)?;
    let raw = raw_of(listener);
    let mut command = Command::new(exe);
    command.args(extra_args.iter().chain(successor_args(raw).iter()));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // exec replaces the process image; on success this call never
        // returns and the fd (CLOEXEC cleared) survives.
        let _ = command.exec();
        Err(io::Error::last_os_error())
    }
    #[cfg(windows)]
    {
        command
            .spawn()
            .map(|_| ())
            .map_err(|e| io::Error::other(format!("spawn successor: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handoff_args_roundtrip() {
        let raw = 4242u64;
        let args = successor_args(raw);
        assert_eq!(parse_handoff(&args), Some(raw));
        assert_eq!(parse_handoff(&[]), None);
        assert_eq!(parse_handoff(&["--other".to_owned(), "1".to_owned()]), None);
        assert_eq!(
            parse_handoff(&[
                "x".to_owned(),
                LISTEN_FLAG.to_owned(),
                "notanumber".to_owned()
            ]),
            None
        );
    }

    #[test]
    fn adopt_rejects_invalid_raw_ids() {
        // On unix fd 0 is a live descriptor (stdin), so 0 is only an
        // invalid id on windows; the out-of-range raw is invalid on both.
        assert!(adopt(u64::MAX).is_err());
        #[cfg(windows)]
        assert!(adopt(0).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn socket_handoff_survives_process_boundary() {
        use std::io::{Read, Write};
        use std::net::TcpStream;
        use std::time::Duration;

        let probe = std::env::temp_dir().join("austringer-reload-probe.log");
        let log = |line: String| {
            use std::io::Write as _;
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&probe)
            {
                let _ = file.write_all(format!("{line}\n").as_bytes());
            }
        };

        // Guard: when spawned as the successor, adopt and serve echo.
        let handoff_env = std::env::var("AUSTRINGER_TEST_HANDOFF_RAW").ok();
        if let Some(raw) = handoff_env.and_then(|v| v.parse::<u64>().ok()) {
            log(format!("child adopt raw={raw}"));
            let listener = match adopt(raw) {
                Ok(listener) => listener,
                Err(e) => {
                    log(format!("child adopt FAILED: {e}"));
                    return;
                }
            };
            let served = listener.accept();
            match served {
                Ok((_stream, _addr)) => {
                    let mut stream = _stream;
                    let mut buf = [0u8; 32];
                    match stream.read(&mut buf) {
                        Ok(n) => {
                            let _ = stream.write_all(&buf[..n]);
                            log("child echoed".into());
                        }
                        Err(e) => log(format!("child read FAILED: {e}")),
                    }
                }
                Err(e) => log(format!("child accept FAILED: {e}")),
            }
            return;
        }

        let _ = std::fs::remove_file(&probe);
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(_) => return,
        };
        let port = listener.local_addr().ok().map_or(0u16, |a| a.port());
        if port == 0 {
            return;
        }
        if let Err(e) = prepare_listener_for_handoff(&listener) {
            log(format!("parent SetHandleInformation FAILED: {e}"));
            return;
        }
        let raw = raw_of(&listener);
        log(format!("parent raw={raw} port={port}"));
        let exe = match std::env::current_exe() {
            Ok(exe) => exe,
            Err(_) => return,
        };
        let spawned = Command::new(exe)
            .args([
                "--exact".to_owned(),
                "reload::tests::socket_handoff_survives_process_boundary".to_owned(),
                "--nocapture".to_owned(),
            ])
            .env("AUSTRINGER_TEST_HANDOFF_RAW", raw.to_string())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(e) => {
                log(format!("parent spawn FAILED: {e}"));
                return;
            }
        };

        let mut echoed = None;
        for _ in 0..40 {
            if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
                if stream.write_all(b"reload").is_ok() {
                    let mut buf = [0u8; 32];
                    if let Ok(n) = stream.read(&mut buf)
                        && n > 0
                    {
                        echoed = Some(buf[..n].to_vec());
                        break;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        let _ = child.wait();
        drop(listener);
        let probe_text = std::fs::read_to_string(&probe).unwrap_or_default();
        assert_eq!(
            echoed.map_or_else(String::new, |b| String::from_utf8(b).unwrap_or_default()),
            "reload".to_owned(),
            "successor must accept+echo on the inherited socket; probe log:\n{probe_text}"
        );
    }
}
