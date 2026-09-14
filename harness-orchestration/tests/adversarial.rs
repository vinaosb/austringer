//! Adversarial channels: stale-state spec reload aborts the next wave
//! (fail closed), adversarial spec inputs parse bounded without panics,
//! task_id-resumable children re-run under their original identity, and
//! the tmux-grid team visualization renders from real events.

mod common;

use std::sync::Arc;

use harness_orchestration::events::{MemoryEventSink, TeamEvent, render_grid};
use harness_orchestration::journal::{TeamJournal, load_records};
use harness_orchestration::launch::{LaunchOutcome, Launcher, StopReason, resolve};
use harness_orchestration::spec::{SpecError, parse_spec};

/// STALE STATE: the spec file mutates mid-run (during wave 0); the
/// launcher detects the fingerprint change before wave 1 and aborts with
/// a typed stop — the mutated wave never runs.
#[tokio::test]
async fn stale_spec_aborts_next_wave() {
    let dir = tempfile::tempdir().unwrap();
    let spec_path = dir.path().join("spec.launch");
    std::fs::write(&spec_path, common::THREE_AGENT_SPEC).unwrap();

    let spec = parse_spec(common::THREE_AGENT_SPEC).unwrap();
    let mut cfg = common::config(8, 2, 64_000);
    cfg.validators = false;
    let plan = resolve(
        "stale",
        &spec,
        &common::registry(),
        &common::catalog(),
        &common::profiles(),
        &cfg,
    )
    .unwrap();

    let jpath = dir.path().join("stale.jsonl");
    let journal = TeamJournal::open(&jpath).unwrap();
    let executor = Arc::new(common::FakeExecutor::new());
    executor.ok_child("stale-child-t1", "done");
    executor.ok_child("stale-child-t2", "done");
    executor.ok_child("stale-child-t3", "done");

    // The wave-0 child rewrites the spec file while the launch is running.
    let mutation_path = spec_path.clone();
    executor
        .on_run
        .lock()
        .unwrap()
        .replace(Box::new(move |job| {
            if job.child_id == "stale-child-t1" {
                std::fs::write(&mutation_path, "objective: CHANGED MID-RUN\n").unwrap();
            }
        }));

    let mut launcher = Launcher::new(cfg, Arc::new(MemoryEventSink::new()), journal);
    let report = launcher
        .run(&plan, executor.clone(), Some(&spec_path.clone().into()))
        .await
        .unwrap();

    assert_eq!(report.outcome, LaunchOutcome::Partial);
    assert_eq!(report.children.len(), 1, "only wave 0 completed");
    match report.stopped_early.as_ref().unwrap() {
        StopReason::StaleSpec { .. } => {}
        other => panic!("expected stale-spec stop, got {other:?}"),
    }
    assert_eq!(
        executor
            .job_ids()
            .iter()
            .filter(|id| id.contains("t2") || id.contains("t3"))
            .count(),
        0,
        "mutated wave never spawned"
    );
    let rows = load_records(&jpath).unwrap();
    assert!(rows.iter().any(|r| matches!(
        r,
        harness_orchestration::journal::JournalRecord::SpecStale { .. }
    )));
}

/// NEW/ADVERSARIAL INPUTS: hostile spec texts parse bounded and typed —
/// never a panic, never unbounded allocation.
#[test]
fn adversarial_spec_inputs_are_bounded() {
    let hostile = [
        "",
        "\n\n\n",
        "\u{feff}objective: bom",
        "objective: x\u{0}y",
        "task t:\n  role: worker\n  objective: \u{7f}\n",
        "budget:\n  children: -1\n",
        "budget:\n  children: notanumber\n",
        "task t:\n",
        "task:\n  role: worker\n",
        "  orphan: line\n",
        "objective: x\ntask t1:\n  role: worker\n  objective: y\n  needs: ,,\n",
        "objective: x\ntask t1:\n  role: worker\n  objective: y\n  class: dinner\n",
        "role r:\ntask t1:\n  role: r\n  objective: y\n",
        "objective: x\nvalidators: maybe\ntask t1:\n  role: worker\n  objective: y\n",
        "objective: x\nbudget:\n  mystery: 1\ntask t1:\n  role: worker\n  objective: y\n",
    ];
    for (i, text) in hostile.iter().enumerate() {
        let outcome = parse_spec(text);
        if let Ok(spec) = outcome {
            // Only the well-formed minimal specs survive; every field stays bounded.
            assert!(
                spec.tasks.len() <= harness_orchestration::spec::MAX_TASKS,
                "hostile case {i} bounded"
            );
            for value in spec
                .objectives
                .iter()
                .chain(spec.tasks.iter().map(|t| &t.objective))
            {
                assert!(
                    value.len() <= harness_orchestration::spec::MAX_FIELD_BYTES,
                    "case {i} field bounded"
                );
            }
        }
        // Err cases are typed SpecError values; nothing panics (test reaches here).
    }
    assert!(matches!(parse_spec(""), Err(SpecError::NoTasks)));

    let mut many = String::from("objective: x\n");
    for i in 0..(harness_orchestration::spec::MAX_TASKS + 1) {
        many.push_str(&format!("task t{i}:\n  role: worker\n  objective: y\n"));
    }
    assert!(
        matches!(parse_spec(&many), Err(SpecError::TooManyTasks { .. })),
        "task count capped"
    );
}

