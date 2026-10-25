//! /boost-style 3-phase decoupling (strategy -> isolated execution ->
//! verification), mission validator milestones (scrutiny + user-testing,
//! config-skippable), the CLEAN-CONTEXT reviewer (zero shared context —
//! reasons backward from the diff alone), and synthesis with the
//! cross-consistency check.

mod common;

use std::sync::Arc;

use austringer_orchestration::events::MemoryEventSink;
use austringer_orchestration::journal::{TeamJournal, load_records};
use austringer_orchestration::launch::{Artifact, ChildReport, ChildTelemetry};
use austringer_orchestration::launch::{Launcher, resolve};
use austringer_orchestration::phases::{
    BoostPhase, CleanContextReview, ValidatorKind, validator_jobs,
};
use austringer_orchestration::synthesis::synthesize;

/// THREE-PHASE DECOUPLE: with phases on, the launch runs strategy and
/// verification as separate agent phases around the execution waves.
#[tokio::test]
async fn boost_decouples_strategy_execution_verification() {
    let spec = austringer_orchestration::spec::parse_spec(common::THREE_AGENT_SPEC).unwrap();
    let mut cfg = common::config(12, 2, 640_000);
    cfg.phases = true;
    cfg.validators = false;
    let plan = resolve(
        "boost",
        &spec,
        &common::registry(),
        &common::catalog(),
        &common::profiles(),
        &cfg,
    )
    .unwrap();

    let dir = tempfile::tempdir().unwrap();
    let jpath = dir.path().join("boost.jsonl");
    let journal = TeamJournal::open(&jpath).unwrap();
    let executor = Arc::new(common::FakeExecutor::new());
    for job in plan.waves.iter().flatten() {
        executor.ok_child(&job.child_id, "done");
    }
    executor.ok_child("boost-child-strategy", "strategy ready");
    executor.ok_child("boost-child-verification", "review complete");

    let mut launcher = Launcher::new(cfg, Arc::new(MemoryEventSink::new()), journal);
    common::grant_fresh(&mut launcher, &plan);
    let report = launcher.run(&plan, executor.clone(), None).await.unwrap();

    assert_eq!(
        report.phase_trace,
        vec!["strategy", "execution", "verification"],
        "three separate agent phases"
    );
    assert!(
        report
            .children
            .iter()
            .any(|c| c.child_id == "boost-child-strategy")
    );
    assert!(
        report
            .children
            .iter()
            .any(|c| c.child_id == "boost-child-verification")
    );

    // The strategy phase runs read-only (explorer tier) before any wave.
    let recorded = executor.jobs.lock().unwrap();
    let strategy_job = recorded
        .iter()
        .find(|r| r.job.child_id == "boost-child-strategy")
        .unwrap();
    assert_eq!(common::tier_name(strategy_job.job.tier), "explorer");
    assert!(strategy_job.job.writes.is_empty());
    drop(recorded);
}

