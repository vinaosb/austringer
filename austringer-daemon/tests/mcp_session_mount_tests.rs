//! Todo 12 acceptance: MCP servers declared ENABLED through the
//! manifest path mount into daemon sessions as REAL tools. A fixture
//! in-process MCP server (duplex wire — zero spawned processes, zero
//! network) declared via `.austringer/mcp.json` with
//! `trusted_autostart: true` and an approved fingerprint:
//! `DaemonState::session` mounts its tools into the session registry
//! (observable through the model-facing `tool_search` hydration — the
//! wire surface is the bounded six-tool core by design, aux tools ride
//! the registry), a model tool call dispatched through the daemon
//! engine reaches the fixture server, untrusted/disabled declarations
//! mount NOTHING, and eviction + teardown leave no client handle alive.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::broadcast;

use austringer_core::agent_loop::inference::{
    InferenceError, InferenceRequest, PendingCall, TASK_COMPLETED_TOOL,
};
use austringer_core::agent_loop::{AssistantTurn, CancelToken, Inference};
use austringer_core::event::ContentBlock;

use austringer_daemon::attach::HostCommand;
use austringer_daemon::config::DaemonConfig;
use austringer_daemon::mcp_mounts::McpMounts;
use austringer_daemon::mediation::{MediationPolicy, PermissionMediator};
use austringer_daemon::protocol::{Answer, ServerMsg, SessionEvent};
use austringer_daemon::server::DaemonState;
use austringer_daemon::tools::CoreExecutors;

use austringer_mcp::allowlist::OrgAllowlist;
use austringer_mcp::client::McpClient;
use austringer_mcp::manager::scan_directories;
use austringer_mcp::pool::{Connector, PoolConfig, PoolEvent, TransportPool};
use austringer_mcp::reference_server::{ReferenceFsServer, serve_duplex, write_fixture};
use austringer_mcp::trust::{MemoryTrustStore, TrustStore};

type Factory = Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync>;

/// Scripted inference over a SHARED step queue: consumption survives
/// host eviction and re-creation (each factory call yields a handle
/// onto the same deque).
struct ScriptedInference {
    steps: Arc<Mutex<VecDeque<AssistantTurn>>>,
}

