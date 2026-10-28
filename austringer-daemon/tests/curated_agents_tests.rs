//! Curated cheap-pool read-only agents (todo 14, ecosystem
//! alignment): `task(explore)` / `task(librarian)` dispatch on their
//! routing.json ROLES entries — utility roles, so the table's
//! cheap-pool validation (fail-closed at build) pins their models —
//! and the children run under READ-ONLY tool allowlists enforced at
//! the approval stage: a write-tool call inside a curated agent is
//! refused BEFORE any ask fires and logged as a `readonly_refusal`
//! notification. The curated names are reserved: a user category
//! cannot shadow them (the cheap-pool + read-only guarantees are the
//! point).

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;

use austringer_core::agent_loop::inference::{
    InferenceError, InferenceRequest, PendingCall, TurnUsage,
};
use austringer_core::agent_loop::{AssistantTurn, CancelToken, Inference};
use austringer_core::event::{ContentBlock, Event};
use austringer_core::tools::{IoCap, ProviderError, ToolProvider, ToolRegistry};

use austringer_daemon::attach::SessionHost;
use austringer_daemon::config::DaemonConfig;
use austringer_daemon::launch::LaunchServices;
use austringer_daemon::protocol::SessionEvent;
use austringer_daemon::routing::{ModelSource, RoutingIntake};
use austringer_daemon::server::DaemonState;
use austringer_daemon::task::{TaskRefusal, TaskRequest, TaskServices, TaskTool};
use austringer_daemon::tools::{ExecutorFactory, RegistryToolExecutor};
use austringer_daemon::usage::{ComposedInference, InferenceFactory};

use austringer_intel::catalog::Catalog;
use austringer_intel::routing::RoutingTable;
use austringer_providers::identity::ModelIdentity;

/// routing.json with the curated roles on the cheap pool plus —
/// deliberately — a user category ALSO named `explore` pinned to a
/// frontier model: the reserved-name probe (the curated role must
/// win; the category cannot smuggle the agent off the cheap pool).
const CURATED_ROUTING: &str = r#"
{
  "provider": "anthropic",
  "cheap_pool": ["claude-sonnet-4-6", "claude-haiku-5-5"],
  "roles": [
    { "role": "default",   "model": "claude-opus-4-7" },
    { "role": "explore",   "model": "claude-sonnet-4-6", "effort": "low" },
    { "role": "librarian", "model": "claude-haiku-5-5", "effort": "low" }
  ],
  "categories": [
    { "category": "quick",    "model": "claude-sonnet-4-6", "score_floor": 0.5 },
    { "category": "explore",  "model": "claude-opus-4-7", "score_floor": 0.5 }
  ],
  "rate_limit_wait_out_secs": 1,
  "retry_attempts": 1
}
"#;

/// routing.json with NO curated role routes: the unrouted-curated
/// refusal probe.
const UNROUTED_CURATED: &str = r#"
{
  "provider": "anthropic",
  "cheap_pool": ["claude-sonnet-4-6"],
  "roles": [ { "role": "default", "model": "claude-opus-4-7" } ],
  "categories": [
    { "category": "quick", "model": "claude-sonnet-4-6", "score_floor": 0.5 }
  ],
  "rate_limit_wait_out_secs": 1,
  "retry_attempts": 1
}
"#;

fn usage_turn(text: &str) -> AssistantTurn {
    AssistantTurn {
        blocks: vec![ContentBlock::Text {
            text: text.to_owned(),
        }],
        tool_calls: Vec::new(),
        usage: Some(TurnUsage {
            input_tokens: 20,
            output_tokens: 5,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        }),
    }
}

/// One scripted step of a model's stack.
#[derive(Clone)]
enum Step {
    Turn(AssistantTurn),
}

/// What the fake provider seam observed: stack LOOKUPS (dispatch
/// decisions) and INFER calls (who actually served).
#[derive(Default)]
struct RouterLog {
    lookups: Vec<String>,
    infer_calls: Vec<String>,
}

struct FakeModels {
    log: Arc<Mutex<RouterLog>>,
    /// One SHARED queue per model (curated children re-look-up their
    /// role model per child).
    scripts: Mutex<HashMap<String, Arc<Mutex<VecDeque<Step>>>>>,
}

impl FakeModels {
    fn new(log: Arc<Mutex<RouterLog>>, scripts: Vec<(&str, Vec<Step>)>) -> Arc<Self> {
        Arc::new(Self {
            log,
            scripts: Mutex::new(
                scripts
                    .into_iter()
                    .map(|(model, steps)| {
                        (
                            model.to_owned(),
                            Arc::new(Mutex::new(VecDeque::from(steps))),
                        )
                    })
                    .collect(),
            ),
        })
    }

