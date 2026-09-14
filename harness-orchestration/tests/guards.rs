//! Guard clauses of todo 40, each pinned by a test: spawn pacing (burst
//! quota + refill), depth (flat depth-1 default, opt-in nesting),
//! disjoint-write-set contract, FileTouch collision alerts, deny-only
//! permission inheritance, authorization classes + bubbling, cache-
//! preserving forks, parallelism normalization, family-switch loudness,
//! capability gating, and the model-bracket grammar (new-input parsing).

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use harness_intel::catalog::{EffortValue, ThinkingLevel};
use harness_orchestration::authorize::{
    CommandExecutionPolicy, EscalationRequest, ParentEscalations, SpawnAuthzClass, classify_spawn,
    inherit_denies,
};
use harness_orchestration::budget::{BudgetBreach, BudgetGuard, Pacer};
use harness_orchestration::events::{MemoryEventSink, TeamEvent};
use harness_orchestration::fork::{
    DepthGuard, ToolDecl, ToolResultRecord, ToolSurface, batch, fork_child,
};
use harness_orchestration::journal::{TeamJournal, is_child_session};
use harness_orchestration::knobs::{BracketError, parse_model_bracket};
use harness_orchestration::launch::Launcher;
use harness_orchestration::role::Tier;
use harness_orchestration::write_set::{FileTouch, TouchOp, TouchRegistry, WriteSet};

use harness_safety::perm::grammar::{Action, Effect, Provenance, Rule, Target, TargetPattern};

/// SPAWN-PACING (Amp orb): 20-burst, then one refill per 5 minutes.
#[test]
fn pacer_burst_quota_then_refill_rate() {
    let mut pacer = Pacer::amp();
    let step = 60_000u64; // one minute
    let mut now = 0u64;
    for i in 0..20 {
        assert!(pacer.try_acquire(now).is_ok(), "burst slot {i}");
        now += step; // slow drip stays inside the rate
    }
    // Exhaust the burst: back-to-back acquires at the same instant stop.
    let instant = now;
    let mut drained = 0;
    while pacer.try_acquire(instant).is_ok() {
        drained += 1;
        assert!(drained < 100, "pacer must deny eventually");
    }
    assert!(drained < 20, "refill between drains: {drained}");
    // A full 5-minute window refills exactly one token.
    assert!(pacer.try_acquire(instant + 5 * 60_000).is_ok());
}

/// HARD BUDGET GUARDS: children/parallel/tokens caps type their breaches.
#[test]
fn budget_guards_breach_typed() {
    let mut guard = BudgetGuard::new(1, 1, 100);
    let lease = guard.authorize_spawn().unwrap();
    assert!(matches!(
        guard.authorize_spawn(),
        Err(BudgetBreach::ChildrenExhausted { spawned: 1, max: 1 })
    ));
    guard.release(lease);
    assert!(
        matches!(
            guard.authorize_spawn(),
            Err(BudgetBreach::ChildrenExhausted { .. })
        ),
        "spawned count never resets — the child cap is per launch"
    );

    let mut parallel = BudgetGuard::new(2, 1, 100);
    let lease = parallel.authorize_spawn().unwrap();
    assert!(matches!(
        parallel.authorize_spawn(),
        Err(BudgetBreach::ParallelExhausted { active: 1, max: 1 })
    ));
    parallel.release(lease);
    assert!(parallel.authorize_spawn().is_ok());

    guard.record_spend(60).unwrap();
    assert!(matches!(
        guard.record_spend(50),
        Err(BudgetBreach::TokensExhausted {
            spent: 110,
            budget: 100
        })
    ));
    assert_eq!(
        guard.snapshot().tokens_spent,
        110,
        "spend is recorded even when breached"
    );
}

