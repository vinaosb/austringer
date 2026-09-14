//! BEST-OF-N multi-version runs (up to 4 parallel same-prompt
//! implementations with per-version models) and PLAN-BRANCH model
//! comparison (same spec on different models). The winner's DIFF is
//! applied from its worktree, not just reported (Qwen Arena).

mod common;

use std::sync::Arc;

use harness_orchestration::events::MemoryEventSink;
use harness_orchestration::journal::TeamJournal;
use harness_orchestration::knobs::parse_model_bracket;
use harness_orchestration::launch::{
    ChildCapabilities, ChildFailureClass, ChildReport, ChildTelemetry, LaunchConfig,
    SubagentExecutor,
};
use harness_orchestration::variants::{
    BestOfN, PlanBranch, VariantError, apply_winner, run_best_of_n, run_plan_branch,
};

use harness_orchestration::launch::{ChildFailure, ChildJob};

/// A scoring function is injected; the fixture scores by output tokens.
fn score_by_output(report: &ChildReport) -> u32 {
    report.telemetry.output_tokens as u32
}

/// An executor whose children WRITE REAL FILES into per-version worktrees
/// and report them as artifacts (winner-diff application must move bytes).
struct WorktreeExecutor {
    root: std::path::PathBuf,
    outputs: std::collections::BTreeMap<String, u64>,
}

#[async_trait::async_trait]
impl SubagentExecutor for WorktreeExecutor {
    async fn run(&self, job: ChildJob) -> Result<ChildReport, ChildFailure> {
        let out_tokens = *self.outputs.get(&job.child_id).unwrap_or(&100);
        let dir = self.root.join(&job.child_id);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("launcher.md");
        std::fs::write(
            &file,
            format!("implementation by {}\n", job.knobs.model.model),
        )
        .unwrap();
        Ok(ChildReport {
            child_id: job.child_id.clone(),
            summary: format!("done on {}", job.knobs.model.model),
            artifacts: vec![harness_orchestration::launch::Artifact {
                path: "launcher.md".to_string(),
                content_hash: blake3::hash(
                    format!("implementation by {}\n", job.knobs.model.model).as_bytes(),
                )
                .to_hex()
                .to_string(),
                source: file,
            }],
            telemetry: ChildTelemetry {
                input_tokens: 100,
                output_tokens: out_tokens,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                cost_micro_usd: 5,
                duration_ms: 10,
                failure_class: None,
            },
            touches: Vec::new(),
        })
    }
}

fn fixture_cfg() -> LaunchConfig {
    LaunchConfig {
        validators: false,
        child_capabilities: ChildCapabilities::default(),
        budget_children: 8,
        budget_parallel: 4,
        budget_tokens: 1_000_000,
        ..LaunchConfig::default()
    }
}

