//! Daemon configuration: one struct the binary, servers, and tests
//! share. Defaults are the hardened posture: loopback bind, generated
//! token, bounded queues, durable execution ON.

use std::path::PathBuf;
use std::time::Duration;

use crate::mediation::MediationPolicy;
use crate::provider::ProviderSelection;

/// Daemon-wide configuration.
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// HTTP/WS/SSE bind address. Hard default `127.0.0.1:0` (loopback
    /// only; the auth layer additionally refuses non-loopback peers).
    pub bind_addr: String,
    /// Namespaced local-socket name for the IPC transport (named pipe
    /// `\\.\pipe\<name>` on Windows, abstract/tmp namespace on Unix —
    /// no filesystem socket corpses by construction).
    pub ipc_name: String,
    /// Fixed auth token. `None` generates one from OS entropy at
    /// startup.
    pub token: Option<String>,
    /// Data root: journals land under `{data}/journal/{session}/`.
    pub data_dir: PathBuf,
    /// Replay-ring capacity (frames retained per session for delta
    /// resync).
    pub ring_capacity: usize,
    /// Live fan-out capacity per session (lagging receivers get a
    /// forced resync, never unbounded buffers).
    pub broadcast_capacity: usize,
    /// Per-session attach cap (reconnect-storm bound).
    pub max_clients_per_session: usize,
    /// Idle read timeout before a client connection is dropped (the
    /// session survives).
    pub idle_timeout: Duration,
    /// Permission mediation policy for new sessions.
    pub ask_policy: MediationPolicy,
    /// Synchronous ask deadline (N1 timeout arm).
    pub ask_timeout: Duration,
    /// Durable turn-workflow journals (default ON; overhead is one
    /// JSONL append per step).
    pub durable: bool,
    /// Remote-worker per-call timeout.
    pub worker_call_timeout: Duration,
    /// Provider seam for the session factory (M32). `None` — the
    /// default — leaves the daemon unconfigured: no provider is ever
    /// enabled implicitly, and inference fails typed and actionable.
    pub provider: Option<ProviderSelection>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:0".into(),
            ipc_name: "harness-daemon".into(),
            token: None,
            data_dir: PathBuf::from(".harness-daemon"),
            ring_capacity: 1024,
            broadcast_capacity: 256,
            max_clients_per_session: 64,
            idle_timeout: Duration::from_secs(300),
            ask_policy: MediationPolicy::FirstResponder,
            ask_timeout: Duration::from_secs(120),
            durable: true,
            worker_call_timeout: Duration::from_secs(60),
            provider: None,
        }
    }
}