#[async_trait]
impl Inference for ScriptedInference {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        lock(&self.steps)
            .pop_front()
            .ok_or_else(|| InferenceError::Failure("script exhausted".into()))
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn shared_script(steps: Vec<AssistantTurn>) -> Factory {
    let queue = Arc::new(Mutex::new(VecDeque::from(steps)));
    Arc::new(move || {
        Box::new(ScriptedInference {
            steps: Arc::clone(&queue),
        })
    })
}

fn text_turn(text: &str, calls: Vec<PendingCall>) -> AssistantTurn {
    AssistantTurn {
        blocks: vec![ContentBlock::Text {
            text: text.to_owned(),
        }],
        tool_calls: calls,
        usage: None,
    }
}

fn completion_turn() -> AssistantTurn {
    text_turn(
        "done",
        vec![PendingCall {
            call_id: "done".into(),
            tool: TASK_COMPLETED_TOOL.to_owned(),
            arguments: serde_json::json!({}),
        }],
    )
}

fn call(id: &str, tool: &str, arguments: serde_json::Value) -> PendingCall {
    PendingCall {
        call_id: id.to_owned(),
        tool: tool.to_owned(),
        arguments,
    }
}

const READ_FILE: &str = "mcp__fs__read_file";

/// The full acceptance script: hydrate the mounted schemas through the
/// model-facing search, call the mounted tool, complete.
fn mount_script() -> Vec<AssistantTurn> {
    vec![
        text_turn(
            "searching for the mounted tools",
            vec![call(
                "t0",
                "tool_search",
                serde_json::json!({ "query": "mcp__fs" }),
            )],
        ),
        text_turn(
            "reading through the mounted server",
            vec![call(
                "t1",
                READ_FILE,
                serde_json::json!({ "path": "notes/a.txt" }),
            )],
        ),
        completion_turn(),
    ]
}

async fn read_live_until_finished(mut live: broadcast::Receiver<ServerMsg>) -> Vec<String> {
    let mut frames = Vec::new();
    loop {
        match live.recv().await {
            Ok(ServerMsg::Event { frame }) => {
                let finished = matches!(frame.event, SessionEvent::TurnFinished { .. });
                if let Ok(encoded) = serde_json::to_string(&ServerMsg::Event { frame }) {
                    frames.push(encoded);
                }
                if finished {
                    return frames;
                }
            }
            Ok(_) => {}
            Err(broadcast::error::RecvError::Lagged(_)) => {}
            Err(broadcast::error::RecvError::Closed) => return frames,
        }
    }
}

fn spawn_answerer(mediator: Arc<PermissionMediator>, client: &str) -> tokio::task::JoinHandle<()> {
    let client = client.to_owned();
    let mut watch = mediator.subscribe();
    tokio::spawn(async move {
        while let Ok(msg) = watch.recv().await {
            if let ServerMsg::PermissionAsk { ask } = msg {
                let _ = mediator.answer(ask.ask_id, &client, Answer::AllowOnce);
            }
        }
    })
}

async fn drive_turn(state: &DaemonState, sid: &str) -> Vec<String> {
    let host = state.session(sid);
    let attach = host.attach("tui", None).expect("attach");
    let answerer = spawn_answerer(host.mediator(), "tui");
    let reader = tokio::spawn(read_live_until_finished(attach.events.live));
    host.handle_command(HostCommand::Submit("go".into()))
        .await
        .expect("submit");
    let collected = tokio::time::timeout(Duration::from_secs(90), reader)
        .await
        .expect("turn never finished")
        .expect("reader failed");
    answerer.abort();
    host.detach("tui");
    collected
}

fn write_file(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
    std::fs::write(path, content).expect("write file");
}

/// Writes the manifest entry for the fixture server: `enabled` decides
/// `trusted_autostart` (the pack installs' landing shape is `false`).
fn write_declaration(data_dir: &Path, fixture_root: &Path, enabled: bool) {
    let manifest = serde_json::json!({
        "mcpServers": {
            "fs": {
                "command": "in-process-fixture",
                "args": [],
                "roots": [fixture_root.to_string_lossy()],
                "trusted_autostart": enabled,
            }
        }
    });
    write_file(
        &data_dir.join(".austringer").join("mcp.json"),
        &manifest.to_string(),
    );
}

fn test_pool_config() -> PoolConfig {
    PoolConfig {
        max_live: 4,
        warn_at: 3,
        hysteresis_gap: 1,
        // ZERO so a tick right after release drains immediately (the
        // teardown assertion depends on it).
        idle_drain_after: Duration::ZERO,
    }
}

/// One serve-done flag per pool connect (the wire-close observable).
type ServeDone = Arc<Mutex<Vec<Arc<AtomicBool>>>>;

/// The hermetic transport: every pool connect serves one in-process
/// `ReferenceFsServer` over a duplex wire. The serve-done flags are
/// recorded so the teardown test observes the wire closing.
fn fixture_pool(fixture_root: &Path) -> (Arc<TransportPool>, ServeDone) {
    let root = fixture_root.to_path_buf();
    let finished: ServeDone = Arc::new(Mutex::new(Vec::new()));
    let connector: Connector = {
        let root = root.clone();
        let finished = Arc::clone(&finished);
        Arc::new(move |entry: austringer_mcp::config::ServerEntry| {
            let root = root.clone();
            let finished = Arc::clone(&finished);
            Box::pin(async move {
                let (wire, done) = serve_duplex(ReferenceFsServer::new(root.clone())).await;
                lock(&finished).push(Arc::clone(&done));
                McpClient::connect_with_transport(entry, wire).await
            })
        })
    };
    (
        Arc::new(TransportPool::with_connector(test_pool_config(), connector)),
        finished,
    )
}

/// Full setup: fixture fs root with one readable file, enabled
/// declaration, approval recorded against the SCANNED entry's
/// fingerprint (trust binds to the entry as parsed, not as imagined).
async fn mounted_state(
    data_dir: &Path,
    steps: Vec<AssistantTurn>,
    approve: bool,
) -> (
    Arc<DaemonState>,
    Arc<McpMounts>,
    Arc<TransportPool>,
    ServeDone,
) {
    let fixture_root = data_dir.join("fsroot");
    std::fs::create_dir_all(&fixture_root).expect("fixture root");
    write_fixture(&fixture_root, "notes/a.txt", "mounted content");
    write_declaration(data_dir, &fixture_root, true);

    let trust = Arc::new(MemoryTrustStore::new());
    if approve {
        let scan = scan_directories(&[data_dir.to_path_buf()]);
        let installation = scan
            .installations
            .iter()
            .find(|installation| installation.entry.name == "fs")
            .expect("declaration scanned");
        trust.record_approval("fs", installation.entry.fingerprint());
    }

    let (pool, finished) = fixture_pool(&fixture_root);
    let mounts =
        Arc::new(McpMounts::start(data_dir, trust, OrgAllowlist::new(), Arc::clone(&pool)).await);

    let mut state = DaemonState::new(
        DaemonConfig {
            data_dir: data_dir.to_owned(),
            token: Some("mcp-mount-token".into()),
            ask_policy: MediationPolicy::FirstResponder,
            ask_timeout: Duration::from_secs(30),
            ..DaemonConfig::default()
        },
        shared_script(steps),
    )
    .expect("daemon state");
    let executors = Arc::new(CoreExecutors::in_process());
    executors.bind_mcp_mounts(Arc::clone(&mounts));
    state.executor_factory = executors.executor_factory();
    (Arc::new(state), mounts, pool, finished)
}

// ---------------------------------------------------------------------------
// (1) the mount: registry surface + a real dispatch through the engine
// ---------------------------------------------------------------------------

#[tokio::test]
async fn enabled_trusted_server_mounts_and_dispatches_through_the_engine() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (state, mounts, _pool, _finished) = mounted_state(dir.path(), mount_script(), true).await;