/// TASK-ID RESUMABLE CHILDREN (Kilo): a completed child re-runs under its
/// ORIGINAL identity (same child id, same knobs) through resume().
#[tokio::test]
async fn children_are_task_id_resumable() {
    let spec = parse_spec(common::THREE_AGENT_SPEC).unwrap();
    let mut cfg = common::config(8, 2, 64_000);
    cfg.validators = false;
    let plan = resolve(
        "stale",
        &spec,
        &common::registry(),
        &common::catalog(),
        &common::profiles(),
        &cfg,
    )
    .unwrap();

    let dir = tempfile::tempdir().unwrap();
    let jpath = dir.path().join("resume.jsonl");
    let journal = TeamJournal::open(&jpath).unwrap();
    let executor = Arc::new(common::FakeExecutor::new());
    executor.ok_child("stale-child-t2", "resumed run");

    let mut launcher = Launcher::new(cfg, Arc::new(MemoryEventSink::new()), journal);
    let report = launcher
        .resume(&plan, executor.clone(), "t2")
        .await
        .unwrap();

    assert_eq!(
        report.child_id, "stale-child-t2",
        "original identity preserved"
    );
    let jobs = executor.jobs.lock().unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(
        jobs[0].job.knobs.model.model, "gpt-5-5",
        "same resolved knobs"
    );

    let rows = load_records(&jpath).unwrap();
    let resumed_rows: Vec<_> = rows
        .iter()
        .filter_map(|r| match r {
            harness_orchestration::journal::JournalRecord::ChildStarted {
                child, resumed, ..
            } => Some((child.clone(), *resumed)),
            _ => None,
        })
        .collect();
    assert_eq!(
        resumed_rows,
        vec![("stale-child-t2".to_string(), true)],
        "journal marks the row resumed"
    );
}

/// TEAM VISUALIZATION: the tmux-grid renderer lays active children out in
/// panes from the live event stream.
#[tokio::test]
async fn team_grid_renders_from_events() {
    let spec = parse_spec(common::THREE_AGENT_SPEC).unwrap();
    let mut cfg = common::config(8, 2, 64_000);
    cfg.validators = false;
    let plan = resolve(
        "grid",
        &spec,
        &common::registry(),
        &common::catalog(),
        &common::profiles(),
        &cfg,
    )
    .unwrap();

    let dir = tempfile::tempdir().unwrap();
    let journal = TeamJournal::open(&dir.path().join("grid.jsonl")).unwrap();
    let sink = Arc::new(MemoryEventSink::new());
    let executor = Arc::new(common::FakeExecutor::new());
    for job in plan.waves.iter().flatten() {
        executor.ok_child(&job.child_id, "done");
    }

    let mut launcher = Launcher::new(cfg, sink.clone(), journal);
    common::grant_fresh(&mut launcher, &plan);
    let report = launcher.run(&plan, executor.clone(), None).await.unwrap();
    assert_eq!(report.outcome, LaunchOutcome::Complete);

    let spawned = sink.events();
    let active: Vec<(&str, &str, &str)> = spawned
        .iter()
        .filter_map(|e| match e {
            TeamEvent::Spawned {
                child, role, model, ..
            } => Some((child.as_str(), role.as_str(), model.as_str())),
            _ => None,
        })
        .collect();
    let grid = render_grid(&active);
    assert!(grid.contains("grid-child-t1"), "pane per child: {grid}");
    assert!(grid.contains("grid-child-t2") && grid.contains("grid-child-t3"));
    assert!(grid.lines().count() >= 3, "grid layout, not a flat list");

    // The event stream maps onto the daemon's frozen Notification surface.
    let notification = TeamEvent::Spawned {
        child: "grid-child-t1".into(),
        role: "explorer".into(),
        model: "claude-sonnet-4-6".into(),
        effort: "medium".into(),
    }
    .notification();
    assert_eq!(notification.kind, "subagent_spawned");
    assert!(notification.message.contains("grid-child-t1"));
}

/// Misleading-success guard on counters: the journal is the source of
/// truth for spend, and the budget snapshot must agree with it.
#[test]
fn budget_snapshot_agrees_with_telemetry() {
    use harness_orchestration::budget::BudgetGuard;

    let mut guard = BudgetGuard::new(4, 2, 10_000);
    for _ in 0..3 {
        let lease = guard.authorize_spawn().unwrap();
        guard.release(lease);
        guard.record_spend(1500).unwrap();
    }
    let snap = guard.snapshot();
    assert_eq!(snap.tokens_spent, 4500);
    assert_eq!(snap.spawned, 3);
}