/// BEST-OF-N: four versions run in parallel on per-version models; the
/// scored winner's files are applied into the shared scope; a fifth
/// version is a typed error.
#[tokio::test]
async fn best_of_n_runs_versions_and_applies_winners_diff() {
    let root = tempfile::tempdir().unwrap();
    let executor = Arc::new(WorktreeExecutor {
        root: root.path().to_path_buf(),
        outputs: [
            ("bof-child-v0", 100u64),
            ("bof-child-v1", 400),
            ("bof-child-v2", 900),
            ("bof-child-v3", 200),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect(),
    });

    let models = [
        "claude-sonnet-4-6",
        "gpt-5-5",
        "claude-opus-4-7",
        "gemini-3-pro",
    ]
    .iter()
    .map(|m| parse_model_bracket(m).unwrap())
    .collect();
    let bof = BestOfN::new("implement the launcher", models, Arc::new(score_by_output)).unwrap();

    let sink = Arc::new(MemoryEventSink::new());
    let journal = TeamJournal::in_memory();
    let report = run_best_of_n(
        &bof,
        "bof",
        &common::catalog(),
        &common::profiles(),
        executor,
        sink,
        journal,
    )
    .await
    .unwrap();

    assert_eq!(report.runs.len(), 4, "four parallel versions");
    let models: Vec<&str> = report.runs.iter().map(|r| r.model.as_str()).collect();
    assert_eq!(
        models,
        vec![
            "claude-sonnet-4-6",
            "gpt-5-5",
            "claude-opus-4-7",
            "gemini-3-pro"
        ],
        "per-version models"
    );
    assert_eq!(
        report.winner.as_deref(),
        Some("bof-child-v2"),
        "highest output tokens wins"
    );

    // The winner's DIFF is applied from its worktree into the shared scope.
    let scope = root.path().join("shared-scope");
    let winner_report = report
        .runs
        .iter()
        .find(|r| r.child_id == "bof-child-v2")
        .and_then(|r| r.report.as_ref())
        .unwrap();
    let applied = apply_winner(winner_report, &scope).unwrap();
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0].path, "launcher.md");
    let applied_text = std::fs::read_to_string(scope.join("launcher.md")).unwrap();
    assert_eq!(
        applied_text, "implementation by claude-opus-4-7\n",
        "bytes on disk came from the winner"
    );

    // N > 4 is refused.
    let five: Vec<_> = ["a", "b", "c", "d", "e"]
        .iter()
        .map(|_| parse_model_bracket("claude-sonnet-4-6").unwrap())
        .collect();
    assert!(matches!(
        BestOfN::new("p", five, Arc::new(score_by_output)),
        Err(VariantError::TooManyVersions { n: 5, max: 4 })
    ));
    assert!(matches!(
        BestOfN::new("p", vec![], Arc::new(score_by_output)),
        Err(VariantError::NoVersions)
    ));
}

/// PLAN-BRANCH: the same spec text runs to completion on different models
/// and the report compares them branch-by-branch.
#[tokio::test]
async fn plan_branch_compares_models_on_same_spec() {
    let root = tempfile::tempdir().unwrap();
    let executor = Arc::new(WorktreeExecutor {
        root: root.path().to_path_buf(),
        outputs: std::collections::BTreeMap::new(),
    });

    let spec = harness_orchestration::spec::parse_spec(common::THREE_AGENT_SPEC).unwrap();
    let branches = ["claude-sonnet-4-6", "gpt-5-5"]
        .iter()
        .map(|m| parse_model_bracket(m).unwrap())
        .collect();
    let pb = PlanBranch::new(spec, branches);

    let sink = Arc::new(MemoryEventSink::new());
    let report = run_plan_branch(
        &pb,
        "branch",
        &common::registry(),
        &common::catalog(),
        &common::profiles(),
        executor.clone(),
        fixture_cfg(),
        sink,
    )
    .await
    .unwrap();

    assert_eq!(report.per_model.len(), 2, "one run per branch model");
    let models: Vec<&str> = report.per_model.iter().map(|b| b.model.as_str()).collect();
    assert_eq!(models, vec!["claude-sonnet-4-6", "gpt-5-5"]);
    for branch in &report.per_model {
        assert_eq!(branch.children_completed, 3, "whole spec ran on the branch");
        assert!(branch.children_failed == 0);
    }
}

/// Failure classes survive best-of-N bookkeeping.
#[tokio::test]
async fn best_of_n_records_failures() {
    struct FailingExecutor;
    #[async_trait::async_trait]
    impl SubagentExecutor for FailingExecutor {
        async fn run(&self, job: ChildJob) -> Result<ChildReport, ChildFailure> {
            let _ = &job;
            Err(ChildFailure {
                child_id: job.child_id.clone(),
                class: ChildFailureClass::Protocol,
                detail: "child protocol broke".into(),
                telemetry: None,
            })
        }
    }
    let models = vec![parse_model_bracket("claude-sonnet-4-6").unwrap()];
    let bof = BestOfN::new("prompt", models, Arc::new(score_by_output)).unwrap();
    let journal = TeamJournal::in_memory();
    let report = run_best_of_n(
        &bof,
        "bof",
        &common::catalog(),
        &common::profiles(),
        Arc::new(FailingExecutor),
        Arc::new(MemoryEventSink::new()),
        journal,
    )
    .await
    .unwrap();
    assert!(
        report.winner.is_none(),
        "no winner when every version failed"
    );
    assert_eq!(report.runs.len(), 1);
    assert!(report.runs[0].failure.is_some());
    assert!(report.runs[0].report.is_none());
}