/// DEPTH: flat depth-1 default; nesting is opt-in and typed.
#[test]
fn depth_guard_is_flat_by_default() {
    let guard = DepthGuard::default();
    assert_eq!(guard.max, 1);
    assert!(guard.check(1).is_ok());
    assert!(matches!(
        guard.check(2),
        Err(harness_orchestration::fork::DepthExceeded { depth: 2, max: 1 })
    ));
    let deep = DepthGuard::new(3);
    assert!(deep.check(3).is_ok());
    assert!(deep.check(4).is_err());
}

/// DISJOINT-WRITE-SET CONTRACT: parallel siblings may not overlap write
/// globs; sequenced (dependent) siblings may.
#[test]
fn disjoint_write_sets_for_parallel_siblings() {
    let a = WriteSet::parse("src/**, docs/**");
    let b = WriteSet::parse("src/launcher.rs");
    let c = WriteSet::parse("tests/**");
    assert!(!a.overlap_with(&b).is_empty(), "nested path overlaps");
    assert!(a.overlap_with(&c).is_empty(), "disjoint stays disjoint");
    assert!(WriteSet::parse("").is_empty());
}

/// FILE-TOUCH COLLISION ALERTS: a second sibling touching an overlapping
/// file raises a collision naming BOTH sides; retirement ends the alert
/// window.
#[test]
fn file_touch_collisions_alert_both_sides() {
    let mut registry = TouchRegistry::new();
    assert!(
        registry
            .record(FileTouch {
                path: "src/lib.rs".into(),
                op: TouchOp::Edit,
                intent: "wire launcher".into(),
                child: "l-child-a".into(),
            })
            .is_none()
    );
    let collision = registry
        .record(FileTouch {
            path: "src/lib.rs".into(),
            op: TouchOp::Write,
            intent: "regen exports".into(),
            child: "l-child-b".into(),
        })
        .expect("sibling overlap must collide");
    assert_eq!(collision.a, "l-child-a");
    assert_eq!(collision.b, "l-child-b");
    assert_eq!(collision.path, "src/lib.rs");

    registry.retire("l-child-a");
    assert!(
        registry
            .record(FileTouch {
                path: "src/lib.rs".into(),
                op: TouchOp::Delete,
                intent: "cleanup".into(),
                child: "l-child-c".into(),
            })
            .is_none(),
        "retired siblings no longer collide"
    );
}

/// DENY-ONLY PERMISSION INHERITANCE: children inherit the parent's deny
/// rules — never allows or asks.
#[test]
fn deny_only_inheritance() {
    let deny = Rule::new(
        Effect::Deny,
        Action::Execute,
        TargetPattern::prefix("cargo"),
        Provenance::User,
        "test",
    )
    .unwrap();
    let allow = Rule::new(
        Effect::Allow,
        Action::Read,
        TargetPattern::prefix("src"),
        Provenance::User,
        "test",
    )
    .unwrap();
    let ask = Rule::new(
        Effect::Ask,
        Action::Write,
        TargetPattern::prefix("docs"),
        Provenance::User,
        "test",
    )
    .unwrap();
    let inherited = inherit_denies(&[deny.clone(), allow, ask]);
    assert_eq!(inherited.len(), 1);
    assert_eq!(inherited[0].effect, Effect::Deny);
    assert!(
        inherited[0]
            .pattern
            .matches(&Target::Command("cargo publish".into()))
    );
}

