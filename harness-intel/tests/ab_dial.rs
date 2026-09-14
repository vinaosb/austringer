//! A/B skill-variant selection (deterministic, per-model) with proposals
//! flowing through the Skill Forge gate vocabulary, and the dial
//! effort-bundle grammar (named positions bundle model+effort+prompt+tools).

use std::collections::BTreeMap;

use harness_intel::ab::{AbDecision, VariantOutcome, ab_select, winner_proposals};
use harness_intel::catalog::{Catalog, CatalogError, EffortValue};
use harness_intel::dial::{DialSpec, build_dial};
use harness_skills::forge::distill::DraftRecord;
use harness_skills::forge::document::SkillSpec;
use harness_skills::forge::gates::DraftStore;
use harness_skills::skill::SkillName;

fn outcome(variant: &str, model: &str, score: f64, samples: u32) -> VariantOutcome {
    VariantOutcome {
        variant: variant.to_string(),
        model: model.to_string(),
        score,
        samples,
    }
}

fn base_spec() -> SkillSpec {
    SkillSpec {
        name: SkillName::new("repo-onboarding").expect("valid kebab name"),
        description: "A/B-optimized repo onboarding guidance".to_string(),
        version: "0.2.0".to_string(),
        triggers: vec!["onboard this repo".to_string()],
        negative_triggers: vec![],
        allowed_tools: vec![],
    }
}

#[test]
fn ab_picks_the_higher_scoring_variant_per_model() {
    // Given: variant guidance-a scores 0.4 and guidance-b 0.9 on
    // glm-5-sonnet, while the ordering REVERSES on glm-5-air.
    let outcomes = vec![
        outcome("guidance-a", "glm-5-sonnet", 0.4, 12),
        outcome("guidance-b", "glm-5-sonnet", 0.9, 12),
        outcome("guidance-a", "glm-5-air", 0.8, 9),
        outcome("guidance-b", "glm-5-air", 0.7, 9),
    ];

    // When: selecting winners.
    let decision = ab_select(&outcomes);

    // Then: each model gets ITS OWN winner — there is no global best.
    let by_model: BTreeMap<&str, &str> = decision
        .winners
        .iter()
        .map(|w| (w.model.as_str(), w.winner.as_str()))
        .collect();
    assert_eq!(by_model["glm-5-sonnet"], "guidance-b");
    assert_eq!(by_model["glm-5-air"], "guidance-a");
    // And the runner-up evidence rides along.
    let sonnet = decision
        .winners
        .iter()
        .find(|w| w.model == "glm-5-sonnet")
        .expect("present");
    assert_eq!(sonnet.winner_score, 0.9);
    let runner_up = sonnet.runner_up.as_ref().expect("two variants ran");
    assert_eq!(runner_up.variant, "guidance-a");
    assert_eq!(runner_up.score, 0.4);
}

#[test]
fn ties_break_deterministically_on_variant_id() {
    // Given: an exact score tie.
    let outcomes = vec![
        outcome("guidance-b", "glm-5-sonnet", 0.75, 10),
        outcome("guidance-a", "glm-5-sonnet", 0.75, 10),
    ];

    // When / Then: the lexicographically smallest variant id wins — the same
    // input always produces the same decision.
    let decision = ab_select(&outcomes);
    assert_eq!(decision.winners[0].winner, "guidance-a");
    assert_eq!(
        ab_select(&outcomes).winners[0].winner,
        decision.winners[0].winner
    );
}

#[test]
fn zero_sample_variants_never_win() {
    // Given: a variant with a spectacular score but ZERO samples — an
    // unscored variant is not evidence.
    let outcomes = vec![
        outcome("guidance-a", "glm-5-sonnet", 0.5, 8),
        outcome("guidance-b", "glm-5-sonnet", 0.99, 0),
    ];

    // When / Then: the sampled variant wins.
    let decision = ab_select(&outcomes);
    assert_eq!(decision.winners[0].winner, "guidance-a");
}

