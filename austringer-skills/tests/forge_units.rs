//! Cycle-3 units: /curator lifecycle (injected clock aging), usage
//! analytics + pruning suggestions, knowledge suggestions (regenerable,
//! in-place updates, org→enterprise promotion), closed-loop episodes +
//! SkillReviser, prime-agent base prompt + versioned refinements, and
//! the Hermes five-mechanism enumeration.

use std::collections::BTreeMap;
use std::path::PathBuf;

use austringer_skills::forge::ForgeError;
use austringer_skills::forge::analytics::{PruneReason, UsageAnalytics};
use austringer_skills::forge::document::SkillSection;
use austringer_skills::forge::episodes::{
    Episode, EpisodeStore, PROVEN_REWARD, SkillReviser, SkillUseTrace,
};
use austringer_skills::forge::hermes::HermesMechanism;
use austringer_skills::forge::knowledge::{
    ChatFeedback, KnowledgeOp, KnowledgeScope, KnowledgeStore,
};
use austringer_skills::forge::lifecycle::{
    ARCHIVE_AFTER_DAYS, CuratorAction, LifecycleStage, STALE_AFTER_DAYS, SkillVitals, classify,
    curator_pass,
};
use austringer_skills::forge::prime::{BasePrompt, ContinualHarness};
use austringer_skills::forge::quality::TaskOutcome;

const DAY: u64 = 86_400;

#[test]
fn lifecycle_ages_stale_then_archive_and_pin_protects() {
    let now = 200 * DAY;
    let active = SkillVitals {
        last_invoked_unix: Some(now - 10 * DAY),
        pinned: false,
    };
    let stale = SkillVitals {
        last_invoked_unix: Some(now - (STALE_AFTER_DAYS + 1) * DAY),
        pinned: false,
    };
    let archived = SkillVitals {
        last_invoked_unix: Some(now - (ARCHIVE_AFTER_DAYS + 1) * DAY),
        pinned: false,
    };
    let pinned = SkillVitals {
        last_invoked_unix: Some(now - (ARCHIVE_AFTER_DAYS + 40) * DAY),
        pinned: true,
    };
    assert!(matches!(classify(&active, now), LifecycleStage::Active));
    assert!(matches!(classify(&stale, now), LifecycleStage::Stale));
    assert!(matches!(classify(&archived, now), LifecycleStage::Archived));
    assert!(matches!(classify(&pinned, now), LifecycleStage::Pinned));
    let never = SkillVitals {
        last_invoked_unix: None,
        pinned: false,
    };
    assert!(
        matches!(classify(&never, now), LifecycleStage::Stale),
        "never-invoked skills age to stale, not archived-by-default"
    );

    let mut records = BTreeMap::new();
    records.insert("active-skill".to_string(), active);
    records.insert("stale-skill".to_string(), stale);
    records.insert("old-skill".to_string(), archived);
    records.insert("pinned-skill".to_string(), pinned);
    let actions = curator_pass(&records, now);
    let by_skill: BTreeMap<&str, &CuratorAction> = actions
        .iter()
        .map(|a| (a.skill.as_str(), &a.action))
        .collect();
    assert!(matches!(by_skill["active-skill"], CuratorAction::Keep));
    assert!(matches!(by_skill["stale-skill"], CuratorAction::MarkStale));
    assert!(matches!(by_skill["old-skill"], CuratorAction::Archive));
    assert!(matches!(
        by_skill["pinned-skill"],
        CuratorAction::KeepPinned
    ));
}