/// SPAWN-AUTHORIZATION CLASSES: read-only exploration is standing; edits
/// require fresh authorization.
#[test]
fn authorization_classes() {
    let roles = common::registry();
    let explorer = roles.get("explorer").unwrap().clone();
    let worker = roles.get("worker").unwrap().clone();
    assert_eq!(
        classify_spawn(&explorer, &WriteSet::parse("")),
        SpawnAuthzClass::Standing
    );
    assert_eq!(
        classify_spawn(&explorer, &WriteSet::parse("docs/map.md")),
        SpawnAuthzClass::Fresh,
        "explorer that writes is no longer standing"
    );
    assert_eq!(
        classify_spawn(&worker, &WriteSet::parse("src/**")),
        SpawnAuthzClass::Fresh
    );

    // commandExecutionPolicy ladder: off < auto < eager; sandbox is the
    // forced-sandboxed rung; parsing is strict.
    let ladder = CommandExecutionPolicy::ladder();
    assert_eq!(ladder.len(), 4);
    assert!(CommandExecutionPolicy::Off.permits(&CommandExecutionPolicy::Auto));
    assert!(!CommandExecutionPolicy::Eager.permits(&CommandExecutionPolicy::Auto));
    assert_eq!(
        CommandExecutionPolicy::parse("sandbox").unwrap(),
        CommandExecutionPolicy::Sandbox
    );
    assert!(CommandExecutionPolicy::parse("yolo").is_none());
}

/// PERMISSION BUBBLING: children escalate THROUGH the parent; the default
/// parent record-and-refuse is fail-closed, and an explicitly approving
/// parent grants. There is no child-side self-approval path in the API.
#[test]
fn permission_bubbling_through_parent() {
    let parent = ParentEscalations::new();
    let req = EscalationRequest {
        child_id: "l-child-t2".into(),
        action: Action::Execute,
        target: Target::Command("cargo test".into()),
        justification: "verify my edit".into(),
    };
    let decision = parent.escalate(req.clone());
    assert!(!decision.granted, "fail-closed default");
    assert_eq!(decision.decided_by, "parent");
    assert_eq!(parent.requests().len(), 1);
    assert_eq!(parent.requests()[0].child_id, "l-child-t2");

    let approving = ParentEscalations::approving();
    assert!(approving.escalate(req).granted);
}

/// CACHE-PRESERVING FORKS (Qwen): the fork inherits the parent's exact
/// system prompt and tool DECLARATIONS verbatim; execution is narrowed by
/// the allowlist; declarations are never rewritten.
#[test]
fn forks_preserve_declarations_and_prompt() {
    let surface = ToolSurface {
        declarations: vec![
            ToolDecl {
                name: "read".into(),
                declaration: "read(path) -> bytes".into(),
            },
            ToolDecl {
                name: "edit".into(),
                declaration: "edit(path, diff)".into(),
            },
            ToolDecl {
                name: "grep".into(),
                declaration: "grep(pattern)".into(),
            },
        ],
        batch_tool: true,
        serial_tools: vec!["edit".into()],
    };
    let plan = fork_child(
        "PARENT SYSTEM PROMPT v1",
        &surface,
        &["read".to_string(), "grep".to_string()],
    );
    assert_eq!(
        plan.system_prompt, "PARENT SYSTEM PROMPT v1",
        "verbatim prompt"
    );
    assert_eq!(
        plan.surface.declaration_names(),
        vec!["read", "edit", "grep"],
        "declarations unchanged"
    );
    assert!(plan.execution_allowed("read"));
    assert!(
        !plan.execution_allowed("edit"),
        "execution narrowed by allowlist"
    );
}

/// PARALLELISM NORMALIZATION: tool results batch into ONE message; mutating
/// tools are flagged serial.
#[test]
fn parallelism_normalization_batches_results() {
    let batched = batch(vec![
        ToolResultRecord {
            call_id: "c1".into(),
            tool: "read".into(),
            ok: true,
            bytes: 10,
        },
        ToolResultRecord {
            call_id: "c2".into(),
            tool: "grep".into(),
            ok: true,
            bytes: 4,
        },
    ]);
    let message = batched.single_message();
    assert!(
        message.contains("c1") && message.contains("c2"),
        "one message carries both: {message}"
    );
    assert_eq!(
        message.matches('\n').count(),
        0,
        "single line = single message"
    );

    let surface = ToolSurface {
        declarations: vec![],
        batch_tool: true,
        serial_tools: vec!["edit".into(), "write".into()],
    };
    assert!(surface.is_serial("edit"));
    assert!(!surface.is_serial("read"));
    assert!(surface.batch_tool);
}