    fn source(self: &Arc<Self>) -> ModelSource {
        let fake = Arc::clone(self);
        Arc::new(move |model: &str| {
            {
                let mut log = fake.log.lock().expect("log");
                log.lookups.push(model.to_owned());
            }
            let scripts = fake.scripts.lock().expect("scripts");
            let queue = scripts.get(model)?.clone();
            Some(Box::new(ModelScript {
                model: model.to_owned(),
                queue,
                log: Arc::clone(&fake.log),
            }) as Box<dyn Inference>)
        })
    }
}

struct ModelScript {
    model: String,
    queue: Arc<Mutex<VecDeque<Step>>>,
    log: Arc<Mutex<RouterLog>>,
}

#[async_trait]
impl Inference for ModelScript {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        {
            let mut log = self.log.lock().expect("log");
            log.infer_calls.push(self.model.clone());
        }
        let mut queue = self.queue.lock().expect("queue");
        match queue.pop_front() {
            Some(Step::Turn(turn)) => Ok(turn),
            None => Err(InferenceError::Failure("script exhausted".to_owned())),
        }
    }
}

struct NeverServes;

#[async_trait]
impl Inference for NeverServes {
    async fn infer(
        &mut self,
        _request: &InferenceRequest<'_>,
        _cancel: &CancelToken,
    ) -> Result<AssistantTurn, InferenceError> {
        Err(InferenceError::Failure(
            "factory stack must never serve on a routed path".to_owned(),
        ))
    }
}

/// An executor provider that records the fact it RAN (the write probe:
/// a curated agent's allowlist must refuse the call before this ever
/// fires).
struct RecordingEdit {
    fired: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl ToolProvider for RecordingEdit {
    fn schema(&self) -> austringer_core::tools::ToolSchema {
        austringer_core::tools::ToolSchema {
            name: "edit_lines".into(),
            description: "recording edit probe".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }),
        }
    }

    async fn execute(&self, arguments: &serde_json::Value) -> Result<Vec<u8>, ProviderError> {
        self.fired
            .lock()
            .expect("fired")
            .push(arguments.to_string());
        Ok(b"edited".to_vec())
    }
}

/// The curated-agent test kit: a routed DaemonState plus TaskServices
/// on the todo-13 child-session pattern (per-session registries with
/// the task tool and, optionally, the recording write probe).
struct CuratedKit {
    state: Arc<DaemonState>,
    services: Arc<TaskServices>,
    log: Arc<Mutex<RouterLog>>,
    edits_fired: Arc<Mutex<Vec<String>>>,
    _dir: tempfile::TempDir,
}

fn curated_kit(
    routing: &str,
    child_ask_timeout: Duration,
    scripts: Vec<(&str, Vec<Step>)>,
    bind_recording_edit: bool,
) -> CuratedKit {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = DaemonConfig {
        data_dir: dir.path().to_path_buf(),
        ..DaemonConfig::default()
    };
    let log = Arc::new(Mutex::new(RouterLog::default()));
    let models = FakeModels::new(Arc::clone(&log), scripts);
    let table = RoutingTable::from_json(routing, &Catalog::seeded())
        .unwrap_or_else(|e| panic!("routing doc rejected: {e:?}"));
    let intake = Arc::new(RoutingIntake {
        table,
        models: models.source(),
    });
    let factory: InferenceFactory = Arc::new(|| ComposedInference {
        inference: Box::new(NeverServes),
        identity: Some(ModelIdentity::new("xai", "grok-4-6")),
    });
    let slot: Arc<std::sync::OnceLock<Arc<TaskServices>>> = Arc::new(std::sync::OnceLock::new());
    let edits_fired: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let executor_factory: ExecutorFactory = {
        let slot = Arc::clone(&slot);
        let edits_fired = Arc::clone(&edits_fired);
        let bind_edit = bind_recording_edit;
        Arc::new(move |session_dir: &Path, _sink| {
            let mut registry = ToolRegistry::open(session_dir, IoCap::DEFAULT)
                .unwrap_or_else(|e| panic!("registry: {e}"));
            if bind_edit {
                registry
                    .bind_executor(
                        "edit_lines",
                        Arc::new(RecordingEdit {
                            fired: Arc::clone(&edits_fired),
                        }),
                    )
                    .expect("bind edit_lines");
            }
            if let Some(services) = slot.get() {
                registry
                    .register(Arc::new(TaskTool::new(session_dir, Arc::clone(services))))
                    .expect("register task tool");
            }
            let tools = {
                let pinned = registry.pin_current();
                registry.pinned_wire_tools(&pinned)
            };
            (Box::new(RegistryToolExecutor::open(registry)), tools)
        })
    };
    let mut state = DaemonState::new(config, factory).expect("daemon state");
    state.routing = Some(Arc::clone(&intake));
    state.executor_factory = executor_factory.clone();
    let state = Arc::new(state);
    let launch = Arc::new(LaunchServices {
        inference_factory: state.inference_factory.clone(),
        executor_factory,
        sessions_root: dir.path().join("sessions"),
        ask_policy: state.config.ask_policy.clone(),
        ask_timeout: child_ask_timeout,
    });
    let resolver_state = Arc::clone(&state);
    let services = Arc::new(TaskServices::new(
        intake,
        launch,
        Arc::new(move |sid: &str| resolver_state.try_session(sid)),
    ));
    assert!(slot.set(Arc::clone(&services)).is_ok());
    CuratedKit {
        state,
        services,
        log,
        edits_fired,
        _dir: dir,
    }
}

