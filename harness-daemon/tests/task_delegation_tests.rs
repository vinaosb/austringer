//! `task(category)` delegation acceptance (plan todo 13, ecosystem
//! alignment): the model-facing delegation tool and its spawn path —
//! category routing through the todo-9 Dispatcher, the task-8
//! child-session launch pattern, fan-out caps with explicit escalation
//! (T10/A6 subagent-storm guard), the fork.rs DepthGuard on the
//! task-tool path, FRESH grant state on every spawn, per-parent-session
//! budget aggregation across task() calls, and the ask-first fallback
//! contract (a chain hop mid-task is a FallbackAsk — never a silent
//! substitution).

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::broadcast;

use harness_core::agent_loop::inference::{
    InferenceError, InferenceRequest, PendingCall, TurnUsage,
};
use harness_core::agent_loop::{AssistantTurn, CancelToken, Inference};
use harness_core::event::{ContentBlock, Event};
use harness_core::inspectors::store::AskRequest;
use harness_core::tools::{IoCap, ProviderError, ToolProvider, ToolRegistry};

use harness_daemon::ask::{Answer, Resolution};
use harness_daemon::attach::{HostCommand, SessionHost};
use harness_daemon::config::DaemonConfig;
use harness_daemon::launch::LaunchServices;
use harness_daemon::protocol::{ServerMsg, SessionEvent};
use harness_daemon::routing::{ModelSource, RoutingIntake};
use harness_daemon::server::DaemonState;
use harness_daemon::task::{
    DEFAULT_FAN_OUT_CAP, DelegationBudget, TaskRefusal, TaskRequest, TaskServices, TaskTool,
};
use harness_daemon::tools::{ExecutorFactory, RegistryToolExecutor};
use harness_daemon::usage::{ComposedInference, InferenceFactory};

use harness_intel::catalog::Catalog;
use harness_intel::routing::RoutingTable;
use harness_orchestration::budget::BudgetBreach;
use harness_providers::identity::ModelIdentity;