#[test]
fn analytics_track_usage_and_suggest_pruning_without_deleting() {
    let mut analytics = UsageAnalytics::new();
    for _ in 0..6 {
        analytics.record_invocation("rarely-useful", 2_000, false);
    }
    analytics.record_invocation("rarely-useful", 2_000, true);
    for _ in 0..10 {
        analytics.record_invocation("workhorse", 500, true);
    }
    for _ in 0..4 {
        analytics.record_invocation("costly-silent", 30_000, false);
    }
    let stats = analytics.stats("rarely-useful").expect("tracked");
    assert_eq!(stats.invocations, 7);
    assert_eq!(stats.hits, 1);
    assert!(stats.hit_rate() < 0.2, "{}", stats.hit_rate());

    let suggestions = analytics.suggest_pruning();
    let find = |name: &str| suggestions.iter().find(|s| s.skill == name);
    let rare = find("rarely-useful").expect("low hit-rate suggested");
    assert!(
        matches!(&rare.reason, PruneReason::LowHitRate { rate } if *rate < 0.2),
        "{:?}",
        rare.reason
    );
    let costly = find("costly-silent").expect("cost-without-value suggested");
    assert!(
        matches!(&costly.reason, PruneReason::CostWithoutValue { tokens_spent } if *tokens_spent >= 100_000),
        "{:?}",
        costly.reason
    );
    assert!(
        find("workhorse").is_none(),
        "healthy skills get no suggestion"
    );
    assert!(
        analytics.stats("rarely-useful").is_some(),
        "suggestions never delete the underlying data"
    );
}

#[test]
fn knowledge_suggestions_update_in_place_and_promote_org_to_enterprise() {
    let mut store = KnowledgeStore::new();
    let feedback = ChatFeedback {
        quote: "always run cargo fmt before committing here".to_string(),
        instruction: "prefer rustfmt over ad-hoc formatting".to_string(),
    };
    let op = store.upsert_from_feedback(&feedback);
    match &op {
        KnowledgeOp::Created { id } => {
            let item = store.get(id).expect("created");
            assert!(matches!(item.scope, KnowledgeScope::Organization));
            assert!(
                item.body.contains("cargo fmt"),
                "regenerable source carried: {}",
                item.source
            );
        }
        other => panic!("first feedback creates: {other:?}"),
    }

    let again = store.upsert_from_feedback(&ChatFeedback {
        quote: feedback.quote.clone(),
        instruction: "updated wording of the rule".to_string(),
    });
    match &again {
        KnowledgeOp::UpdatedInPlace { id } => {
            let count = store.items().count();
            assert_eq!(count, 1, "same quote updates the EXISTING item");
            let item = store.get(id).expect("still there");
            assert!(item.body.contains("updated wording"));
        }
        other => panic!("repeat feedback updates in place: {other:?}"),
    }

    let id = match &again {
        KnowledgeOp::UpdatedInPlace { id } => id.clone(),
        _ => unreachable!(),
    };
    store
        .regenerate(&id, "recomputed from the source quote v2")
        .expect("regenerable");
    assert!(
        store.get(&id).expect("r").body.contains("recomputed"),
        "regeneration replaces the body from source"
    );

    store.promote_to_enterprise(&id).expect("org→enterprise");
    assert!(matches!(
        store.get(&id).expect("p").scope,
        KnowledgeScope::Enterprise
    ));
    let err = store
        .promote_to_enterprise(&id)
        .expect_err("no reverse path");
    assert!(matches!(err, ForgeError::PromotionPath { .. }), "{err:?}");
    let err = store
        .regenerate("missing", "x")
        .expect_err("unknown item typed error");
    assert!(
        matches!(err, ForgeError::KnowledgeNotFound { .. }),
        "{err:?}"
    );
}

