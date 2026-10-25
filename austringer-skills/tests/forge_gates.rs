//! Trust gates (failure QA): rejection leaves NOTHING; eval failure
//! blocks activation with a typed report; inert (flagged) drafts can
//! never activate; no silent overwrites (existing skills need delta +
//! version, stale drafts are typed errors); no decision = no activation.

mod forge_common;

use forge_common::{MockDistillModel, good_export, good_proposal, injection_proposal};

use austringer_skills::forge::ForgeError;
use austringer_skills::forge::delta::{SectionChange, SkillDelta};
use austringer_skills::forge::distill::{LearnOutcome, SkillDistiller};
use austringer_skills::forge::document::{SkillSection, render_document};
use austringer_skills::forge::gates::{
    ActivationGate, ApprovalDecision, DecisionOutcome, DraftStore,
};
use austringer_skills::forge::journal::{CorrectionJournal, RedactionFilter};
use austringer_skills::forge::quality::QualityFilter;
use austringer_skills::skill::SkillName;
use std::fs;

fn distill_and_stage(
    tmp: &tempfile::TempDir,
    model: MockDistillModel,
) -> austringer_skills::forge::distill::DraftRecord {
    let drafts = DraftStore::new(tmp.path().join("drafts"));
    let distiller = SkillDistiller::new(model, QualityFilter::new(0.5));
    let journal = CorrectionJournal::new(RedactionFilter::new(&[]));
    match forge_common::block_on(distiller.learn(&good_export(), &journal)).expect("learn") {
        LearnOutcome::Distilled(draft) => {
            drafts.stage(&draft).expect("stage");
            *draft
        }
        LearnOutcome::Filtered { reason } => panic!("unexpected filter: {reason}"),
    }
}

#[test]
fn rejected_proposal_leaves_nothing() {
    let tmp = tempfile::tempdir().expect("tmp");
    let draft = distill_and_stage(&tmp, MockDistillModel::fixed(good_proposal()));
    let drafts = DraftStore::new(tmp.path().join("drafts"));
    let gate = ActivationGate::new(drafts.clone(), tmp.path().join("skills"));
    let draft_file = drafts.draft_file(&draft.id);
    assert!(draft_file.is_file());

    let outcome = gate
        .decide(
            &draft.id,
            ApprovalDecision::Reject {
                approver: "maintainer".into(),
                reason: "not generally useful".into(),
            },
        )
        .expect("decide");
    assert!(
        matches!(outcome, DecisionOutcome::RejectedPurged { ref id } if id == &draft.id),
        "{outcome:?}"
    );
    assert!(!draft_file.exists(), "draft file purged");
    assert!(
        drafts.list().expect("list").is_empty(),
        "draft namespace empty"
    );
    let err = drafts.get(&draft.id).expect_err("purged draft is gone");
    assert!(matches!(err, ForgeError::DraftNotFound { .. }), "{err:?}");
    assert!(
        !tmp.path().join("skills").join("rust-release-flow").exists(),
        "rejected proposal leaves NOTHING in the skills tree"
    );
}

#[test]
fn eval_failure_blocks_activation_with_report() {
    let tmp = tempfile::tempdir().expect("tmp");
    // A proposal whose triggers do NOT route on the originating prompts.
    let mut non_routing = good_proposal();
    non_routing.spec.triggers = vec!["completely unrelated trigger".to_string()];
    non_routing.spec.description = "Unrelated helper. USE FOR: totally unrelated work".to_string();
    let draft = distill_and_stage(&tmp, MockDistillModel::fixed(non_routing));
    let gate = ActivationGate::new(
        DraftStore::new(tmp.path().join("drafts")),
        tmp.path().join("skills"),
    );

    let outcome = gate
        .decide(
            &draft.id,
            ApprovalDecision::Approve {
                approver: "m".into(),
            },
        )
        .expect("decide");
    match &outcome {
        DecisionOutcome::Blocked { report } => {
            assert!(!report.failed.is_empty(), "typed failure report");
            let failure = &report.failed[0];
            assert!(
                failure.expected_route,
                "the originating prompt should have routed"
            );
            assert!(!failure.actual_route, "the draft does not route");
            let rendered = report.render();
            assert!(
                rendered.contains("please cut a release of the crate"),
                "report names the failing replay prompt: {rendered}"
            );
        }
        other => panic!("eval must block activation, got {other:?}"),
    }
    // Nothing was written to the skills tree; the draft remains staged.
    assert!(!tmp.path().join("skills").join("rust-release-flow").exists());
    assert!(
        DraftStore::new(tmp.path().join("drafts"))
            .list()
            .expect("list")
            .len()
            == 1
    );
}

#[test]
fn flagged_drafts_are_inert_and_cannot_be_activated() {
    let tmp = tempfile::tempdir().expect("tmp");
    let draft = distill_and_stage(&tmp, MockDistillModel::fixed(injection_proposal()));
    assert!(
        !draft.findings.is_empty(),
        "injection-carrying proposal is flagged by the scanner"
    );
    assert!(draft.is_inert());
    let gate = ActivationGate::new(
        DraftStore::new(tmp.path().join("drafts")),
        tmp.path().join("skills"),
    );
    let err = gate
        .decide(
            &draft.id,
            ApprovalDecision::Approve {
                approver: "m".into(),
            },
        )
        .expect_err("inert drafts never activate");
    assert!(matches!(err, ForgeError::DraftInert { .. }), "{err:?}");
    assert!(!tmp.path().join("skills").exists());
}

