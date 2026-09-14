//! Spec-launcher acceptance (plan todo 40): scripted multi-model spec run —
//! decomposition, parallel children, merged result, per-agent telemetry;
//! typed failure channels (unknown role, unresolvable model, dependency
//! cycle); budget breach stops spawning and reports partials.
//!
//! The misleading-success discipline: assertions read the SPAWNED EFFECTS
//! (on-disk journal + executor-recorded jobs), never only return values.

mod common;

use std::sync::Arc;

use common::{THREE_AGENT_SPEC, catalog, config, profiles, registry};

use harness_orchestration::LaunchError;
use harness_orchestration::events::{MemoryEventSink, TeamEvent};
use harness_orchestration::journal::{TeamJournal, load_records};
use harness_orchestration::launch::{LaunchOutcome, Launcher, StopReason, resolve};
use harness_orchestration::spec::{SpecError, parse_spec};

fn tmp_journal(tag: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(format!("{tag}-journal.jsonl"));
    (dir, path)
}

/// HAPPY (plan acceptance): three agents, two waves, in budget. Proves
/// decomposition (waves), parallel children, merged synthesis, per-agent
/// telemetry — from the journal on disk and the executor's recorded jobs.
#[tokio::test]
async fn happy_three_agent_spec_completes_in_budget() {
    let spec = parse_spec(THREE_AGENT_SPEC).unwrap();
    let mut cfg = config(8, 2, 64_000);
    cfg.validators = false; // validator milestones asserted in phases.rs
    let plan = resolve("launch", &spec, &registry(), &catalog(), &profiles(), &cfg).unwrap();
    assert_eq!(plan.waves.len(), 2, "t1 | t2,t3 decomposition");

    let (_dir, jpath) = tmp_journal("happy");
    let journal = TeamJournal::open(&jpath).unwrap();
    let sink = Arc::new(MemoryEventSink::new());
    let executor = Arc::new(common::FakeExecutor::new());
    for job in plan.waves.iter().flatten() {
        executor.ok_child(&job.child_id, "done");
    }

    let mut launcher = Launcher::new(cfg, sink.clone(), journal);
    common::grant_fresh(&mut launcher, &plan);
    let report = launcher.run(&plan, executor.clone(), None).await.unwrap();

    assert_eq!(report.outcome, LaunchOutcome::Complete);
    assert_eq!(report.children.len(), 3, "three agents ran");
    assert!(report.stopped_early.is_none());
    assert!(report.synthesis.is_some(), "merged result present");

    // Per-agent telemetry rollup: 3 x (1000 in + 500 out).
    let syn = report.synthesis.as_ref().unwrap();
    assert_eq!(syn.per_agent.len(), 3);
    assert!(syn.per_agent.iter().all(|l| l.tokens == 1500));

    // SPAWNED EFFECTS: the journal on disk carries one ChildStarted and one
    // ChildCompleted per agent, with the resolved model + effort in the spec
    // payload, and a LaunchFinished row.
    let rows = load_records(&jpath).unwrap();
    let started: Vec<_> = rows
        .iter()
        .filter_map(|r| match r {
            harness_orchestration::journal::JournalRecord::ChildStarted { child, spec, .. } => {
                Some((child.clone(), spec.clone()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(started.len(), 3, "journal child_started rows");
    assert!(
        started
            .iter()
            .any(|(_, s)| s.contains("claude-opus-4-7") && s.contains("high")),
        "explicit bracket knobs resolved into the spawned spec: {started:?}"
    );
    assert!(
        started.iter().any(|(_, s)| s.contains("gpt-5-5")),
        "worker role resolves its default model: {started:?}"
    );
    let completed = rows
        .iter()
        .filter(|r| {
            matches!(
                r,
                harness_orchestration::journal::JournalRecord::ChildCompleted { .. }
            )
        })
        .count();
    assert_eq!(completed, 3);
    assert!(rows.iter().any(|r| matches!(
        r,
        harness_orchestration::journal::JournalRecord::LaunchFinished { complete: true, .. }
    )));

    // Heterogeneous launch: the executor saw three distinct models.
    let models: Vec<_> = executor
        .jobs
        .lock()
        .unwrap()
        .iter()
        .map(|r| r.job.knobs.model.model.clone())
        .collect();
    assert_eq!(models.len(), 3);
    assert!(
        models.iter().any(|m| m == "claude-sonnet-4-6")
            && models.iter().any(|m| m == "gpt-5-5")
            && models.iter().any(|m| m == "claude-opus-4-7"),
        "heterogeneous (capability, model) profiles: {models:?}"
    );

    // Child ids are resumable task ids and hidden child sessions.
    assert!(
        report
            .child_ids
            .iter()
            .all(|id| harness_orchestration::journal::is_child_session(id))
    );

    // Events: team visualization saw every spawn and completion.
    let events = sink.events();
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, TeamEvent::Spawned { .. }))
            .count(),
        3
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, TeamEvent::Completed { .. }))
            .count(),
        3
    );
}

