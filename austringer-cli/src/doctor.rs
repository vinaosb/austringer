//! `austringer doctor` (plan todo 39): runs every probe, prints one
//! status line each, exits nonzero when any probe FAILED — "doctor
//! exits nonzero on induced failure" is the plan's acceptance.

use std::path::{Path, PathBuf};

use crate::error::CliError;
use crate::target::{DEFAULT_DATA_DIR, Target};
use crate::{doctor_probes as probes, doctor_probes::Status};

/// Doctor options.
#[derive(Debug, Clone, Default)]
pub struct DoctorOptions {
    /// `--sessions` root override (defaults under the cwd).
    pub sessions_root: Option<PathBuf>,
}

/// Default sessions root when `--sessions` is absent: the daemon's
/// data dir — the root the daemon passes to `SessionStore::open` —
/// under `base`.
pub fn default_sessions_root(base: &Path) -> PathBuf {
    base.join(DEFAULT_DATA_DIR)
}

/// Run every probe and report. Returns the exit code.
///
/// # Errors
/// never — probe failures are reported lines, not exceptions; the
/// nonzero exit code is the typed failure surface.
pub async fn doctor(target: &Target, options: &DoctorOptions) -> Result<i32, CliError> {
    let daemon = probes::daemon(target).await;
    let probes = [
        daemon.clone(),
        probes::network(&daemon),
        probes::provider(),
        probes::env(),
        probes::sandbox(),
        probes::mcp(),
        probes::sessions(&options.sessions_root.clone().unwrap_or_else(|| {
            default_sessions_root(&std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        })),
        probes::skills(),
    ];
    let mut failed = false;
    for probe in &probes {
        let verdict = match probe.status {
            Status::Pass => "PASS",
            Status::Warn => "WARN",
            Status::Fail => {
                failed = true;
                "FAIL"
            }
        };
        println!("{:<9} {verdict}: {}", probe.name, probe.detail);
    }
    Ok(i32::from(failed))
}

#[cfg(test)]
mod tests {
    use super::default_sessions_root;
    use crate::target::DEFAULT_DATA_DIR;

    /// Doctor's default store root must be the daemon's data dir —
    /// the root `SessionStore::open` actually reads in
    /// `DaemonConfig::default()` — not the retired `.austringer/store`.
    #[test]
    fn default_sessions_root_matches_daemon_data_dir() {
        let base = tempfile::tempdir().expect("tempdir");
        let root = default_sessions_root(base.path());
        assert_eq!(root, base.path().join(DEFAULT_DATA_DIR));
        assert!(!root.ends_with(".austringer/store"));
    }
}