/// MISSION VALIDATOR MILESTONES: task boundaries spawn separate scrutiny
/// and user-testing validator workers; `validators: off` skips them.
#[tokio::test]
async fn validator_milestones_spawn_and_are_skippable() {
    let spec = austringer_orchestration::spec::parse_spec(common::THREE_AGENT_SPEC).unwrap();

    // ON: one boundary between the two waves -> two validators.
    let mut cfg = common::config(12, 2, 640_000);
    cfg.validators = true;
    let plan = resolve(
        "val",
        &spec,
        &common::registry(),
        &common::catalog(),
        &common::profiles(),
        &cfg,
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let journal = TeamJournal::open(&dir.path().join("val.jsonl")).unwrap();
    let executor = Arc::new(common::FakeExecutor::new());
    for job in plan.waves.iter().flatten() {
        executor.ok_child(&job.child_id, "done");
    }
    executor.ok_child("val-child-v-scrutiny", "scrutiny pass");
    executor.ok_child("val-child-v-user-testing", "user-testing pass");
    let mut launcher = Launcher::new(cfg, Arc::new(MemoryEventSink::new()), journal);
    common::grant_fresh(&mut launcher, &plan);
    let report = launcher.run(&plan, executor.clone(), None).await.unwrap();
    let validator_ids: Vec<_> = report
        .children
        .iter()
        .map(|c| c.child_id.as_str())
        .filter(|id| id.contains("v-scrutiny") || id.contains("v-user"))
        .collect();
    assert_eq!(
        validator_ids.len(),
        2,
        "scrutiny + user-testing at the boundary: {validator_ids:?}"
    );

    // OFF: no validator children at all.
    let mut cfg_off = common::config(12, 2, 640_000);
    cfg_off.validators = false;
    let plan_off = resolve(
        "val",
        &spec,
        &common::registry(),
        &common::catalog(),
        &common::profiles(),
        &cfg_off,
    )
    .unwrap();
    let dir2 = tempfile::tempdir().unwrap();
    let jpath2 = dir2.path().join("val2.jsonl");
    let journal2 = TeamJournal::open(&jpath2).unwrap();
    let executor2 = Arc::new(common::FakeExecutor::new());
    for job in plan_off.waves.iter().flatten() {
        executor2.ok_child(&job.child_id, "done");
    }
    let mut launcher2 = Launcher::new(cfg_off, Arc::new(MemoryEventSink::new()), journal2);
    common::grant_fresh(&mut launcher2, &plan_off);
    let report2 = launcher2
        .run(&plan_off, executor2.clone(), None)
        .await
        .unwrap();
    assert_eq!(report2.children.len(), 3, "config-skippable: no validators");
    assert!(load_records(&jpath2).unwrap().iter().all(|r| !matches!(
        r,
        austringer_orchestration::journal::JournalRecord::ChildStarted { child, .. } if child.contains("v-scrutiny")
    )));

    // The validator job builders produce both kinds, read-only.
    let wave_jobs: Vec<_> = plan.waves[0].iter().collect();
    let validators = validator_jobs(&wave_jobs, "val");
    let kinds: Vec<ValidatorKind> = validators.iter().map(|(k, _)| *k).collect();
    assert!(
        kinds.contains(&ValidatorKind::Scrutiny) && kinds.contains(&ValidatorKind::UserTesting)
    );
    assert!(validators.iter().all(|(_, job)| job.writes.is_empty()));
}

/// CLEAN-CONTEXT REVIEWER: zero shared context — the reviewer's input is
/// the diff manifest alone; the parent's objectives never leak in.
#[test]
fn clean_context_reviewer_reasons_from_diff_alone() {
    let review = CleanContextReview::from_artifacts(
        "src/lib.rs +3 -1 b3f9\nsrc/launch.rs +40 -0 a11c\n",
        "SECRECY-MARKER-parent-objective-deliver-the-launcher",
    );
    let job = review.reviewer_job("rv", 1024);
    let objective = &job.objective;
    assert!(
        objective.contains("src/lib.rs +3 -1"),
        "diff content is the input"
    );
    assert!(
        !objective.contains("SECRECY-MARKER"),
        "zero shared context: parent objective must not leak: {objective}"
    );
    assert!(
        job.fork.system_prompt.contains("review")
            || job.fork.system_prompt.to_lowercase().contains("doctrine"),
        "reviewer runs on the review doctrine, not the parent prompt"
    );
    assert!(
        !review.parent_context_present(),
        "structured proof the parent context is absent"
    );
}

/// SYNTHESIS with cross-consistency: duplicate artifacts dedup; divergent
/// content on the same path is an inconsistency naming both children;
/// per-agent telemetry lines roll up.
#[test]
fn synthesis_merges_dedups_and_flags_inconsistencies() {
    let mk = |child: &str, artifacts: Vec<(&str, &str)>| ChildReport {
        child_id: child.into(),
        summary: format!("{child} summary"),
        artifacts: artifacts
            .into_iter()
            .map(|(path, hash)| Artifact {
                path: path.into(),
                content_hash: hash.into(),
                source: std::path::PathBuf::from("nowhere"),
            })
            .collect(),
        telemetry: ChildTelemetry {
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            cost_micro_usd: 10,
            duration_ms: 25,
            failure_class: None,
        },
        touches: Vec::new(),
    };

    let reports = vec![
        mk("l-child-t1", vec![("docs/map.md", "hash-a")]),
        mk(
            "l-child-t2",
            vec![("docs/map.md", "hash-a"), ("src/lib.rs", "hash-b")],
        ),
        mk("l-child-t3", vec![("src/lib.rs", "hash-c")]),
    ];
    let report = synthesize(&reports);
    assert_eq!(
        report.artifacts.len(),
        1,
        "duplicate deduped, conflict withheld"
    );
    assert_eq!(report.inconsistencies.len(), 1, "divergent content flagged");
    let inconsistency = &report.inconsistencies[0];
    assert_eq!(inconsistency.topic, "src/lib.rs");
    let children: Vec<&str> = inconsistency
        .positions
        .iter()
        .map(|p| p.child.as_str())
        .collect();
    assert!(children.contains(&"l-child-t2") && children.contains(&"l-child-t3"));
    assert_eq!(report.per_agent.len(), 3, "per-agent telemetry");
    assert!(report.per_agent.iter().all(|l| l.tokens == 1500));
    assert!(
        report.merged_summary.contains("l-child-t3 summary"),
        "summaries merged"
    );

    let _ = BoostPhase::Strategy; // ladder vocabulary stays exported
}
