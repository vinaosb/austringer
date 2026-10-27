//! Client-side daemon lifecycle for the "just starts" launch posture
//! (research §34.2, the daemon-by-default convergence receipt): locate
//! the `austringer-daemon` binary, spawn it detached when no daemon
//! answered, and wait for the readiness contract — the `daemon-token`
//! file, which the daemon writes only after both listeners bind (CI
//! receipt 2026-10-26, macOS leg ENOENT race). Every spawn prints a
//! visible notice: loopback-only, pid, token path — never silent.

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::process::Command;

/// The daemon's readiness file under its data dir.
pub const TOKEN_FILENAME: &str = "daemon-token";

/// The daemon's bound base URL (`http://127.0.0.1:PORT`), written just
/// before the token file so anything that waited on readiness can read
/// the URL without parsing daemon stdout.
pub const URL_FILENAME: &str = "daemon-url";

/// Environment override naming the daemon binary (tests, relocatable
/// installs).
pub const DAEMON_BIN_ENV: &str = "AUSTRINGER_DAEMON_BIN";

/// Environment override naming the TUI binary (tests, relocatable
/// installs).
pub const TUI_BIN_ENV: &str = "AUSTRINGER_TUI_BIN";

/// Default daemon data dir and IPC name, mirroring the daemon
/// binary's `DaemonConfig::default()`.
pub const DEFAULT_DATA_DIR: &str = ".austringer-daemon";
pub const DEFAULT_IPC_NAME: &str = "austringer-daemon";

/// Set to `1` to refuse auto-spawn: clients then fail with a typed
/// error telling the user to start the daemon themselves.
pub const NO_AUTOSPAWN_ENV: &str = "AUSTRINGER_NO_AUTOSPAWN";

/// The daemon's detached-startup log, read when its stderr could not
/// be inherited (a piped parent would block on the held handle —
/// `austringer capture | jq` must not wait for the daemon to die).
pub const SPAWN_LOG_FILENAME: &str = "daemon-spawn.log";

const READY_DEADLINE: Duration = Duration::from_secs(15);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error(
        "{which} not found next to the client or on PATH; install it \
         alongside or set {env}"
    )]
    BinaryNotFound { which: String, env: String },
    #[error(
        "auto-spawn refused ({NO_AUTOSPAWN_ENV}=1); start austringer-daemon \
         manually or unset the variable"
    )]
    AutospawnDisabled,
    #[error("spawn {exe}: {source}")]
    Spawn {
        exe: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("daemon exited before readiness (code {code:?}){diagnostic}")]
    Exited {
        code: Option<i32>,
        diagnostic: String,
    },
    #[error(
        "daemon not ready within {} s (no token at {}){diagnostic}",
        READY_DEADLINE.as_secs(),
        token_path.display()
    )]
    NotReady {
        token_path: PathBuf,
        diagnostic: String,
    },
}

/// A freshly spawned, readiness-proven daemon.
pub struct Spawned {
    /// The daemon's process id (for the user's `kill`, not ours).
    pub pid: u32,
    /// Path of the token file whose appearance proved readiness.
    pub token_path: PathBuf,
}

/// Resolve the daemon binary: explicit env override, then a sibling of
/// the running executable (the installer layout), then `PATH`.
///
/// # Errors
/// [`LaunchError::BinaryNotFound`] when no candidate exists.
pub fn find_daemon_exe() -> Result<PathBuf, LaunchError> {
    locate_exe(DAEMON_BIN_ENV, &daemon_binary_name())
}

/// Resolve the TUI binary the same way [`find_daemon_exe`] resolves
/// the daemon (env override, exe-dir sibling, `PATH`).
///
/// # Errors
/// [`LaunchError::BinaryNotFound`] when no candidate exists.
pub fn find_tui_exe() -> Result<PathBuf, LaunchError> {
    locate_exe(TUI_BIN_ENV, &tui_binary_name())
}