#[test]
fn existing_skills_update_only_through_deltas_with_version_checks() {
    let tmp = tempfile::tempdir().expect("tmp");
    let skills_root = tmp.path().join("skills");
    let skill_dir = skills_root.join("rust-release-flow");
    fs::create_dir_all(&skill_dir).expect("mkdir");
    let live = render_document(
        &good_proposal().spec,
        &[
            SkillSection {
                heading: "Steps".into(),
                body: "old steps".into(),
            },
            SkillSection {
                heading: "Untouched".into(),
                body: "keep me".into(),
            },
        ],
    );
    fs::write(skill_dir.join("SKILL.md"), &live).expect("write v1");

    // A draft claiming to be NEW while the skill exists: typed refusal.
    let new_draft = distill_and_stage(&tmp, MockDistillModel::fixed(good_proposal()));
    let gate = ActivationGate::new(
        DraftStore::new(tmp.path().join("drafts")),
        skills_root.clone(),
    );
    let err = gate
        .decide(
            &new_draft.id,
            ApprovalDecision::Approve {
                approver: "m".into(),
            },
        )
        .expect_err("no silent overwrite");
    assert!(
        matches!(err, ForgeError::OverwriteRequiresDelta { .. }),
        "{err:?}"
    );
    assert!(
        fs::read_to_string(skill_dir.join("SKILL.md")).expect("r") == live,
        "existing skill byte-identical after refused overwrite"
    );

    // A stale delta (from_version mismatch): typed refusal.
    let mut stale = good_proposal();
    stale.spec.version = "1.0.1".to_string();
    let stale_draft = austringer_skills::forge::distill::DraftRecord {
        id: "stale-draft".into(),
        proposal: stale,
        from_version: Some("9.9.9".into()),
        findings: vec![],
        replay_cases: vec![],
        created_unix: 1,
        delta: Some(SkillDelta {
            skill: "rust-release-flow".into(),
            from_version: "9.9.9".into(),
            to_version: "9.9.10".into(),
            changes: vec![SectionChange::Changed {
                heading: "Steps".into(),
                body: "x".into(),
            }],
        }),
    };
    DraftStore::new(tmp.path().join("drafts"))
        .stage(&stale_draft)
        .expect("stage");
    let err = gate
        .decide(
            "stale-draft",
            ApprovalDecision::Approve {
                approver: "m".into(),
            },
        )
        .expect_err("stale draft refused");
    assert!(matches!(err, ForgeError::StaleDraft { .. }), "{err:?}");

    // A correct delta: merged (untouched sections preserved), version bumped.
    let mut updated = good_proposal();
    updated.spec.version = "1.0.1".to_string();
    let delta_draft = austringer_skills::forge::distill::DraftRecord {
        id: "delta-draft".into(),
        proposal: updated,
        from_version: Some("1.0.0".into()),
        findings: vec![],
        replay_cases: vec![austringer_skills::forge::eval::ReplayCase {
            prompt: "please cut a release of the crate".into(),
            should_route: true,
            expect_section: Some("Steps".into()),
        }],
        created_unix: 2,
        delta: Some(SkillDelta {
            skill: "rust-release-flow".into(),
            from_version: "1.0.0".into(),
            to_version: "1.0.1".into(),
            changes: vec![SectionChange::Changed {
                heading: "Steps".into(),
                body: "1. cargo test\n2. bump version\n3. cargo publish".into(),
            }],
        }),
    };
    DraftStore::new(tmp.path().join("drafts"))
        .stage(&delta_draft)
        .expect("stage");
    let outcome = gate
        .decide(
            "delta-draft",
            ApprovalDecision::Approve {
                approver: "m".into(),
            },
        )
        .expect("delta activation");
    match &outcome {
        DecisionOutcome::Activated(activation) => assert_eq!(activation.version, "1.0.1"),
        other => panic!("delta should activate, got {other:?}"),
    }
    let merged = fs::read_to_string(skill_dir.join("SKILL.md")).expect("r");
    assert!(merged.contains("cargo publish"), "changed section updated");
    assert!(merged.contains("keep me"), "untouched section preserved");
    assert!(merged.contains("1.0.1"), "version bumped in frontmatter");
}

#[test]
fn approval_is_required_no_decision_means_no_activation() {
    let tmp = tempfile::tempdir().expect("tmp");
    let draft = distill_and_stage(&tmp, MockDistillModel::fixed(good_proposal()));
    assert!(
        !tmp.path().join("skills").join("rust-release-flow").exists(),
        "a staged draft without a decision activates nothing"
    );
    let gate = ActivationGate::new(
        DraftStore::new(tmp.path().join("drafts")),
        tmp.path().join("skills"),
    );
    let err = gate
        .decide(
            "does-not-exist",
            ApprovalDecision::Approve {
                approver: "m".into(),
            },
        )
        .expect_err("unknown draft");
    assert!(matches!(err, ForgeError::DraftNotFound { .. }), "{err:?}");
    // The real draft is still only staged.
    assert!(!tmp.path().join("skills").join("rust-release-flow").exists());
    let _ = draft;
    let _ = SkillName::new("x");
}
