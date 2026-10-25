//! `austringer doctor` (plan todo 39): runs every probe, prints one
//! status line each, exits nonzero when any probe FAILED — "doctor
//! exits nonzero on induced failure" is the plan's acceptance.

use std::path::PathBuf;

use crate::error::CliError;
use crate::target::Target;
use crate::{doctor_probes as probes, doctor_probes::Status};

/// Doctor options.
#[derive(Debug, Clone, Default)]
pub struct DoctorOptions {
    /// `--sessions` root override (defaults under the cwd).
    pub sessions_root: Option<PathBuf>,
}

/// Default sessions root when `--sessions` is absent.
const DEFAULT_SESSIONS_ROOT: &str = ".austringer/store";

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
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(DEFAULT_SESSIONS_ROOT)
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