#[test]
fn winner_proposals_are_forge_typed_and_flow_through_the_draft_gate() {
    // Given: a completed A/B decision.
    let decision = AbDecision {
        winners: vec![harness_intel::ab::ModelWinner {
            model: "glm-5-sonnet".to_string(),
            winner: "guidance-b".to_string(),
            winner_score: 0.9,
            runner_up: Some(harness_intel::ab::RunnerUp {
                variant: "guidance-a".to_string(),
                score: 0.4,
            }),
            samples: 12,
        }],
    };

    // When: rendering proposals in the Skill Forge's own vocabulary.
    let proposals = winner_proposals(&decision, &base_spec());

    // Then: exactly one proposal per winning model, carrying the winning
    // variant's identity + the numeric evidence in its sections/rationale.
    assert_eq!(proposals.len(), 1);
    let proposal = &proposals[0];
    assert_eq!(proposal.spec.name.as_str(), "repo-onboarding");
    assert!(
        proposal
            .sections
            .iter()
            .any(|s| s.body.contains("guidance-b") && s.body.contains("glm-5-sonnet"))
    );
    assert!(proposal.rationale.contains("0.9"));

    // And the proposal STAGES as a draft through the real forge gate surface
    // — inert by construction: the drafts namespace is never loader-scanned
    // and activation requires ApprovalDecision + passing eval (todo 31).
    let dir = tempfile::tempdir().expect("tempdir");
    let drafts = DraftStore::new(dir.path().join("drafts"));
    let record = DraftRecord {
        id: "ab-repo-onboarding".to_string(),
        proposal: proposal.clone(),
        from_version: Some("0.1.0".to_string()),
        findings: vec![],
        replay_cases: vec![],
        created_unix: 1,
        delta: None,
    };
    drafts.stage(&record).expect("stage");
    let listed = drafts.list().expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].proposal, *proposal);
    // Zero scanner findings + still a DRAFT: nothing activated, no skill
    // file written anywhere in this flow.
    assert!(!listed[0].is_inert());
}

#[test]
fn dial_positions_are_concrete_bundles() {
    // Given: dial specs mixing explicit efforts and catalog-defaulted ones.
    let catalog = Catalog::seeded();
    let specs = vec![
        DialSpec {
            name: "quick".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            effort: None,
            prompt_addenda: vec!["keep updates terse".to_string()],
            tools: vec!["view_window".to_string()],
        },
        DialSpec {
            name: "deep".to_string(),
            model: "claude-opus-4-7".to_string(),
            effort: Some(EffortValue::Xhigh),
            prompt_addenda: vec![],
            tools: vec!["view_window".to_string(), "edit_lines".to_string()],
        },
    ];

    // When: building the dial.
    let positions = build_dial("anthropic", &specs, &catalog).expect("seeded models");

    // Then: every position is concrete — a resolved identity, an EXPLICIT
    // effort (defaulted from the catalog when the spec omitted one, never
    // the provider's implicit default), prompt addenda, and a tool set.
    assert_eq!(positions.len(), 2);
    let quick = &positions[0];
    assert_eq!(quick.model.model, "claude-sonnet-4-6");
    assert_eq!(quick.effort, EffortValue::Medium);
    assert_eq!(quick.prompt_addenda, vec!["keep updates terse".to_string()]);
    let deep = &positions[1];
    assert_eq!(deep.effort, EffortValue::Xhigh);
    assert_eq!(deep.tools.len(), 2);
}

#[test]
fn dial_rejects_unknown_models_and_illegal_efforts() {
    let catalog = Catalog::seeded();

    // Unknown model: typed error, never a guess.
    let unknown = vec![DialSpec {
        name: "broken".to_string(),
        model: "mystery-model".to_string(),
        effort: None,
        prompt_addenda: vec![],
        tools: vec![],
    }];
    let err = build_dial("anthropic", &unknown, &catalog).expect_err("unknown model");
    assert!(matches!(err, CatalogError::UnknownModel(_)));

    // Legal-grid violation: fail closed exactly like a direct wire request.
    let illegal = vec![DialSpec {
        name: "overdrive".to_string(),
        model: "gpt-5-5".to_string(),
        effort: Some(EffortValue::Max),
        prompt_addenda: vec![],
        tools: vec![],
    }];
    let err = build_dial("openai", &illegal, &catalog).expect_err("max not on gpt-5-5 grid");
    assert!(matches!(err, CatalogError::Trap(_)));
}
