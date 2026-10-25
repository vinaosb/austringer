//! ACCEPTANCE happy path (task-31 QA): scripted session → quality pass →
//! distiller proposes a draft → human approval → replay eval passes →
//! ACTIVATED — and the activated skill round-trips through the REAL T30
//! loader (parse, route, invoke) behind the T30 consent gate.

mod forge_common;

use forge_common::{MockDistillModel, good_export, good_proposal};

use austringer_skills::consent::ConsentGate;
use austringer_skills::forge::distill::{LearnOutcome, SkillDistiller};
use austringer_skills::forge::gates::{
    ActivationGate, ApprovalDecision, DecisionOutcome, DraftStore,
};
use austringer_skills::forge::journal::{CorrectionJournal, CorrectionSignal, RedactionFilter};
use austringer_skills::forge::quality::QualityFilter;
use austringer_skills::loader::{LockMode, SkillLoader, Vendor};

#[test]
fn full_loop_session_to_activated_routable_skill() {
    let tmp = tempfile::tempdir().expect("tmp");
    let drafts = DraftStore::new(tmp.path().join("drafts"));
    let skills_root = tmp.path().join("skills");
    let gate = ActivationGate::new(drafts.clone(), skills_root.clone());

    // 1. A session happened; the journal captured one correction.
    let mut journal = CorrectionJournal::new(RedactionFilter::new(&["ACME-INTERNAL-CODENAME"]));
    journal.capture(
        CorrectionSignal::PostOutputEdit {
            target: "RELEASE.md".to_string(),
            edit_summary: "user added the ACME-INTERNAL-CODENAME gate step".to_string(),
        },
        100,
    );

    // 2. Distill (mock cheap model — no live LLM in tests, ever).
    let distiller = SkillDistiller::new(
        MockDistillModel::fixed(good_proposal()),
        QualityFilter::new(0.5),
    );
    let outcome =
        forge_common::block_on(distiller.learn(&good_export(), &journal)).expect("distill runs");
    let draft = match outcome {
        LearnOutcome::Distilled(draft) => draft,
        LearnOutcome::Filtered { reason } => panic!("good session filtered: {reason}"),
    };
    assert!(
        draft.findings.is_empty(),
        "clean proposal, no scanner findings"
    );
    assert_eq!(draft.proposal.spec.name.as_str(), "rust-release-flow");
    assert_eq!(draft.from_version, None, "new skill");

    // 3. Draft is staged in the draft namespace — NOT in the skills dir.
    drafts.stage(&draft).expect("stage");
    assert!(drafts.draft_file(&draft.id).is_file(), "draft persisted");
    let skills_index_before = SkillLoader::new()
        .skills_dir(&skills_root, Vendor::Austringer)
        .lock_mode(LockMode::Off)
        .load()
        .expect("loader sees draft-namespace skills dir empty");
    assert_eq!(skills_index_before.report().parsed, 0);

    // 4. Human approval + eval-before-activate.
    let activated = match gate
        .decide(
            &draft.id,
            ApprovalDecision::Approve {
                approver: "maintainer".into(),
            },
        )
        .expect("decide")
    {
        DecisionOutcome::Activated(activation) => activation,
        other => panic!("expected activation, got {other:?}"),
    };
    assert_eq!(activated.skill.as_str(), "rust-release-flow");
    assert_eq!(activated.version, "1.0.0");
    assert!(
        activated.skill_path.starts_with(&skills_root),
        "SKILL.md written under the skills root: {}",
        activated.skill_path.display()
    );
    assert!(
        !drafts.draft_file(&draft.id).exists(),
        "activated draft leaves the draft namespace"
    );

    // 5. Round-trip through the REAL T30 loader.
    let index = SkillLoader::new()
        .skills_dir(&skills_root, Vendor::Austringer)
        .lock_mode(LockMode::Off)
        .load()
        .expect("real loader loads the activated skill");
    assert_eq!(index.report().parsed, 1);
    let meta = index.meta("rust-release-flow").expect("meta");
    assert_eq!(meta.version.as_deref(), Some("1.0.0"));

    // 6. Routes behind the T30 consent gate (integration with T30 gate).
    let mut consent = ConsentGate::new();
    let warning = ConsentGate::install_warning(meta);
    let pre = index.route_with(
        &austringer_skills::routing::RouteQuery {
            text: "please cut a release of the crate",
            work_path: None,
        },
        Some(&consent),
    );
    assert!(
        pre.pending_consent
            .iter()
            .any(|n| n.as_str() == "rust-release-flow"),
        "inert until the recorded approval flows into the consent gate"
    );
    consent.grant(meta, warning);
    let post = index.route_with(
        &austringer_skills::routing::RouteQuery {
            text: "please cut a release of the crate",
            work_path: None,
        },
        Some(&consent),
    );
    assert!(
        post.matched
            .iter()
            .any(|m| m.name.as_str() == "rust-release-flow"),
        "routes on the originating prompt after consent"
    );
    let negative = index.route_text("fix the flaky windows test", None);
    assert!(
        !negative
            .matched
            .iter()
            .any(|m| m.name.as_str() == "rust-release-flow"),
        "negative prompt does not route"
    );

    // 7. Invoke surfaces the distilled body (progressive disclosure).
    let invoked = index.invoke("rust-release-flow").expect("invoke");
    assert!(invoked.body.contains("cargo test"));
    assert!(invoked.body.contains("Never publish without green tests."));
    assert_eq!(
        invoked.allowed_tools,
        vec!["Read".to_string(), "Bash(cargo:*)".to_string()]
    );
}
