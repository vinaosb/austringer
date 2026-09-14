//! FS-contention hardening (plan todo 20): retry-with-backoff on
//! `ERROR_SHARING_VIOLATION` (Defender/OneDrive/indexer locks on freshly
//! written files), long-path (`\\?\`) awareness, and OneDrive-synced-folder
//! detection with a warning surface.
//!
//! Wired into harness-exec's own file operations (the spill writer);
//! harness-tools' edit paths adopt the same helpers in todo 22's
//! pre-flight — this module is the shared home on purpose.

use std::io;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;

/// Win32 `ERROR_SHARING_VIOLATION`: another process holds the file with
/// an incompatible share mode (AV scan, sync engine, indexer).
pub const ERROR_SHARING_VIOLATION: i32 = 32;

/// How hard a contended operation retries before surfacing the error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryBudget {
    /// Total attempts (first try included).
    pub attempts: u8,
    /// Backoff before the second attempt; doubles each attempt.
    pub initial_backoff: Duration,
}

impl Default for RetryBudget {
    fn default() -> Self {
        // ~310 ms of total backoff across 6 attempts: far outlasts a
        // Defender on-open scan, well under any command timeout budget.
        Self {
            attempts: 6,
            initial_backoff: Duration::from_millis(10),
        }
    }
}

impl RetryBudget {
    #[must_use]
    pub fn new(attempts: u8, initial_backoff: Duration) -> Self {
        Self {
            attempts,
            initial_backoff,
        }
    }
}

/// Runs `operation`, retrying only while it fails with
/// `ERROR_SHARING_VIOLATION`. Any other error returns immediately.
pub fn retry_sharing_violation<T>(
    budget: &RetryBudget,
    operation: impl Fn() -> io::Result<T>,
) -> io::Result<T> {
    let mut backoff = budget.initial_backoff;
    let mut attempt = 1;
    loop {
        match operation() {
            Ok(value) => return Ok(value),
            Err(error) if error.raw_os_error() == Some(ERROR_SHARING_VIOLATION) => {
                if attempt >= budget.attempts {
                    return Err(error);
                }
                sleep(backoff);
                backoff = backoff.saturating_mul(2);
                attempt += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

/// Extended-length form of an absolute path (`\\?\C:\…`), which lifts the
/// 260-char MAX_PATH limit for the APIs that accept it. Relative paths,
/// already-extended paths, and UNC paths (32k-enabled by nature) pass
/// through unchanged.
#[must_use]
pub fn long_path(path: &Path) -> PathBuf {
    // The \\?\ verbatim form is a Windows-only long-path mechanism; on
    // unix it is not a valid path and corrupts POSIX prefixes.
    if !cfg!(windows) {
        return path.to_path_buf();
    }
    let text = path.as_os_str().to_string_lossy();
    if !path.is_absolute() || text.starts_with(r"\\?\") || text.starts_with(r"\\.\") {
        return path.to_path_buf();
    }
    if text.starts_with(r"\\") {
        return path.to_path_buf();
    }
    let mut extended = String::with_capacity(text.len() + 4);
    extended.push_str(r"\\?\");
    extended.push_str(&text);
    PathBuf::from(extended)
}

/// Whether a path lives inside a folder a sync engine mirrors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnedriveStatus {
    /// Under a OneDrive root: file locks and on-open scans are likely;
    /// embedders should surface a warning.
    Synced,
    /// Ordinary local storage.
    Local,
}

/// Pure classification against explicit roots (case-insensitive,
/// separator-normalized); the env-aware wrapper is [`onedrive_status`].
#[must_use]
pub fn onedrive_status_in(path: &Path, roots: &[PathBuf]) -> OnedriveStatus {
    let normalize = |p: &Path| {
        p.to_string_lossy()
            .to_lowercase()
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_string()
    };
    let target = normalize(path);
    for root in roots {
        let root = normalize(root);
        if target == root || target.starts_with(&format!("{root}\\")) {
            return OnedriveStatus::Synced;
        }
    }
    OnedriveStatus::Local
}

/// Classification against the machine's OneDrive roots
/// (`%OneDrive%`, `%OneDriveCommercial%`, `%OneDriveConsumer%`).
#[must_use]
pub fn onedrive_status(path: &Path) -> OnedriveStatus {
    let roots: Vec<PathBuf> = ["OneDrive", "OneDriveCommercial", "OneDriveConsumer"]
        .iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .collect();
    onedrive_status_in(path, &roots)
}