#[test]
fn proven_episodes_promote_and_misled_skills_get_fix_drafts() {
    let mut episodes = EpisodeStore::new();
    episodes.record(Episode {
        situation: "cut a release".to_string(),
        trace: vec!["ran cargo test".to_string(), "bumped version".to_string()],
        reward: PROVEN_REWARD + 0.2,
        outcome: TaskOutcome::Success,
    });
    episodes.record(Episode {
        situation: "half-remembered flow".to_string(),
        trace: vec!["guessed".to_string()],
        reward: 0.1,
        outcome: TaskOutcome::Success,
    });
    episodes.record(Episode {
        situation: "broken flow".to_string(),
        trace: vec!["failed".to_string()],
        reward: 0.9,
        outcome: TaskOutcome::Failure {
            reason: "tests red".to_string(),
        },
    });
    let proven = episodes.proven();
    assert_eq!(proven.len(), 1, "only PROVEN episodes promote");
    assert_eq!(proven[0].situation, "cut a release");

    let export = proven[0].clone().into_export();
    assert_eq!(export.reward, PROVEN_REWARD + 0.2);
    assert!(
        export
            .trajectory
            .originating_prompts
            .contains(&"cut a release".to_string())
    );

    let misled = SkillUseTrace {
        skill: "rust-release-flow".to_string(),
        situation: "hotfix one-liner".to_string(),
        misled: true,
        evidence: "skill told the agent to publish; user wanted a dry-run".to_string(),
    };
    let current = vec![
        SkillSection {
            heading: "Steps".into(),
            body: "old".into(),
        },
        SkillSection {
            heading: "Guardrails".into(),
            body: "generic".into(),
        },
    ];
    let delta = SkillReviser::fix_draft(&misled, &current, "1.2.0")
        .expect("fix draft builds")
        .expect("misled trace produces a draft");
    assert_eq!(delta.from_version, "1.2.0");
    assert_eq!(delta.to_version, "1.2.1", "fix drafts bump the version");
    assert_eq!(delta.changes.len(), 1);
    assert!(matches!(
        delta.changes[0],
        austringer_skills::forge::delta::SectionChange::Changed { ref heading, .. }
            if heading == "Guardrails"
    ));
    assert!(
        delta.changes[0].heading().eq("Guardrails")
            && matches!(&delta.changes[0], austringer_skills::forge::delta::SectionChange::Changed { body, .. } if body.contains("hotfix one-liner")),
        "the misleading situation is recorded in the guardrail"
    );

    let fine = SkillUseTrace {
        skill: "rust-release-flow".into(),
        situation: "normal release".to_string(),
        misled: false,
        evidence: String::new(),
    };
    assert!(
        SkillReviser::fix_draft(&fine, &current, "1.2.1")
            .expect("builds")
            .is_none(),
        "non-misleading traces produce no draft"
    );
    let _ = PathBuf::new();
}

#[test]
fn base_prompt_is_immutable_refinements_are_versioned_overlays() {
    let base = BasePrompt::new("You are the release agent. Core duties follow.");
    let mut austringer = ContinualHarness::new(base);
    assert_eq!(
        austringer.base().as_str(),
        "You are the release agent. Core duties follow."
    );
    assert_eq!(austringer.overlays().len(), 0);

    let v1 = austringer
        .refine("PR #42 review thread", "prefer draft releases")
        .expect("refine");
    assert_eq!(v1, 1);
    let v2 = austringer
        .refine("incident 2026-09-01", "never auto-publish")
        .expect("refine");
    assert_eq!(v2, 2, "versions increment");
    assert_eq!(austringer.overlays().len(), 2);
    assert_eq!(austringer.overlays()[0].evidence, "PR #42 review thread");

    let err = austringer
        .refine("", "no evidence")
        .expect_err("evidence required");
    assert!(matches!(err, ForgeError::EvidenceRequired), "{err:?}");

    let rendered = austringer.render();
    assert!(rendered.starts_with("You are the release agent."));
    assert!(rendered.contains("prefer draft releases") && rendered.contains("never auto-publish"));
    assert_eq!(
        austringer.base().as_str(),
        "You are the release agent. Core duties follow."
    );

    austringer.revert(1).expect("revert CRUD");
    assert_eq!(austringer.overlays().len(), 1);
    assert_eq!(austringer.overlays()[0].version, 2);
    let err = austringer.revert(9).expect_err("unknown version");
    assert!(
        matches!(err, ForgeError::RefinementNotFound { .. }),
        "{err:?}"
    );
}

#[test]
fn hermes_loop_is_five_composable_stages_all_gated_by_write_approval() {
    assert_eq!(HermesMechanism::ALL.len(), 5);
    let names: Vec<&str> = HermesMechanism::ALL
        .iter()
        .map(|m| m.stage_name())
        .collect();
    for expected in [
        "in-use improvement",
        "post-turn background review",
        "idle curator",
        "memory nudge",
        "write-approval gate",
    ] {
        assert!(
            names.contains(&expected),
            "missing stage {expected} in {names:?}"
        );
    }
    assert!(
        HermesMechanism::ALL
            .iter()
            .all(|m| m.stage_name() != m.description()),
        "each stage carries a description distinct from its name"
    );
}
