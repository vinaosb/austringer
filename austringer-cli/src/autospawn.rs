//! The "just starts" transport path: on the IPC default transport a
//! missing token or a dead daemon spawns one `austringer-daemon`
//! (visible notice; see [`austringer_launch`]) and retries resolution
//! or connection exactly once. Explicit `--url` or `--token` inputs
//! are the user managing the daemon: never spawned, never retried.

use austringer_launch::LaunchError;

use crate::client::{Connect, SessionTransport};
use crate::error::CliError;
use crate::target::{DEFAULT_DATA_DIR, DEFAULT_IPC_NAME, Target, TargetSpec};

/// Auto-spawn is only for the pure default path: no `--url`, no
/// `--token`.
fn spawn_allowed(spec: &TargetSpec) -> bool {
    spec.url.is_none() && spec.token.is_none()
}

/// Resolve the transport target, spawning a daemon first when the
/// default IPC path has no token file (no daemon has ever run here).
///
/// # Errors
/// the resolve failure itself, or the typed spawn failure.
pub async fn resolve_target(spec: &TargetSpec) -> Result<Target, CliError> {
    match Target::resolve(spec) {
        Ok(target) => Ok(target),
        Err(_) if spawn_allowed(spec) => {
            ensure_daemon(spec).await?;
            Target::resolve(spec)
        }
        Err(failure) => Err(failure),
    }
}

/// Connect the session transport, spawning a daemon first when a token
/// exists but no daemon answered it (a stale generation left behind by
/// a crashed daemon). `allow_spawn` comes from [`spawn_allowed`]; the
/// retry re-reads the token file the fresh daemon just wrote.
///
/// # Errors
/// the connect failure itself, or the typed spawn failure.
pub async fn connect_transport(
    connect: &Connect,
    allow_spawn: bool,
) -> Result<SessionTransport, CliError> {
    match SessionTransport::connect(connect).await {
        Ok(transport) => Ok(transport),
        Err(_) if allow_spawn && connect.url.is_none() => {
            ensure_daemon_for(&connect.data_dir, &connect.ipc_name).await?;
            let mut retry = connect.clone();
            if let Some(token) = crate::target::read_token(&connect.data_dir) {
                retry.token = Some(token);
            }
            SessionTransport::connect(&retry).await
        }
        Err(failure) => Err(failure),
    }
}

async fn ensure_daemon(spec: &TargetSpec) -> Result<(), CliError> {
    let data_dir = spec
        .data_dir
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from(DEFAULT_DATA_DIR));
    let ipc_name = spec
        .ipc_name
        .clone()
        .unwrap_or_else(|| DEFAULT_IPC_NAME.to_owned());
    ensure_daemon_for(&data_dir, &ipc_name).await
}

async fn ensure_daemon_for(data_dir: &std::path::Path, ipc_name: &str) -> Result<(), CliError> {
    austringer_launch::ensure_daemon(data_dir, ipc_name)
        .await
        .map(|_| ())
        .map_err(launch_failure)
}

fn launch_failure(failure: LaunchError) -> CliError {
    CliError::Unreachable {
        detail: failure.to_string(),
    }
}