    assert_eq!(mounts.server_names(), vec!["fs"], "one server started");
    assert_eq!(mounts.pool_live_count(), 1, "one pooled connection");

    let frames = drive_turn(&state, "s-mcp-mount").await;
    let joined = frames.join("\n");
    assert!(
        joined.contains("hydrated tool schemas (3)"),
        "tool_search must hydrate all three mounted tools:\n{joined}"
    );
    assert!(
        joined.contains("Read one text file under the server root"),
        "the hydrated schema is the fixture server's own description:\n{joined}"
    );
    assert!(
        joined.contains("mounted content"),
        "the mounted read_file must answer from the fixture server:\n{joined}"
    );
    assert!(
        !joined.contains("no executor registered") && !joined.contains("unknown tool"),
        "the mounted tool must dispatch, not miss:\n{joined}"
    );
}

// ---------------------------------------------------------------------------
// (2) refusals: enabled-but-unapproved and approved-but-disabled mount nothing
// ---------------------------------------------------------------------------

#[tokio::test]
async fn enabled_but_unapproved_declaration_mounts_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let steps = vec![
        text_turn(
            "searching",
            vec![call(
                "t0",
                "tool_search",
                serde_json::json!({ "query": "mcp__fs" }),
            )],
        ),
        text_turn(
            "trying anyway",
            vec![call(
                "t1",
                READ_FILE,
                serde_json::json!({ "path": "notes/a.txt" }),
            )],
        ),
        completion_turn(),
    ];
    let (state, mounts, _pool, _finished) = mounted_state(dir.path(), steps, false).await;

    assert!(
        mounts.server_names().is_empty(),
        "no approval = no mount (fail-closed)"
    );
    assert_eq!(mounts.pool_live_count(), 0, "no connection spawned");

    let frames = drive_turn(&state, "s-mcp-unapproved").await;
    let joined = frames.join("\n");
    assert!(
        joined.contains("tool_search miss: no tool matched"),
        "the search must miss for a session with no mounts:\n{joined}"
    );
    assert!(
        // The model's own attempted call echoes "mcp__" in the
        // transcript; the surface assertion is that NOTHING hydrates.
        !joined.contains("hydrated tool schemas"),
        "no mcp tool may hydrate into the session surface:\n{joined}"
    );
    assert!(
        joined.contains("unknown tool \\\"mcp__fs__read_file\\\""),
        "the direct call must get the typed unknown-tool error:\n{joined}"
    );
}