/// routing.json with two delegation categories (`quick` on sonnet at
/// explicit effort low, `deep` on opus at its catalog default) plus the
/// verbatim chains the ask-first scenario proposes from.
const TASK_ROUTING: &str = r#"
{
  "provider": "anthropic",
  "cheap_pool": ["claude-sonnet-4-6"],
  "roles": [{ "role": "default", "model": "claude-opus-4-7" }],
  "categories": [
    { "category": "quick", "model": "claude-sonnet-4-6", "effort": "low", "score_floor": 0.5 },
    { "category": "deep",  "model": "claude-opus-4-7", "score_floor": 0.5 }
  ],
  "fallbacks": [
    { "model": "claude-sonnet-4-6", "chain": ["gpt-5-5"] },
    { "model": "claude-opus-4-7", "chain": ["gemini-3-pro"] }
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

fn outage_step() -> Step {
    Step::Fail("provider unreachable: connection refused".to_owned())
}

/// One scripted step of a model's stack.
#[derive(Clone)]
enum Step {
    Turn(AssistantTurn),
    Fail(String),
}

/// What the fake provider seam observed: stack LOOKUPS (dispatch
/// decisions) and INFER calls (who actually served) — the pair is the
/// zero-substitution proof.
#[derive(Default)]
struct RouterLog {
    lookups: Vec<String>,
    infer_calls: Vec<String>,
}

struct FakeModels {
    log: Arc<Mutex<RouterLog>>,
    /// One SHARED queue per model: every stack lookup of the same model
    /// pops from it (task children re-look-up their category model per
    /// child, unlike single-session dispatcher tests).
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
            Some(Step::Fail(message)) => Err(InferenceError::Failure(message)),
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

/// An executor provider that records the fact it RAN (the fresh-grants
/// probe: an inherited grant would let it fire inside a child).
struct RecordingEdit {
    fired: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl ToolProvider for RecordingEdit {
    fn schema(&self) -> harness_core::tools::ToolSchema {
        harness_core::tools::ToolSchema {
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

/// The task-delegation test kit: a routed DaemonState plus TaskServices
/// whose child executor factory opens per-session registries carrying
/// the `task` tool (the production CoreExecutors binding, minus the
/// heavy in-process executors) and, optionally, the recording edit
/// probe. Construction order breaks the services<->factory cycle the
/// same way the binary does: the factory reads the services through a
/// set-once slot AFTER TaskServices exists.
struct TaskKit {
    state: Arc<DaemonState>,
    services: Arc<TaskServices>,
    log: Arc<Mutex<RouterLog>>,
    edits_fired: Arc<Mutex<Vec<String>>>,
    _dir: tempfile::TempDir,
}

fn task_kit(
    child_ask_timeout: Duration,
    scripts: Vec<(&str, Vec<Step>)>,
    bind_recording_edit: bool,
) -> TaskKit {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = DaemonConfig {
        data_dir: dir.path().to_path_buf(),
        ..DaemonConfig::default()
    };
    let log = Arc::new(Mutex::new(RouterLog::default()));
    let models = FakeModels::new(Arc::clone(&log), scripts);
    let table = RoutingTable::from_json(TASK_ROUTING, &Catalog::seeded())
        .unwrap_or_else(|e| panic!("routing doc rejected: {e:?}"));
    let intake = Arc::new(RoutingIntake {
        table,
        models: models.source(),
    });
    let factory: InferenceFactory = Arc::new(|| ComposedInference {
        inference: Box::new(NeverServes),
        identity: Some(ModelIdentity::new("xai", "grok-4-6")),
    });
    // The registry factory: per-session registry with the task tool
    // (resolved through the set-once services slot, so construction
    // order breaks the services<->factory cycle the way the binary
    // does) and the optional recording edit probe. Parent sessions and
    // task children BOTH serve through it — the depth-refusal contract
    // needs the tool present in child registries too.
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
            Box::new(RegistryToolExecutor::open(registry))
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
    TaskKit {
        state,
        services,
        log,
        edits_fired,
        _dir: dir,
    }
}

fn quick_request(prompt: &str) -> TaskRequest {
    TaskRequest {
        category: "quick".to_owned(),
        prompt: prompt.to_owned(),
        escalate_fan_out: false,
    }
}

/// `expect_err` for results whose success type is not `Debug`
/// (`TaskChildOutcome` carries a live session host).
fn expect_refusal(
    result: Result<harness_daemon::task::TaskChildOutcome, TaskRefusal>,
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

fn child_texts(host: &SessionHost) -> Vec<String> {
    hub_events(host)
        .into_iter()
        .filter_map(|event| match event {
            SessionEvent::Recorded {
                event: Event::Assistant { blocks },
                ..
            } => Some(
                blocks
                    .into_iter()
                    .map(|block| match block {
                        ContentBlock::Text { text } => text,
                        other => other.text_marker(),
                    })
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            _ => None,
        })
        .collect()
}

// ------------------------------------------------------------------
// (a) category routing + the labeled summary contract
// ------------------------------------------------------------------

#[tokio::test]
async fn task_child_routes_to_category_model_and_returns_labeled_summary() {
    // Given: the quick category on claude-sonnet-4-6 at effort low, and
    // a one-turn child script for it.
    let kit = task_kit(
        Duration::from_secs(30),
        vec![(
            "claude-sonnet-4-6",
            vec![Step::Turn(usage_turn("quick answer"))],
        )],
        false,
    );
    let parent = kit.state.session("parent-a");
    // When: one task(quick) child runs.
    let outcome = kit
        .services
        .run_child(&parent, quick_request("count the widgets"))
        .await
        .expect("child runs");
    // Then: the child dispatched under the CATEGORY request — the
    // category's model composed and served, at the category's effort.
    assert_eq!(outcome.child_id, "parent-a-child-task1");
    assert_eq!(outcome.summary, "quick answer");
    assert_eq!(outcome.outcome, "turn_complete");
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
    assert_eq!(log.infer_calls, vec!["claude-sonnet-4-6".to_owned()]);
    drop(log);
    // And: the child session received the prompt verbatim.
    assert!(
        child_texts(&outcome.child)
            .iter()
            .any(|text| text.contains("quick answer")),
        "child transcript: {:?}",
        child_texts(&outcome.child)
    );
    let prompt_seen = hub_events(&outcome.child).iter().any(|event| {
        matches!(
            event,
            SessionEvent::Recorded {
                event: Event::User { .. },
                ..
            }
        )
    });
    assert!(prompt_seen, "child recorded its user prompt");
    // And: the parent hub carries the team events (todo-15 contract).
    let started = hub_events(&parent)
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
        .collect::<Vec<_>>();
    assert_eq!(
        started,
        vec![(
            "parent-a-child-task1".to_owned(),
            "quick".to_owned(),
            "worker".to_owned(),
            "claude-sonnet-4-6".to_owned()
        )]
    );
    let finished = hub_events(&parent)
        .into_iter()
        .filter_map(|event| match event {
            SessionEvent::ChildFinished { child, state, .. } => Some((child, state)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        finished,
        vec![("parent-a-child-task1".to_owned(), "success".to_owned())]
    );
}

#[tokio::test]
async fn task_two_parallel_category_children_both_return_summaries() {
    // Given: scripts for BOTH category models (quick -> sonnet, deep ->
    // opus).
    let kit = task_kit(
        Duration::from_secs(30),
        vec![
            (
                "claude-sonnet-4-6",
                vec![Step::Turn(usage_turn("quick says 1"))],
            ),
            (
                "claude-opus-4-7",
                vec![Step::Turn(usage_turn("deep says 2"))],
            ),
        ],
        false,
    );
    let parent = kit.state.session("parent-par");
    let services = Arc::clone(&kit.services);
    let parent_for_deep = Arc::clone(&parent);
    // When: two category tasks run CONCURRENTLY.
    let (quick, deep) = tokio::join!(
        services.run_child(&parent, quick_request("quick work")),
        services.run_child(
            &parent_for_deep,
            TaskRequest {
                category: "deep".to_owned(),
                prompt: "deep work".to_owned(),
                escalate_fan_out: false,
            }
        )
    );
    // Then: both children completed with their own summaries.
    assert_eq!(quick.expect("quick child").summary, "quick says 1");
    assert_eq!(deep.expect("deep child").summary, "deep says 2");
    // And: the parent saw two children start.
    let started = hub_events(&parent)
        .into_iter()
        .filter(|event| matches!(event, SessionEvent::ChildStarted { .. }))
        .count();
    assert_eq!(started, 2);
}

// ------------------------------------------------------------------
// failure: unknown category lists the valid ones
// ------------------------------------------------------------------

#[tokio::test]
async fn task_unknown_category_is_typed_error_listing_valid_categories() {
    let kit = task_kit(Duration::from_secs(30), Vec::new(), false);
    let parent = kit.state.session("parent-unknown");
    // Services level: typed refusal carrying the valid list.
    let error = expect_refusal(
        kit.services
            .run_child(
                &parent,
                TaskRequest {
                    category: "nope".to_owned(),
                    prompt: "x".to_owned(),
                    escalate_fan_out: false,
                },
            )
            .await,
    );
    match &error {
        TaskRefusal::Category(harness_intel::routing::ResolutionError::UnconfiguredCategory {
            valid,
            ..
        }) => {
            assert_eq!(valid, &vec!["deep".to_owned(), "quick".to_owned()]);
        }
        other => panic!("expected UnconfiguredCategory, got {other:?}"),
    }
    // Tool level: the refusal reaches the model as typed tool text
    // naming the valid categories.
    let dir = kit._dir.path().join("sessions").join("parent-unknown");
    std::fs::create_dir_all(&dir).expect("session dir");
    let tool = TaskTool::new(&dir, Arc::clone(&kit.services));
    let result = tool
        .execute(&serde_json::json!({"category": "nope", "prompt": "x"}))
        .await;
    match result {
        Err(ProviderError::Execution { message }) => {
            assert!(message.contains("nope"), "message: {message}");
            assert!(message.contains("quick"), "message lists valid: {message}");
            assert!(message.contains("deep"), "message lists valid: {message}");
        }
        other => panic!("expected typed execution error, got {other:?}"),
    }
    // And: nothing ever spawned.
    let log = kit.log.lock().expect("log");
    assert!(log.lookups.is_empty());
}

// ------------------------------------------------------------------
// (b) fan-out cap, default 4, explicit escalation
// ------------------------------------------------------------------

#[test]
fn delegation_budget_default_cap_is_four_with_typed_escalation_refusal() {
    // The T10/A6 storm guard: hard cap, default 4, refusal names the
    // explicit escalation criteria.
    assert_eq!(DEFAULT_FAN_OUT_CAP, 4);
    let mut budget = DelegationBudget::new(DEFAULT_FAN_OUT_CAP, 1_000_000);
    for expected in 1..=4 {
        let ordinal = budget.authorize(false).expect("within cap");
        assert_eq!(ordinal, expected);
    }
    let refused = budget.authorize(false).expect_err("cap reached");
    match refused {
        TaskRefusal::FanOut(exceeded) => {
            assert_eq!(exceeded.spawned, 4);
            assert_eq!(exceeded.cap, 4);
        }
        other => panic!("expected FanOut, got {other:?}"),
    }
    assert!(
        refused.to_string().contains("escalate_fan_out"),
        "refusal names the escalation criteria: {refused}"
    );
    // Explicit escalation lifts the cap to the absolute ceiling — still
    // a hard cap, never unbounded.
    let escalated = budget.authorize(true).expect("escalated spawn");
    assert_eq!(escalated, 5);
}

#[tokio::test]
async fn task_fan_out_cap_aggregates_across_calls_and_needs_escalation() {
    // Given: a tightened cap (2) so the aggregate is observable without
    // four real spawns; one script slot per allowed spawn.
    let kit = task_kit(
        Duration::from_secs(30),
        vec![(
            "claude-sonnet-4-6",
            vec![
                Step::Turn(usage_turn("one")),
                Step::Turn(usage_turn("two")),
                Step::Turn(usage_turn("three")),
            ],
        )],
        false,
    );
    kit.services.set_fan_out_cap(2);
    let parent = kit.state.session("parent-fan");
    // When: three task() calls run on ONE parent session.
    kit.services
        .run_child(&parent, quick_request("1"))
        .await
        .expect("first");
    kit.services
        .run_child(&parent, quick_request("2"))
        .await
        .expect("second");
    let third = expect_refusal(kit.services.run_child(&parent, quick_request("3")).await);
    // Then: the third is refused with the typed fan-out error naming
    // the cap and the escalation criteria — no child spawned.
    match third {
        TaskRefusal::FanOut(exceeded) => {
            assert_eq!(exceeded.spawned, 2);
            assert_eq!(exceeded.cap, 2);
        }
        other => panic!("expected FanOut, got {other:?}"),
    }
    // And: the SAME call with explicit escalation proceeds.
    let escalated = kit
        .services
        .run_child(
            &parent,
            TaskRequest {
                category: "quick".to_owned(),
                prompt: "3".to_owned(),
                escalate_fan_out: true,
            },
        )
        .await
        .expect("escalated spawn allowed");
    assert_eq!(escalated.summary, "three");
}

// ------------------------------------------------------------------
// (e) token budget aggregates PER PARENT SESSION across task() calls
// ------------------------------------------------------------------

#[tokio::test]
async fn task_token_budget_aggregates_per_parent_session_across_calls() {
    // Given: a 35-token budget and children spending 25 tokens each
    // (usage_turn reports 20 input + 5 output).
    let kit = task_kit(
        Duration::from_secs(30),
        vec![(
            "claude-sonnet-4-6",
            vec![
                Step::Turn(usage_turn("one")),
                Step::Turn(usage_turn("two")),
                Step::Turn(usage_turn("other parent")),
            ],
        )],
        false,
    );
    kit.services.set_token_budget(35);
    let parent_a = kit.state.session("parent-tok-a");
    // When: two task() calls run on the SAME parent session.
    let first = kit
        .services
        .run_child(&parent_a, quick_request("1"))
        .await
        .expect("first spends 25");
    assert_eq!(first.tokens, 25);
    let second = kit
        .services
        .run_child(&parent_a, quick_request("2"))
        .await
        .expect("second authorized under the 35 budget (25 spent)");
    assert_eq!(second.tokens, 25);
    // Then: the aggregate (50 of 35) is exhausted — the NEXT call on
    // this parent refuses typed, while a DIFFERENT parent session
    // (per-parent keying, not a global guard) still spawns.
    let refused = expect_refusal(kit.services.run_child(&parent_a, quick_request("3")).await);
    match refused {
        TaskRefusal::Tokens(BudgetBreach::TokensExhausted { spent, budget }) => {
            assert_eq!(spent, 50);
            assert_eq!(budget, 35);
        }
        other => panic!("expected TokensExhausted, got {other:?}"),
    }
    // And: the aggregate is PER PARENT SESSION, not global — a second
    // parent's ledger starts fresh and still spawns.
    let parent_b = kit.state.session("parent-tok-b");
    let independent = kit
        .services
        .run_child(&parent_b, quick_request("mine"))
        .await
        .expect("per-parent keying keeps other parents spawning");
    assert_eq!(independent.summary, "other parent");
}

// ------------------------------------------------------------------
// (c) a CHILD session invoking task is refused with typed DepthExceeded
// ------------------------------------------------------------------

#[tokio::test]
async fn task_child_session_invoking_task_is_refused_depth_exceeded() {
    // Given: a task tool bound in a CHILD session's registry (children
    // live in the `-child-` namespace; fork.rs DepthGuard default max
    // depth is 1).
    let kit = task_kit(Duration::from_secs(30), Vec::new(), false);
    let dir = kit
        ._dir
        .path()
        .join("sessions")
        .join("parent-c-child-task1");
    std::fs::create_dir_all(&dir).expect("child session dir");
    let tool = TaskTool::new(&dir, Arc::clone(&kit.services));
    // When: the child invokes task.
    let result = tool
        .execute(&serde_json::json!({"category": "quick", "prompt": "nest"}))
        .await;
    // Then: typed DepthExceeded refusal (fork.rs vocabulary), before
    // any spawn.
    match result {
        Err(ProviderError::Execution { message }) => {
            assert!(
                message.contains("subagent depth 2 exceeds max 1"),
                "depth refusal: {message}"
            );
        }
        other => panic!("expected typed depth refusal, got {other:?}"),
    }
    let log = kit.log.lock().expect("log");
    assert!(
        log.lookups.is_empty(),
        "no child may spawn: {:?}",
        log.lookups
    );
}

// ------------------------------------------------------------------
// (d) task-tool spawns carry FRESH grant state
// ------------------------------------------------------------------

#[tokio::test]
async fn task_children_spawn_with_fresh_grant_state() {
    // Given: a parent whose mediator carries an AllowSession answer for
    // edit_lines (the session-grant-shaped state), and a child script
    // whose model calls edit_lines on the same shape.
    let kit = task_kit(
        Duration::from_millis(150),
        vec![(
            "claude-sonnet-4-6",
            vec![
                Step::Turn(AssistantTurn {
                    blocks: Vec::new(),
                    tool_calls: vec![PendingCall {
                        call_id: "e1".into(),
                        tool: "edit_lines".into(),
                        arguments: serde_json::json!({"path": "src/lib.rs"}),
                    }],
                    usage: None,
                }),
                Step::Turn(usage_turn("child saw the denial")),
            ],
        )],
        true,
    );
    let parent = kit.state.session("parent-grant");
    let mediator = parent.mediator();
    let mut control = mediator.subscribe();
    let voting_mediator = Arc::clone(&mediator);
    let answerer = tokio::spawn(async move {
        while let Ok(msg) = control.recv().await {
            if let ServerMsg::PermissionAsk { ask } = msg {
                let _ = voting_mediator.answer(ask.ask_id, "grant-holder", Answer::AllowSession);
                return;
            }
        }
    });
    let resolution = parent
        .mediator()
        .mediate(AskRequest {
            tool: "edit_lines".into(),
            target: Some("src/lib.rs".into()),
            reason: "edit file".into(),
        })
        .await;
    assert!(
        matches!(resolution, Resolution::Allowed { .. }),
        "parent grant answered: {resolution:?}"
    );
    answerer.await.expect("answerer");
    // When: a task child calls edit_lines with NOBODY answering its ask
    // (150ms child ask timeout).
    let outcome = kit
        .services
        .run_child(&parent, quick_request("edit something"))
        .await
        .expect("child runs; denial is not a spawn failure");
    // Then: the child's permission state was FRESH — its own chain
    // asked, timed out, denied: the executor NEVER fired (an inherited
    // AllowSession/GrantAlways(Session) grant would have let it run),
    // and the denial is visible in the child transcript.
    assert!(
        kit.edits_fired.lock().expect("fired").is_empty(),
        "inherited grants must never execute child tools"
    );
    let transcript = hub_events(&outcome.child);
    let denied = transcript.iter().any(|event| match event {
        SessionEvent::Recorded {
            event: Event::ToolResult { blocks, .. },
            ..
        } => blocks.iter().any(|block| matches!(block, ContentBlock::Text { text } if text.contains("denied by approval stage"))),
        _ => false,
    });
    assert!(
        denied,
        "child transcript must show the denial: {transcript:#?}"
    );
    assert_eq!(outcome.summary, "child saw the denial");
}

// ------------------------------------------------------------------
// ask-first: a chain hop mid-task is a FallbackAsk, never substitution
// ------------------------------------------------------------------

#[tokio::test]
async fn task_chain_hop_mid_task_surfaces_fallback_ask_never_substitutes() {
    // Given: the quick model hard-fails every attempt; its verbatim
    // chain proposes gpt-5-5; NOBODY answers the child's asks (150ms).
    let kit = task_kit(
        Duration::from_millis(150),
        vec![
            (
                "claude-sonnet-4-6",
                vec![outage_step(), outage_step(), outage_step()],
            ),
            ("gpt-5-5", vec![Step::Turn(usage_turn("never"))]),
        ],
        false,
    );
    let parent = kit.state.session("parent-ask");
    // When: one task child runs through the failures.
    let outcome = kit
        .services
        .run_child(&parent, quick_request("work"))
        .await
        .expect("child turn surfaces the failure typed");
    // Then: the child FAILED (no silent success)…
    assert!(
        outcome.outcome.starts_with("failed"),
        "outcome: {}",
        outcome.outcome
    );
    // …the fallback ASK fired on the child's stream (the ask/resolved
    // notification pair the mediator publishes)…
    let notifications = hub_events(&outcome.child)
        .into_iter()
        .filter_map(|event| match event {
            SessionEvent::Notification { kind, message } => Some((kind, message)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        notifications.iter().any(|(kind, message)| {
            kind == "fallback"
                && message.contains("model fallback proposed")
                && message.contains("claude-sonnet-4-6 -> gpt-5-5")
        }),
        "ask must surface: {notifications:?}"
    );
    assert!(
        notifications
            .iter()
            .any(|(_, message)| message.contains("retrying same model")),
        "unacknowledged ask retries the SAME model: {notifications:?}"
    );
    // …and ZERO substitution: the hop model never composed a stack, so
    // only the origin ever served.
    let log = kit.log.lock().expect("log");
    assert_eq!(log.lookups, vec!["claude-sonnet-4-6".to_owned()]);
    assert_eq!(
        log.infer_calls,
        vec![
            "claude-sonnet-4-6".to_owned(),
            "claude-sonnet-4-6".to_owned()
        ]
    );
    drop(log);
    // And: the parent's team events carry the failed child.
    let finished = hub_events(&parent)
        .into_iter()
        .filter_map(|event| match event {
            SessionEvent::ChildFinished { child, state, .. } => Some((child, state)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        finished,
        vec![("parent-ask-child-task1".to_owned(), "failed".to_owned())]
    );
}

// ------------------------------------------------------------------
// the tool result: labeled child output, clamped at the role budget
// ------------------------------------------------------------------

#[tokio::test]
async fn task_tool_result_is_labeled_child_output_clamped_at_summary_budget() {
    // Given: a child whose final assistant text is far over the
    // delegation role's summary budget (worker: 2048 bytes).
    let kit = task_kit(
        Duration::from_secs(30),
        vec![(
            "claude-sonnet-4-6",
            vec![Step::Turn(usage_turn(&"x".repeat(3000)))],
        )],
        false,
    );
    // The parent host must exist for the tool's resolver to find it.
    kit.state.session("parent-clamp");
    let dir = kit._dir.path().join("sessions").join("parent-clamp");
    std::fs::create_dir_all(&dir).expect("session dir");
    let tool = TaskTool::new(&dir, Arc::clone(&kit.services));
    // When: the model calls task(quick, ...).
    let bytes = tool
        .execute(&serde_json::json!({"category": "quick", "prompt": "long"}))
        .await
        .expect("clamped result");
    let text = String::from_utf8(bytes).expect("utf8");
    // Then: the result is LABELED as child output, naming the child and
    // the category…
    assert!(
        text.starts_with("[task child parent-clamp-child-task1 category=quick]"),
        "label: {text}"
    );
    // …and clamped through clamp_summary at the role's summary budget —
    // never a bare truncation (worker role budget: 2048 bytes).
    assert!(
        text.ends_with(" [truncated]"),
        "clamp marker: {}…",
        &text[text.len().saturating_sub(40)..]
    );
    let label_len = text.find("] ").map(|end| end + 2).expect("label");
    assert!(
        text.len() - label_len <= 2048,
        "clamped to the role budget: {} bytes past the label",
        text.len() - label_len
    );
}

// ------------------------------------------------------------------
// todo 15: budget exhaustion halts further spawns with a typed event
// ------------------------------------------------------------------

#[tokio::test]
async fn budget_exhaustion_halts_further_spawns_with_typed_event() {
    // Given: a 25-token budget and one-turn children spending exactly
    // 25 tokens each (usage_turn reports 20 input + 5 output).
    let kit = task_kit(
        Duration::from_secs(30),
        vec![(
            "claude-sonnet-4-6",
            vec![
                Step::Turn(usage_turn("one")),
                Step::Turn(usage_turn("never")),
            ],
        )],
        false,
    );
    kit.services.set_token_budget(25);
    let parent = kit.state.session("parent-budget");
    // When: the first child spends the whole budget.
    kit.services
        .run_child(&parent, quick_request("1"))
        .await
        .expect("first spends 25");
    // Then: the parent hub carries the budget surfacing — one event
    // for the spawn authorization, one after the spend records.
    let budget_events = hub_events(&parent)
        .iter()
        .filter(|event| matches!(event, SessionEvent::TeamBudget { .. }))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        budget_events.len(),
        2,
        "spawn + spend budget events: {budget_events:?}"
    );
    match &budget_events[1] {
        SessionEvent::TeamBudget {
            spawned,
            tokens_spent,
            fan_out_cap,
            token_budget,
        } => {
            assert_eq!(
                (*spawned, *tokens_spent, *fan_out_cap, *token_budget),
                (1, 25, 4, 25)
            );
        }
        other => panic!("expected TeamBudget, got {other:?}"),
    }
    // And: the NEXT spawn on this parent refuses with the typed
    // exhaustion EVENT on the hub (visible to every attached client).
    let refused = expect_refusal(kit.services.run_child(&parent, quick_request("2")).await);
    match &refused {
        TaskRefusal::Tokens(BudgetBreach::TokensExhausted { spent, budget }) => {
            assert_eq!((*spent, *budget), (25, 25));
        }
        other => panic!("expected TokensExhausted, got {other:?}"),
    }
    let exhausted = hub_events(&parent).iter().any(|event| {
        matches!(
            event,
            SessionEvent::TeamBudgetExhausted {
                spent: 25,
                budget: 25
            }
        )
    });
    assert!(
        exhausted,
        "typed exhaustion event on the hub: {:?}",
        hub_events(&parent)
    );
    // And: the halt is real — exactly one child ever started.
    let started = hub_events(&parent)
        .iter()
        .filter(|event| matches!(event, SessionEvent::ChildStarted { .. }))
        .count();
    assert_eq!(started, 1, "budget exhaustion halts further spawns");
}

// ------------------------------------------------------------------
// todo 15: a blocked child surfaces its ask on the parent stream
// ------------------------------------------------------------------

#[tokio::test]
async fn blocked_child_surfaces_its_ask_on_the_parent_stream() {
    // Given: a child whose model calls edit_lines mid-turn with NOBODY
    // answering the child's ask (150ms timeout), then completes.
    let kit = task_kit(
        Duration::from_millis(150),
        vec![(
            "claude-sonnet-4-6",
            vec![
                Step::Turn(AssistantTurn {
                    blocks: Vec::new(),
                    tool_calls: vec![PendingCall {
                        call_id: "e1".into(),
                        tool: "edit_lines".into(),
                        arguments: serde_json::json!({"path": "src/lib.rs"}),
                    }],
                    usage: None,
                }),
                Step::Turn(usage_turn("resumed after the ask")),
            ],
        )],
        true,
    );
    let parent = kit.state.session("parent-blocked");
    // When: the child runs and its permission ask fires unanswered.
    let outcome = kit
        .services
        .run_child(&parent, quick_request("edit something"))
        .await
        .expect("child runs; the ask outcome is not a spawn failure");
    // Then: the parent stream surfaced the ask as a `blocked`
    // ChildProgress naming the tool — the aggregation vocabulary a
    // team panel renders.
    let events = hub_events(&parent);
    let blocked = events.iter().any(|event| {
        matches!(
            event,
            SessionEvent::ChildProgress { phase, detail, .. }
                if phase == "blocked" && detail.contains("edit_lines")
        )
    });
    assert!(blocked, "blocked ask on the parent stream: {events:?}");
    // And: the resolution surfaced too (the child resumed)…
    let resumed = events.iter().any(|event| {
        matches!(
            event,
            SessionEvent::ChildProgress { phase, .. } if phase == "running"
        )
    });
    assert!(resumed, "ask resolution surfaces: {events:?}");
    // …the child finished successfully after the denial, and the
    // fresh-grant contract still holds (nobody answered, nothing ran).
    assert_eq!(outcome.summary, "resumed after the ask");
    assert!(kit.edits_fired.lock().expect("fired").is_empty());
    assert!(
        events.iter().any(
            |event| matches!(event, SessionEvent::ChildFinished { state, .. } if state == "success")
        ),
        "child finished: {events:?}"
    );
}

// ------------------------------------------------------------------
// e2e: the parent transcript receives the child summary as a tool result
// ------------------------------------------------------------------

/// Subscribes (synchronously, BEFORE any submit) and answers permission
/// asks with allow-once — the task tool asks per call like every aux
/// tool.
fn spawn_permission_answerer(host: &Arc<SessionHost>) -> tokio::task::JoinHandle<()> {
    let mediator = host.mediator();
    let mut control = mediator.subscribe();
    tokio::spawn(async move {
        while let Ok(msg) = control.recv().await {
            if let ServerMsg::PermissionAsk { ask } = msg {
                let _ = mediator.answer(ask.ask_id, "e2e", Answer::AllowOnce);
            }
        }
    })
}

#[tokio::test]
async fn task_tool_e2e_child_summary_reaches_parent_transcript() {
    // Given: a routed daemon whose session registries carry the task
    // tool; the PARENT session routes its own turns on the default role
    // (claude-opus-4-7) and its model calls task(quick) mid-turn.
    let kit = task_kit(
        Duration::from_secs(30),
        vec![
            (
                "claude-opus-4-7",
                vec![
                    Step::Turn(AssistantTurn {
                        blocks: Vec::new(),
                        tool_calls: vec![PendingCall {
                            call_id: "t1".into(),
                            tool: "task".into(),
                            arguments: serde_json::json!({
                                "category": "quick",
                                "prompt": "count the widgets"
                            }),
                        }],
                        usage: None,
                    }),
                    Step::Turn(usage_turn("parent done")),
                ],
            ),
            (
                "claude-sonnet-4-6",
                vec![Step::Turn(usage_turn("e2e child answer"))],
            ),
        ],
        false,
    );
    let parent = kit.state.session("e2e-parent");
    let mut hub = parent.hub_subscribe();
    let _answerer = spawn_permission_answerer(&parent);
    // When: the parent's model turn runs (task call -> tool result ->
    // final text).
    parent
        .handle_command(HostCommand::Submit("delegate please".into()))
        .await
        .expect("submit");
    let mut finished = 0;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while finished < 1 {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        assert!(!remaining.is_zero(), "parent turn never finished");
        match tokio::time::timeout(remaining, hub.recv()).await {
            Ok(Ok(ServerMsg::Event { frame })) => {
                if let SessionEvent::TurnFinished { .. } = frame.event {
                    finished += 1;
                }
            }
            Ok(Ok(_)) => {}
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => {}
            Ok(Err(broadcast::error::RecvError::Closed)) => panic!("hub closed"),
            Err(_) => panic!("parent turn timed out"),
        }
    }
    // Then: the parent transcript holds the child summary as the task
    // TOOL RESULT, labeled as child output.
    let tool_results = hub_events(&parent)
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
        .collect::<Vec<_>>();
    assert!(
        tool_results.iter().any(|text| text
            .contains("[task child e2e-parent-child-task1 category=quick]")
            && text.contains("e2e child answer")),
        "parent tool results: {tool_results:?}"
    );
    // And: the delegation was routed — the parent served on its role
    // model, the child on the category model.
    let log = kit.log.lock().expect("log");
    assert_eq!(
        log.lookups,
        vec!["claude-opus-4-7".to_owned(), "claude-sonnet-4-6".to_owned()]
    );
}