/// FAMILY SWITCH LOUDNESS: a model-family change between waves is surfaced
/// in the report, the journal, and the event stream — never silent.
#[tokio::test]
async fn family_switch_surfaces_loudly() {
    let spec = harness_orchestration::spec::parse_spec(common::THREE_AGENT_SPEC).unwrap();
    let mut cfg = common::config(8, 2, 64_000);
    cfg.validators = false;
    let plan = harness_orchestration::launch::resolve(
        "launch",
        &spec,
        &common::registry(),
        &common::catalog(),
        &common::profiles(),
        &cfg,
    )
    .unwrap();
    // Wave 0 is sonnet (anthropic); wave 1 contains gpt-5-5 (openai).
    assert!(
        !plan.family_switches.is_empty(),
        "resolve surfaces the anthropic->openai switch"
    );

    let dir = tempfile::tempdir().unwrap();
    let jpath = dir.path().join("fam.jsonl");
    let journal = TeamJournal::open(&jpath).unwrap();
    let sink = Arc::new(MemoryEventSink::new());
    let executor = Arc::new(common::FakeExecutor::new());
    for job in plan.waves.iter().flatten() {
        executor.ok_child(&job.child_id, "done");
    }

    let mut launcher = Launcher::new(cfg, sink.clone(), journal);
    common::grant_fresh(&mut launcher, &plan);
    let report = launcher.run(&plan, executor.clone(), None).await.unwrap();
    assert!(
        !report.family_switches.is_empty(),
        "report carries the switch"
    );

    let rows = harness_orchestration::journal::load_records(&jpath).unwrap();
    assert!(rows
        .iter()
        .any(|r| matches!(r, harness_orchestration::journal::JournalRecord::FamilySwitch { from, .. } if from == "anthropic")));
    assert!(
        sink.events()
            .iter()
            .any(|e| matches!(e, TeamEvent::FamilySwitch { .. }))
    );

    // The switched child's job carries a context warning (thinking-block /
    // context-loss surface), the unswitched child does not.
    let jobs = executor.jobs.lock().unwrap();
    let switched = jobs
        .iter()
        .find(|r| r.job.knobs.model.model == "gpt-5-5")
        .unwrap();
    assert!(
        switched.job.context_warning.is_some(),
        "family-changed child sees the warning"
    );
    let same_family = jobs
        .iter()
        .find(|r| r.job.knobs.model.model == "claude-sonnet-4-6")
        .unwrap();
    assert!(same_family.job.context_warning.is_none());
}