/// FAILURE: unknown role is a typed resolution error naming the role and
/// the known registry entries.
#[test]
fn unknown_role_is_typed_error() {
    let text = "objective: x\ntask t1:\n  role: chef\n  objective: cook\n";
    let spec = parse_spec(text).unwrap();
    let cfg = config(4, 2, 10_000);
    let err = resolve("launch", &spec, &registry(), &catalog(), &profiles(), &cfg).unwrap_err();
    match err {
        harness_orchestration::launch::ResolveError::UnknownRole { role, known } => {
            assert_eq!(role, "chef");
            assert!(
                known.iter().any(|k| k == "explorer"),
                "known list in error: {known:?}"
            );
        }
        other => panic!("expected UnknownRole, got {other:?}"),
    }
}

/// FAILURE: unresolvable spec typed — unknown model surfaces the catalog's
/// fail-closed error; a dependency cycle is detected with the path.
#[test]
fn unresolvable_spec_is_typed() {
    let bad_model =
        "objective: x\ntask t1:\n  role: worker\n  model: gpt-9-turbo\n  objective: y\n";
    let spec = parse_spec(bad_model).unwrap();
    let cfg = config(4, 2, 10_000);
    let err = resolve("launch", &spec, &registry(), &catalog(), &profiles(), &cfg).unwrap_err();
    assert!(
        matches!(err, harness_orchestration::launch::ResolveError::Model { ref model, .. } if model == "gpt-9-turbo"),
        "unknown model typed: {err:?}"
    );

    let cyclic = "objective: x\ntask a:\n  role: worker\n  objective: y\n  needs: b\ntask b:\n  role: worker\n  objective: z\n  needs: a\n";
    let spec = parse_spec(cyclic).unwrap();
    let err = resolve("launch", &spec, &registry(), &catalog(), &profiles(), &cfg).unwrap_err();
    assert!(
        matches!(
            err,
            harness_orchestration::launch::ResolveError::DependencyCycle { .. }
        ),
        "cycle typed: {err:?}"
    );

    let dangling = "objective: x\ntask a:\n  role: worker\n  objective: y\n  needs: ghost\n";
    let spec = parse_spec(dangling).unwrap();
    let err = resolve("launch", &spec, &registry(), &catalog(), &profiles(), &cfg).unwrap_err();
    assert!(
        matches!(err, harness_orchestration::launch::ResolveError::MissingDependency { ref task, ref needs } if task == "a" && needs == "ghost"),
        "dangling dep typed: {err:?}"
    );
}

/// FAILURE (plan QA): budget breach stops spawning and reports partials —
/// children cap exhausted after the first agent.
#[tokio::test]
async fn budget_breach_stops_spawning_and_reports_partials() {
    let spec = parse_spec(THREE_AGENT_SPEC).unwrap();
    let cfg = config(1, 2, 64_000); // one child only
    let plan = resolve("launch", &spec, &registry(), &catalog(), &profiles(), &cfg).unwrap();

    let (_dir, jpath) = tmp_journal("breach");
    let journal = TeamJournal::open(&jpath).unwrap();
    let executor = Arc::new(common::FakeExecutor::new());
    executor.ok_child("launch-child-t1", "done");

    let mut launcher = Launcher::new(cfg, Arc::new(MemoryEventSink::new()), journal);
    let report = launcher.run(&plan, executor.clone(), None).await.unwrap();

    assert_eq!(report.outcome, LaunchOutcome::Partial);
    assert_eq!(report.children.len(), 1, "partials reported");
    match report.stopped_early.as_ref().unwrap() {
        StopReason::Budget(b) => assert!(
            matches!(
                b,
                harness_orchestration::budget::BudgetBreach::ChildrenExhausted {
                    spawned: 1,
                    max: 1
                }
            ),
            "children-exhausted stop: {b:?}"
        ),
        other => panic!("expected budget stop, got {other:?}"),
    }
    assert_eq!(executor.job_count(), 1, "no further spawns after breach");
    let rows = load_records(&jpath).unwrap();
    assert!(rows.iter().any(|r| matches!(
        r,
        harness_orchestration::journal::JournalRecord::BudgetStopped { .. }
    )));
}

/// FAILURE: token budget exhausted mid-run stops before the next wave and
/// reports the completed partials.
#[tokio::test]
async fn token_breach_stops_spawning_and_reports_partials() {
    let spec = parse_spec(THREE_AGENT_SPEC).unwrap();
    let cfg = config(8, 2, 1_200); // one child spends 1500
    let plan = resolve("launch", &spec, &registry(), &catalog(), &profiles(), &cfg).unwrap();

    let (_dir, jpath) = tmp_journal("tokbreach");
    let journal = TeamJournal::open(&jpath).unwrap();
    let executor = Arc::new(common::FakeExecutor::new());
    executor.ok_child("launch-child-t1", "done");

    let mut launcher = Launcher::new(cfg, Arc::new(MemoryEventSink::new()), journal);
    common::grant_fresh(&mut launcher, &plan);
    let report = launcher.run(&plan, executor.clone(), None).await.unwrap();

    assert_eq!(report.outcome, LaunchOutcome::Partial);
    assert_eq!(report.children.len(), 1);
    assert!(matches!(
        report.stopped_early.as_ref().unwrap(),
        StopReason::Budget(harness_orchestration::budget::BudgetBreach::TokensExhausted { .. })
    ));
    assert_eq!(executor.job_count(), 1);
}