fn locate_exe(env_key: &str, name: &str) -> Result<PathBuf, LaunchError> {
    let not_found = || LaunchError::BinaryNotFound {
        which: name.trim_end_matches(".exe").to_owned(),
        env: env_key.to_owned(),
    };
    // An explicit override is honored exactly: it must exist, or the
    // error names it (never silently fall back to a sibling/PATH hit).
    if let Some(set) = override_from(std::env::var(env_key).ok().as_deref()) {
        return if set.is_file() {
            Ok(set)
        } else {
            Err(not_found())
        };
    }
    if let Ok(self_exe) = std::env::current_exe()
        && let Some(dir) = self_exe.parent()
    {
        let sibling = dir.join(name);
        if sibling.is_file() {
            return Ok(sibling);
        }
    }
    search_path(name).ok_or_else(not_found)
}

/// Spawn the daemon detached, announce it, and wait for readiness
/// (the token file changing or appearing under `data_dir`).
///
/// # Errors
/// [`LaunchError::AutospawnDisabled`] when [`NO_AUTOSPAWN_ENV`] is `1`;
/// the spawn/wait errors otherwise.
pub async fn ensure_daemon(data_dir: &Path, ipc_name: &str) -> Result<Spawned, LaunchError> {
    if disabled_by(std::env::var(NO_AUTOSPAWN_ENV).ok().as_deref()) {
        return Err(LaunchError::AutospawnDisabled);
    }
    let exe = find_daemon_exe()?;
    spawn_daemon(&exe, data_dir, ipc_name).await
}