#[tokio::test]
async fn approved_but_disabled_declaration_mounts_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fixture_root = dir.path().join("fsroot");
    std::fs::create_dir_all(&fixture_root).expect("fixture root");
    // The pack installs' landing shape: trusted_autostart forced false.
    write_declaration(dir.path(), &fixture_root, false);

    let trust = Arc::new(MemoryTrustStore::new());
    let scan = scan_directories(&[dir.path().to_path_buf()]);
    let installation = scan
        .installations
        .iter()
        .find(|installation| installation.entry.name == "fs")
        .expect("declaration scanned");
    // Even an APPROVED fingerprint must not mount a disabled marker.
    trust.record_approval("fs", installation.entry.fingerprint());

    let (pool, _finished) = fixture_pool(&fixture_root);
    let mounts = Arc::new(McpMounts::start(dir.path(), trust, OrgAllowlist::new(), pool).await);
    assert!(
        mounts.server_names().is_empty(),
        "the DISABLED marker governs: {:?}",
        mounts.server_names()
    );
    assert_eq!(mounts.pool_live_count(), 0, "no connection spawned");
}

// ---------------------------------------------------------------------------
// (3) lifecycle: eviction releases the session, teardown drains the pool
// ---------------------------------------------------------------------------

#[tokio::test]
async fn eviction_releases_session_mounts_and_teardown_drains_the_pool() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut steps = mount_script();
    steps.extend(mount_script());
    let (state, mounts, pool, finished) = mounted_state(dir.path(), steps, true).await;

    let sid = "s-mcp-evict";
    let first = drive_turn(&state, sid).await;
    assert!(
        first.join("\n").contains("mounted content"),
        "turn one reached the fixture server"
    );
    // A second live session shares the SAME daemon-owned connection.
    let second_host = state.session("s-mcp-evict-2");
    assert_eq!(
        mounts.pool_live_count(),
        1,
        "sessions share one pooled connection"
    );
    drop(second_host);

    tokio::time::sleep(Duration::from_millis(10)).await;
    let mut evicted = austringer_daemon::eviction::run_once(&state, Duration::from_millis(1));
    evicted.sort();
    assert_eq!(
        evicted,
        vec![sid.to_owned(), "s-mcp-evict-2".to_owned()],
        "both idle hosts swept"
    );
    assert!(state.try_session(sid).is_none(), "the map entries are gone");
    // The daemon-owned connection SURVIVES eviction (other sessions
    // may still mount) — it is tracked, not orphaned.
    assert_eq!(mounts.pool_live_count(), 1);

    // The reopened session remounts from the same connection.
    let second = drive_turn(&state, sid).await;
    assert!(
        second.join("\n").contains("mounted content"),
        "the reopened session still reaches the fixture server"
    );

    // Teardown: close the reopened host's lanes (plain Arc drop never
    // closes them — eviction does), then drop every daemon holder
    // (state → executors → mounts → pool handle). The idle drain (ZERO
    // window) reclaims the pooled connection and the fixture serve
    // tasks observe the wire closing — no client handle outlives the
    // daemon.
    tokio::time::sleep(Duration::from_millis(10)).await;
    let re_evicted = austringer_daemon::eviction::run_once(&state, Duration::from_millis(1));
    assert_eq!(re_evicted, vec![sid.to_owned()], "reopen swept too");
    let flags = lock(&finished).clone();
    drop(state);
    drop(mounts);
    let drained = pool.tick();
    assert_eq!(pool.live_count(), 0, "pool drained after teardown");
    assert!(
        drained
            .iter()
            .any(|event| matches!(event, PoolEvent::Drained { .. })),
        "drain is an observable pool event: {drained:?}"
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if flags.iter().all(|flag| flag.load(Ordering::SeqCst)) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("fixture serve tasks never finished — the wire stayed open");
}