fn curated_request(category: &str, prompt: &str) -> TaskRequest {
    TaskRequest {
        category: category.to_owned(),
        prompt: prompt.to_owned(),
        escalate_fan_out: false,
    }
}

/// `expect_err` for results whose success type is not `Debug`
/// (`TaskChildOutcome` carries a live session host).
fn expect_refusal(
    result: Result<austringer_daemon::task::TaskChildOutcome, TaskRefusal>,
) -> TaskRefusal {
    match result {
        Err(refusal) => refusal,
        Ok(_) => panic!("expected a typed refusal"),
    }
}

/// Frames of one host's hub snapshot, event payloads only.
fn hub_events(host: &SessionHost) -> Vec<SessionEvent> {
    host.snapshot()
        .into_iter()
        .map(|frame| frame.event)
        .collect()
}

/// (child, task, role, model) rows of every ChildStarted on a host.
fn started_rows(host: &SessionHost) -> Vec<(String, String, String, String)> {
    hub_events(host)
        .into_iter()
        .filter_map(|event| match event {
            SessionEvent::ChildStarted {
                child,
                task,
                role,
                model,
            } => Some((child, task, role, model)),
            _ => None,
        })
        .collect()
}

/// Concatenated text blocks of every ToolResult in a host transcript.
fn tool_result_texts(host: &SessionHost) -> Vec<String> {
    hub_events(host)
        .into_iter()
        .filter_map(|event| match event {
            SessionEvent::Recorded {
                event: Event::ToolResult { blocks, .. },
                ..
            } => Some(
                blocks
                    .into_iter()
                    .map(|block| match block {
                        ContentBlock::Text { text } => text,
                        other => other.text_marker(),
                    })
                    .collect::<String>(),
            ),
            _ => None,
        })
        .collect()
}

// ------------------------------------------------------------------
// happy: explore routes on the cheap-pool ROLE route, grep-shaped
// summary back
// ------------------------------------------------------------------

#[tokio::test]
async fn curated_explore_routes_via_cheap_pool_role_and_returns_grep_shaped_summary() {
    // Given: explore routed on claude-sonnet-4-6 (cheap pool) at
    // effort low, one child script answering grep-shaped findings.
    let kit = curated_kit(
        CURATED_ROUTING,
        Duration::from_secs(30),
        vec![(
            "claude-sonnet-4-6",
            vec![Step::Turn(usage_turn(
                "grep 'RoleRegistry' src/: role.rs 3 matches, resolve.rs 1 match",
            ))],
        )],
        false,
    );
    let parent = kit.state.session("parent-happy");
    // When: one task(explore) child runs.
    let outcome = kit
        .services
        .run_child(
            &parent,
            curated_request("explore", "where is RoleRegistry used?"),
        )
        .await
        .expect("curated child runs");
    // Then: the child dispatched under the ROLE route — the cheap-pool
    // model composed and served, at the role's effort.
    assert_eq!(outcome.child_id, "parent-happy-child-task1");
    assert!(outcome.succeeded());
    assert_eq!(
        outcome.summary,
        "grep 'RoleRegistry' src/: role.rs 3 matches, resolve.rs 1 match"
    );
    assert_eq!(
        outcome.usage,
        vec![(
            "anthropic".to_owned(),
            "claude-sonnet-4-6".to_owned(),
            Some("low".to_owned())
        )]
    );
    let log = kit.log.lock().expect("log");
    assert_eq!(log.lookups, vec!["claude-sonnet-4-6".to_owned()]);
    drop(log);
    // And: the parent's ChildStarted carries the curated role label
    // and the role-routed model.
    assert_eq!(
        started_rows(&parent),
        vec![(
            "parent-happy-child-task1".to_owned(),
            "explore".to_owned(),
            "explore".to_owned(),
            "claude-sonnet-4-6".to_owned()
        )]
    );
}

