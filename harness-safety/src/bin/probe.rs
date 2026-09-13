//! Sandbox escape-attempt probe binary (plan todo 26).
//!
//! Runs INSIDE the sandbox as the attack surface: attempts one filesystem
//! write per invocation and exits with a code that distinguishes outcomes.
//! Exit codes: 42 = write succeeded (ESCAPE when the target was supposed to
//! be denied), 5 = write denied by the OS (EPERM/EACCES - expected for
//! denied targets), 6 = bad invocation. The runtime escape suite spawns
//! this binary through the sandbox backend and asserts exit codes plus
//! on-disk evidence - never booleans alone.

#![deny(missing_docs)]

#[cfg(windows)]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 || args[1] != "--write" {
        std::process::exit(6);
    }
    let target = std::path::PathBuf::from(&args[2]);
    match std::fs::write(&target, b"escape-attempt") {
        Ok(()) => std::process::exit(42),
        Err(error) if denied(&error) => std::process::exit(5),
        Err(_) => std::process::exit(5),
    }
}

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn denied(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::NotFound
    )
}