/// Spawn one daemon binary detached and wait until it is ready. The
/// child handle is intentionally orphaned (the daemon outlives its
/// client); its startup stderr stays inherited so failures are loud.
///
/// # Errors
/// spawn or readiness failures, typed.
pub async fn spawn_daemon(
    exe: &Path,
    data_dir: &Path,
    ipc_name: &str,
) -> Result<Spawned, LaunchError> {
    let token_path = data_dir.join(TOKEN_FILENAME);
    let stale_token = std::fs::read(&token_path).ok();
    // Inherit stderr only on a terminal: a detached child holding a
    // piped ancestor handle blocks that pipe until the daemon exits.
    // Non-terminal runs get a log file the errors below point at.
    // Inherit stderr only on a terminal: a detached child holding a
    // piped ancestor handle blocks that pipe until the daemon exits.
    // Non-terminal runs get a log file the errors below point at.
    let inherit_stderr = std::io::stderr().is_terminal();
    let (stderr, diagnostic) = if inherit_stderr {
        (std::process::Stdio::inherit(), String::new())
    } else {
        let log_path = data_dir.join(SPAWN_LOG_FILENAME);
        let diagnostic = format!("; daemon stderr: {}", log_path.display());
        let stderr = std::fs::File::create(&log_path)
            .map_or_else(|_| std::process::Stdio::null(), std::process::Stdio::from);
        (stderr, diagnostic)
    };
    let mut command = Command::new(exe);
    command
        .arg("--data-dir")
        .arg(data_dir)
        .arg("--ipc-name")
        .arg(ipc_name)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(stderr);
    detach(&mut command);
    // A Windows child marked for handle inheritance takes EVERY
    // inheritable handle, including an ancestor's stdout pipe (spawned
    // `austringer capture | jq` would wedge on EOF until the daemon
    // dies). The daemon gets explicit null/file stdio, so it needs
    // none of ours: clear the inherit flag around the spawn, keeping
    // stderr only when it is the inherited terminal.
    #[cfg(windows)]
    clear_std_handle_inheritance(!inherit_stderr);
    let spawned = command.spawn();
    #[cfg(windows)]
    restore_std_handle_inheritance();
    let mut child = spawned.map_err(|source| LaunchError::Spawn {
        exe: exe.to_owned(),
        source,
    })?;
    let pid = child.id().unwrap_or(0);
    eprintln!(
        "austringer: spawned austringer-daemon (pid {pid}) — loopback-only; token: {}",
        token_path.display()
    );
    let deadline = tokio::time::Instant::now() + READY_DEADLINE;
    loop {
        if let Ok(token_now) = std::fs::read(&token_path)
            && Some(&token_now) != stale_token.as_ref()
        {
            std::mem::forget(child);
            return Ok(Spawned { pid, token_path });
        }
        if let Ok(Some(status)) = child.try_wait() {
            return Err(LaunchError::Exited {
                code: status.code(),
                diagnostic,
            });
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(LaunchError::NotReady {
                token_path,
                diagnostic,
            });
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Read the daemon's bound base URL, when a daemon has written one.
#[must_use]
pub fn read_daemon_url(data_dir: &Path) -> Option<String> {
    std::fs::read_to_string(data_dir.join(URL_FILENAME))
        .ok()
        .map(|url| url.trim().trim_end_matches('/').to_owned())
        .filter(|url| !url.is_empty())
}

fn daemon_binary_name() -> String {
    if cfg!(windows) {
        "austringer-daemon.exe".to_owned()
    } else {
        "austringer-daemon".to_owned()
    }
}

fn tui_binary_name() -> String {
    if cfg!(windows) {
        "austringer-tui.exe".to_owned()
    } else {
        "austringer-tui".to_owned()
    }
}

fn override_from(value: Option<&str>) -> Option<PathBuf> {
    let set = value?.trim();
    (!set.is_empty()).then(|| PathBuf::from(set))
}

fn disabled_by(value: Option<&str>) -> bool {
    value.is_some_and(|set| set.trim() == "1")
}

fn search_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

#[cfg(windows)]
fn detach(command: &mut Command) {
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

#[cfg(unix)]
fn detach(command: &mut Command) {
    command.process_group(0);
}

#[cfg(windows)]
fn std_handles(include_stderr: bool) -> Vec<windows_sys::Win32::Foundation::HANDLE> {
    use windows_sys::Win32::System::Console::{
        GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };
    let kinds: &[windows_sys::Win32::System::Console::STD_HANDLE] = if include_stderr {
        &[STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE]
    } else {
        &[STD_INPUT_HANDLE, STD_OUTPUT_HANDLE]
    };
    // SAFETY: GetStdHandle only reads the PEB's stored handle values.
    kinds
        .iter()
        .map(|kind| unsafe { GetStdHandle(*kind) })
        .filter(|handle| {
            !handle.is_null() && *handle != windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE
        })
        .collect()
}

#[cfg(windows)]
fn set_std_inherit(include_stderr: bool, enabled: bool) {
    use windows_sys::Win32::Foundation::{HANDLE_FLAG_INHERIT, SetHandleInformation};
    let flag = if enabled { HANDLE_FLAG_INHERIT } else { 0 };
    for handle in std_handles(include_stderr) {
        // SAFETY: best-effort flag flip on our own standard handles;
        // failures (console pseudo-handles) are irrelevant here.
        unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, flag) };
    }
}

#[cfg(windows)]
fn clear_std_handle_inheritance(clear_stderr_too: bool) {
    set_std_inherit(clear_stderr_too, false);
}

#[cfg(windows)]
fn restore_std_handle_inheritance() {
    set_std_inherit(true, true);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_override_wins_when_set_and_nonempty() {
        assert_eq!(
            override_from(Some("  /opt/austringer-daemon ")),
            Some(PathBuf::from("/opt/austringer-daemon"))
        );
        assert_eq!(override_from(Some("   ")), None);
        assert_eq!(override_from(None), None);
    }

    #[test]
    fn autospawn_disables_only_on_exact_one() {
        assert!(disabled_by(Some("1")));
        assert!(disabled_by(Some(" 1 ")));
        assert!(!disabled_by(Some("0")));
        assert!(!disabled_by(Some("")));
        assert!(!disabled_by(None));
    }

    #[test]
    fn url_reader_trims_and_rejects_empty() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
        assert_eq!(read_daemon_url(dir.path()), None);
        std::fs::write(dir.path().join(URL_FILENAME), "http://127.0.0.1:9/\n").unwrap_or_default();
        assert_eq!(
            read_daemon_url(dir.path()),
            Some("http://127.0.0.1:9".to_owned())
        );
    }

    #[test]
    fn find_daemon_exe_honors_the_env_override() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
        let exe = dir.path().join("fake-daemon");
        std::fs::write(&exe, b"").unwrap_or_default();
        // SAFETY: single-threaded test runtime; the variable is restored
        // before any other test can observe it.
        unsafe {
            std::env::set_var(DAEMON_BIN_ENV, &exe);
        }
        let found = find_daemon_exe();
        unsafe {
            std::env::remove_var(DAEMON_BIN_ENV);
        }
        assert_eq!(found.unwrap_or_else(|e| panic!("find: {e}")), exe);
    }
}