// ------------------------------------------------------------------
// failure: a write tool call inside explore is refused + logged
// ------------------------------------------------------------------

#[tokio::test]
async fn curated_explore_write_tool_call_is_refused_and_logged() {
    // Given: an explore child whose model attempts a WRITE mid-turn;
    // the recording edit probe is bound so a missed refusal fires it.
    let kit = curated_kit(
        CURATED_ROUTING,
        Duration::from_millis(150),
        vec![(
            "claude-sonnet-4-6",
            vec![
                Step::Turn(AssistantTurn {
                    blocks: Vec::new(),
                    tool_calls: vec![PendingCall {
                        call_id: "w1".into(),
                        tool: "edit_lines".into(),
                        arguments: serde_json::json!({"path": "src/lib.rs"}),
                    }],
                    usage: None,
                }),
                Step::Turn(usage_turn("write refused; 2 grep hits recorded")),
            ],
        )],
        true,
    );
    let parent = kit.state.session("parent-write");
    // When: the explore child attempts edit_lines.
    let outcome = kit
        .services
        .run_child(
            &parent,
            curated_request("explore", "search, then try to edit"),
        )
        .await
        .expect("refusal is a tool outcome, not a spawn failure");
    // Then: the write NEVER executed (allowlist refused it before the
    // executor — and before any permission ask could time out into a
    // different denial).
    assert!(
        kit.edits_fired.lock().expect("fired").is_empty(),
        "curated agents must never execute write tools"
    );
    // And: the refusal reached the model as tool-result text naming
    // the read-only contract…
    let results = tool_result_texts(&outcome.child);
    assert!(
        results
            .iter()
            .any(|text| text.contains("edit_lines") && text.contains("read-only agent `explore`")),
        "refusal text: {results:?}"
    );
    // …and was LOGGED on the child stream as a typed notification.
    let notifications = hub_events(&outcome.child)
        .into_iter()
        .filter_map(|event| match event {
            SessionEvent::Notification { kind, message } => Some((kind, message)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        notifications.iter().any(|(kind, message)| {
            kind == "readonly_refusal" && message.contains("edit_lines")
        }),
        "readonly_refusal notification: {notifications:?}"
    );
    // And: the child turn still completed (deny is not a failure).
    assert!(outcome.succeeded());
    assert_eq!(outcome.summary, "write refused; 2 grep hits recorded");
}

// ------------------------------------------------------------------
// reserved names: a user category cannot shadow the curated agents
// ------------------------------------------------------------------

#[tokio::test]
async fn curated_names_are_reserved_over_user_categories() {
    // Given: CURATED_ROUTING deliberately declares a user category
    // ALSO named `explore` pinned to a FRONTIER model (opus) while the
    // explore ROLE stays on the cheap pool (sonnet).
    let kit = curated_kit(
        CURATED_ROUTING,
        Duration::from_secs(30),
        vec![(
            "claude-sonnet-4-6",
            vec![Step::Turn(usage_turn("reserved-name probe: 1 match"))],
        )],
        false,
    );
    let parent = kit.state.session("parent-reserved");
    // When: task(explore) runs.
    let outcome = kit
        .services
        .run_child(&parent, curated_request("explore", "probe"))
        .await
        .expect("child runs");
    // Then: the CURATED role route won — the cheap-pool model served,
    // the frontier category model never composed a stack.
    assert_eq!(outcome.usage.len(), 1);
    assert_eq!(outcome.usage[0].1, "claude-sonnet-4-6");
    let log = kit.log.lock().expect("log");
    assert_eq!(log.lookups, vec!["claude-sonnet-4-6".to_owned()]);
    drop(log);
    assert_eq!(
        started_rows(&parent)[0].2,
        "explore".to_owned(),
        "role label is the curated agent, not the category"
    );
}

// ------------------------------------------------------------------
// unrouted curated agent: typed refusal naming the missing route
// ------------------------------------------------------------------

#[tokio::test]
async fn curated_agent_without_role_route_is_typed_refusal() {
    // Given: routing WITHOUT a roles[] entry for explore (the curated
    // dispatch needs one; fail closed, never guess a model).
    let kit = curated_kit(UNROUTED_CURATED, Duration::from_secs(30), Vec::new(), false);
    let parent = kit.state.session("parent-unrouted");
    // When: task(explore) is requested at the services level.
    let refusal = expect_refusal(
        kit.services
            .run_child(&parent, curated_request("explore", "x"))
            .await,
    );
    // Then: the typed UnconfiguredRole refusal (the curated agent
    // routes on the ROLE table, not the category table).
    match &refusal {
        TaskRefusal::Category(austringer_intel::routing::ResolutionError::UnconfiguredRole {
            role,
        }) => {
            assert_eq!(*role, austringer_intel::routing::Role::Explore);
        }
        other => panic!("expected UnconfiguredRole, got {other:?}"),
    }
    // And: at the tool level the model sees the typed refusal text
    // naming the missing route.
    let dir = kit._dir.path().join("sessions").join("parent-unrouted");
    std::fs::create_dir_all(&dir).expect("session dir");
    let tool = TaskTool::new(&dir, Arc::clone(&kit.services));
    let result = tool
        .execute(&serde_json::json!({"category": "explore", "prompt": "x"}))
        .await;
    match result {
        Err(ProviderError::Execution { message }) => {
            // "role Explore has no route configured — add it before
            // dispatching" (the enum's Debug name rides the message).
            let message = message.to_lowercase();
            assert!(
                message.contains("explore") && message.contains("no route configured"),
                "refusal: {message}"
            );
        }
        other => panic!("expected typed execution error, got {other:?}"),
    }
    // And: nothing ever spawned.
    let log = kit.log.lock().expect("log");
    assert!(log.lookups.is_empty());
}

// ------------------------------------------------------------------
// the tool result: curated summaries clamp at the agent's own budget
// ------------------------------------------------------------------

#[tokio::test]
async fn curated_summary_clamps_at_the_curated_agent_budget() {
    // Given: an explore child whose final text is far over the
    // curated agent's 1024-byte budget.
    let kit = curated_kit(
        CURATED_ROUTING,
        Duration::from_secs(30),
        vec![(
            "claude-sonnet-4-6",
            vec![Step::Turn(usage_turn(&"x".repeat(3000)))],
        )],
        false,
    );
    kit.state.session("parent-clamp");
    let dir = kit._dir.path().join("sessions").join("parent-clamp");
    std::fs::create_dir_all(&dir).expect("session dir");
    let tool = TaskTool::new(&dir, Arc::clone(&kit.services));
    // When: the model calls task(explore, ...).
    let bytes = tool
        .execute(&serde_json::json!({"category": "explore", "prompt": "long"}))
        .await
        .expect("clamped result");
    let text = String::from_utf8(bytes).expect("utf8");
    // Then: labeled with the curated category…
    assert!(
        text.starts_with("[task child parent-clamp-child-task1 category=explore]"),
        "label: {text}"
    );
    // …and clamped at the CURATED budget (1024), not the worker's
    // 2048.
    let label_len = text.find("] ").map(|end| end + 2).expect("label");
    assert!(
        text.len() - label_len <= 1024,
        "curated clamp: {} bytes past the label",
        text.len() - label_len
    );
    assert!(text.ends_with(" [truncated]"), "clamp marker present");
}

// ------------------------------------------------------------------
// librarian: the second curated agent rides the same path
// ------------------------------------------------------------------

#[tokio::test]
async fn curated_librarian_routes_on_its_own_cheap_pool_role() {
    // Given: librarian routed on claude-haiku-5-5 (cheap pool).
    let kit = curated_kit(
        CURATED_ROUTING,
        Duration::from_secs(30),
        vec![(
            "claude-haiku-5-5",
            vec![Step::Turn(usage_turn(
                "docs: 2 OSS receipts found (research §20, §27)",
            ))],
        )],
        false,
    );
    let parent = kit.state.session("parent-lib");
    // When: one task(librarian) child runs.
    let outcome = kit
        .services
        .run_child(&parent, curated_request("librarian", "find OSS receipts"))
        .await
        .expect("librarian child runs");
    // Then: its own role route served, with the librarian label.
    assert!(outcome.succeeded());
    assert_eq!(outcome.usage[0].1, "claude-haiku-5-5");
    assert_eq!(
        started_rows(&parent)[0],
        (
            "parent-lib-child-task1".to_owned(),
            "librarian".to_owned(),
            "librarian".to_owned(),
            "claude-haiku-5-5".to_owned()
        )
    );
}