/// FAILURE: spawn refusals are LOUD structured errors — a fresh-class child
/// without an explicit grant aborts the launch; nothing runs in-parent.
#[tokio::test]
async fn fresh_authorization_missing_is_loud_refusal() {
    let text = "objective: x\nbudget:\n  children: 4\ntask t1:\n  role: worker\n  objective: y\n  writes: src/**\n";
    let spec = parse_spec(text).unwrap();
    let cfg = config(4, 2, 10_000);
    let plan = resolve("launch", &spec, &registry(), &catalog(), &profiles(), &cfg).unwrap();

    let (_dir, jpath) = tmp_journal("refusal");
    let journal = TeamJournal::open(&jpath).unwrap();
    let executor = Arc::new(common::FakeExecutor::new());

    let mut launcher = Launcher::new(cfg, Arc::new(MemoryEventSink::new()), journal);
    let err = launcher
        .run(&plan, executor.clone(), None)
        .await
        .unwrap_err();
    match err {
        LaunchError::SpawnRefusal(r) => {
            assert_eq!(r.role, "worker");
            assert_eq!(r.writes, vec!["src/**".to_string()]);
        }
        other => panic!("expected SpawnRefusal, got {other:?}"),
    }
    assert_eq!(
        executor.job_count(),
        0,
        "never a silent parent-DIY fallback"
    );
}

/// ADVERSARIAL (bounded parse): malformed specs fail typed and bounded —
/// oversize input, unknown keys, missing objective, duplicates.
#[test]
fn malformed_spec_is_bounded_and_typed() {
    let huge =
        "objective: x\n".to_string() + &"task t:\n  role: worker\n  objective: y\n".repeat(300);
    match parse_spec(&huge) {
        Err(SpecError::TooLarge { lines, limit }) => {
            assert!(lines > harness_orchestration::spec::MAX_SPEC_LINES);
            assert_eq!(limit, harness_orchestration::spec::MAX_SPEC_LINES);
        }
        other => panic!("expected TooLarge, got {other:?}"),
    }

    match parse_spec("objective: x\nmystery: 1\n") {
        Err(SpecError::UnknownKey { key, .. }) => assert_eq!(key, "mystery"),
        other => panic!("expected UnknownKey, got {other:?}"),
    }

    assert!(matches!(
        parse_spec("task t1:\n  role: worker\n"),
        Err(SpecError::MissingObjective)
    ));
    assert!(matches!(
        parse_spec(
            "objective: x\ntask t1:\n  role: worker\n  objective: y\ntask t1:\n  role: worker\n  objective: z\n"
        ),
        Err(SpecError::Duplicate { .. })
    ));
    // A 512 KiB single line is rejected by the byte cap, not allocated into fields.
    let long_line = format!("objective: {}\n", "x".repeat(512 * 1024));
    assert!(matches!(
        parse_spec(&long_line),
        Err(SpecError::TooLarge { .. })
    ));
}

/// ADVERSARIAL (misleading_success): journal truthfulness — a failed child
/// produces ChildFailed and NO ChildCompleted row for that id.
#[tokio::test]
async fn journal_records_failures_honestly() {
    let spec = parse_spec(THREE_AGENT_SPEC).unwrap();
    let mut cfg = config(8, 2, 64_000);
    cfg.validators = false;
    let plan = resolve("launch", &spec, &registry(), &catalog(), &profiles(), &cfg).unwrap();

    let (_dir, jpath) = tmp_journal("honest");
    let journal = TeamJournal::open(&jpath).unwrap();
    let executor = Arc::new(common::FakeExecutor::new());
    executor.ok_child("launch-child-t1", "done");
    executor.ok_child("launch-child-t2", "done");
    executor.fail_child(
        "launch-child-t3",
        harness_orchestration::telemetry::ChildFailureClass::Model,
    );

    let mut launcher = Launcher::new(cfg, Arc::new(MemoryEventSink::new()), journal);
    common::grant_fresh(&mut launcher, &plan);
    let report = launcher.run(&plan, executor.clone(), None).await.unwrap();

    // One failed child does not stop the launch; it is reported as failure.
    assert_eq!(report.outcome, LaunchOutcome::Complete);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.children.len(), 2);

    let rows = load_records(&jpath).unwrap();
    let t3_completed = rows.iter().any(|r| matches!(
        r,
        harness_orchestration::journal::JournalRecord::ChildCompleted { child, .. } if child == "launch-child-t3"
    ));
    assert!(!t3_completed, "no completion row for the failed child");
    let t3_failed = rows.iter().any(|r| matches!(
        r,
        harness_orchestration::journal::JournalRecord::ChildFailed { child, .. } if child == "launch-child-t3"
    ));
    assert!(t3_failed, "failure row present");
}
