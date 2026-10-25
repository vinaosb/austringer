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
    /// Upload cap for `POST /s/{sid}/media` (and the GET fetch cap).
    /// Default 512 MiB; bodies beyond it fail typed 413 mid-stream.
    pub media_upload_cap_bytes: u64,
    /// Provider seam for the session factory (M32). `None` — the
    /// default — leaves the daemon unconfigured: no provider is ever
    /// enabled implicitly, and inference fails typed and actionable.
    pub provider: Option<ProviderSelection>,
    /// The per-session money cap in micro-USD (todo 33): every session
    /// (scheduled ones included) governs its spend against this —
    /// warning at 80%, a typed terminal stop at 100%. User-global only
    /// (`austringer_SESSION_COST_CAP_USD`, parsed by
    /// [`parse_session_cost_cap_usd`]); no repo-local override.
    pub session_cost_cap_micros: u128,
    /// The user-global test-command override for the `run_tests` tool
    /// (todo 17): replaces framework detection (cargo nextest / cargo
    /// test) with a custom command line. Read ONLY from daemon-owned
    /// user-global config (`austringer_TEST_COMMAND`, parsed by the
    /// binary) — a repo-local override file is refused with a typed
    /// error by the tool itself.
    pub test_command: Option<String>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:0".into(),
            ipc_name: "austringer-daemon".into(),
            token: None,
            data_dir: PathBuf::from(".austringer-daemon"),
            ring_capacity: 1024,
            broadcast_capacity: 256,
            max_clients_per_session: 64,
            idle_timeout: Duration::from_secs(300),
            ask_policy: MediationPolicy::FirstResponder,
            ask_timeout: Duration::from_secs(120),
            durable: true,
            worker_call_timeout: Duration::from_secs(60),
            media_upload_cap_bytes: 512 * 1024 * 1024,
            provider: None,
            session_cost_cap_micros: crate::cost_ledger::DEFAULT_SESSION_COST_CAP_MICROS,
            test_command: None,
        }
    }
}

/// Parses the user-global session cost cap override: decimal dollars
/// (integer math — the fractional part scales to micro-USD, at most 6
/// digits because that is the micro boundary). Typed rejection for
/// anything else; never a float anywhere in the money path.
///
/// # Errors
/// A message naming the accepted shape for unparseable input.
pub fn parse_session_cost_cap_usd(raw: &str) -> Result<u128, String> {
    let accepted = "decimal dollars, at most 6 fractional digits (e.g. 12.50)";
    let rejected = || format!("austringer_SESSION_COST_CAP_USD must be {accepted}, got {raw:?}");
    let (whole, fractional) = match raw.split_once('.') {
        Some((whole, fractional)) if !fractional.is_empty() => (whole, fractional),
        Some(_) => return Err(rejected()),
        None => (raw, ""),
    };
    if whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || fractional.len() > 6
        || !fractional.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(rejected());
    }
    let whole: u128 = whole.parse().map_err(|_| rejected())?;
    // Bare integers (no dot) carry an empty fractional part.
    let scaled: u128 = if fractional.is_empty() {
        0
    } else {
        fractional.parse().map_err(|_| rejected())?
    };
    let micros = whole
        .checked_mul(1_000_000)
        .and_then(|base| base.checked_add(scaled * 10_u128.pow(6 - fractional.len() as u32)))
        .ok_or_else(|| format!("austringer_SESSION_COST_CAP_USD overflows, got {raw:?}"))?;
    Ok(micros)
}