/// CAPABILITY GATING: children do not get task/todowrite capability unless
/// the launch config says so.
#[tokio::test]
async fn child_capabilities_are_gated() {
    use harness_orchestration::launch::resolve;

    let spec = harness_orchestration::spec::parse_spec(common::THREE_AGENT_SPEC).unwrap();
    let mut cfg = common::config(8, 2, 64_000);
    cfg.validators = false;

    let plan = resolve(
        "launch",
        &spec,
        &common::registry(),
        &common::catalog(),
        &common::profiles(),
        &cfg,
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let journal = TeamJournal::open(&dir.path().join("cap1.jsonl")).unwrap();
    let executor = Arc::new(common::FakeExecutor::new());
    for job in plan.waves.iter().flatten() {
        executor.ok_child(&job.child_id, "done");
    }
    let mut launcher = Launcher::new(cfg.clone(), Arc::new(MemoryEventSink::new()), journal);
    common::grant_fresh(&mut launcher, &plan);
    launcher.run(&plan, executor.clone(), None).await.unwrap();
    assert!(
        executor
            .jobs
            .lock()
            .unwrap()
            .iter()
            .all(|r| !r.job.capabilities.task && !r.job.capabilities.todo),
        "default: no task/todowrite for children"
    );

    let mut cfg_on = common::config(8, 2, 64_000);
    cfg_on.validators = false;
    cfg_on.child_capabilities = common::full_capabilities();
    let plan = resolve(
        "launch",
        &spec,
        &common::registry(),
        &common::catalog(),
        &common::profiles(),
        &cfg_on,
    )
    .unwrap();
    let dir2 = tempfile::tempdir().unwrap();
    let journal2 = TeamJournal::open(&dir2.path().join("cap2.jsonl")).unwrap();
    let executor2 = Arc::new(common::FakeExecutor::new());
    for job in plan.waves.iter().flatten() {
        executor2.ok_child(&job.child_id, "done");
    }
    let mut launcher2 = Launcher::new(cfg_on, Arc::new(MemoryEventSink::new()), journal2);
    common::grant_fresh(&mut launcher2, &plan);
    launcher2.run(&plan, executor2.clone(), None).await.unwrap();
    assert!(
        executor2
            .jobs
            .lock()
            .unwrap()
            .iter()
            .all(|r| r.job.capabilities.task && r.job.capabilities.todo),
        "opt-in grants the capabilities"
    );
}

/// NEW-INPUT PARSING: the model-bracket grammar `model[k=v,...]` (Cursor's
/// portable per-subagent knobs) parses typed and bounded.
#[test]
fn model_bracket_grammar() {
    let plain = parse_model_bracket("claude-sonnet-4-6").unwrap();
    assert_eq!(plain.model, "claude-sonnet-4-6");
    assert_eq!(plain.effort, None);

    let full =
        parse_model_bracket("claude-opus-4-7[effort=high,context=300k,thinking=medium]").unwrap();
    assert_eq!(full.model, "claude-opus-4-7");
    assert_eq!(full.effort, Some(EffortValue::High));
    assert_eq!(full.context_tokens, Some(300_000));
    assert_eq!(full.thinking_level, Some(ThinkingLevel::Medium));

    assert_eq!(
        parse_model_bracket("m[effort=low]").unwrap().effort,
        Some(EffortValue::Low),
        "effort alone is legal"
    );
    assert_eq!(
        parse_model_bracket("m[context=200K]")
            .unwrap()
            .context_tokens,
        Some(200_000),
        "case-insensitive K suffix"
    );
    assert!(
        matches!(parse_model_bracket("m[turbo=1]"), Err(BracketError::UnknownKey { key }) if key == "turbo")
    );
    assert!(matches!(
        parse_model_bracket("m[effort=turbo]"),
        Err(BracketError::UnknownEffort { .. })
    ));
    assert!(matches!(
        parse_model_bracket("m[context=abc]"),
        Err(BracketError::BadContext { .. })
    ));
    assert!(matches!(
        parse_model_bracket("m[effort=high"),
        Err(BracketError::Unclosed)
    ));
    assert!(matches!(
        parse_model_bracket("[effort=high]"),
        Err(BracketError::EmptyModel)
    ));
    assert!(matches!(
        parse_model_bracket("m[a[b=1]]"),
        Err(BracketError::Nested)
    ));
}

/// HIDDEN CHILD SESSIONS: ids are namespaced and recognizable.
#[test]
fn child_session_ids_are_namespaced() {
    assert!(is_child_session("launch-child-t1"));
    assert!(!is_child_session("launch"));
    assert!(!is_child_session("regular-session"));
}

/// Tier policy ladder defaults: explorer never executes, worker asks, deep
/// runs sandboxed.
#[test]
fn tier_execution_policies() {
    let roles = common::registry();
    assert_eq!(
        roles.get("explorer").unwrap().policy,
        CommandExecutionPolicy::Off
    );
    assert_eq!(
        roles.get("worker").unwrap().policy,
        CommandExecutionPolicy::Auto
    );
    assert_eq!(
        roles.get("deep").unwrap().policy,
        CommandExecutionPolicy::Sandbox
    );
    let _ = common::tier_name(Tier::Explorer);
    let _: BTreeMap<String, u8> = BTreeMap::new();
}
