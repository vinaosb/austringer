//! Cycle-1 delta discipline: forge-emitted documents round-trip through
//! the REAL T30 parser; updates are section deltas (never whole-skill
//! rewrites); versions increment; malformed deltas are typed errors.

use std::path::Path;

use austringer_skills::forge::delta::{
    DeltaError, SectionChange, SkillDelta, apply_delta, next_version,
};
use austringer_skills::forge::document::{
    SkillSection, SkillSpec, extract_body, parse_body_sections, render_document,
};
use austringer_skills::skill::{SkillName, parse_skill_document};

fn spec() -> SkillSpec {
    SkillSpec {
        name: SkillName::new("release-checklist").expect("valid name"),
        description: "Checklist for cutting releases. USE FOR: prepare a release".to_string(),
        version: "1.0.0".to_string(),
        triggers: vec!["prepare a release".to_string()],
        negative_triggers: vec!["hotfix one-liner".to_string()],
        allowed_tools: vec!["Read".to_string(), "Bash(git:*)".to_string()],
    }
}

fn sections() -> Vec<SkillSection> {
    vec![
        SkillSection {
            heading: "Steps".to_string(),
            body: "1. bump version\n2. tag the commit".to_string(),
        },
        SkillSection {
            heading: "Sign-off".to_string(),
            body: "Two maintainers must approve.".to_string(),
        },
    ]
}

#[test]
fn rendered_document_round_trips_through_the_real_parser() {
    let doc = render_document(&spec(), &sections());
    let (meta, warnings) = parse_skill_document(
        Path::new("release-checklist"),
        "release-checklist",
        doc.as_bytes(),
    )
    .expect("forge-emitted document parses via the real T30 parser");
    assert!(
        warnings.iter().all(|w| w.is_unknown_key()),
        "no structural warnings: {warnings:?}"
    );
    assert_eq!(meta.name.as_str(), "release-checklist");
    assert_eq!(meta.version.as_deref(), Some("1.0.0"));
    assert_eq!(
        meta.allowed_tools,
        vec!["Read".to_string(), "Bash(git:*)".to_string()]
    );
    assert_eq!(meta.negative_triggers, vec!["hotfix one-liner".to_string()]);
    assert!(
        meta.triggers.iter().any(|t| t == "prepare a release"),
        "explicit trigger survives: {:?}",
        meta.triggers
    );

    let parsed = parse_body_sections(extract_body(&doc));
    assert_eq!(parsed.len(), 2, "both sections survive: {parsed:?}");
    assert_eq!(parsed[0].heading, "Steps");
    assert_eq!(parsed[0].body, "1. bump version\n2. tag the commit");
    assert_eq!(parsed[1].heading, "Sign-off");
}

#[test]
fn hostile_description_text_is_sanitized_into_a_parseable_document() {
    let mut s = spec();
    s.description = "multi\nline: with colons # and hash".to_string();
    let doc = render_document(&s, &sections());
    let (meta, _) = parse_skill_document(Path::new("x"), "x", doc.as_bytes())
        .expect("newlines/colons in description never break frontmatter");
    assert!(
        meta.description.contains("colons"),
        "description content preserved on one line: {:?}",
        meta.description
    );
    assert!(!meta.description.contains('\n'));
}

#[test]
fn delta_apply_merges_and_preserves_untouched_sections_byte_identical() {
    let delta = SkillDelta {
        skill: "release-checklist".to_string(),
        from_version: "1.0.0".to_string(),
        to_version: "1.0.1".to_string(),
        changes: vec![
            SectionChange::Changed {
                heading: "Steps".to_string(),
                body: "1. bump version\n2. tag\n3. announce".to_string(),
            },
            SectionChange::Added {
                heading: "Rollback".to_string(),
                body: "git revert the tag.".to_string(),
            },
        ],
    };
    let merged = apply_delta(&sections(), &delta).expect("delta applies");
    assert_eq!(merged.len(), 3);
    assert_eq!(merged[0].body, "1. bump version\n2. tag\n3. announce");
    assert_eq!(
        merged[1],
        sections()[1],
        "untouched section preserved structurally identical (no rewrite)"
    );
    assert_eq!(merged[2].heading, "Rollback");
    assert_eq!(
        next_version("1.0.0").as_deref(),
        Ok("1.0.1"),
        "patch bump on every delta"
    );
}

#[test]
fn removal_delta_drops_only_the_named_section() {
    let delta = SkillDelta {
        skill: "release-checklist".to_string(),
        from_version: "1.0.0".to_string(),
        to_version: "1.0.1".to_string(),
        changes: vec![SectionChange::Removed {
            heading: "Sign-off".to_string(),
        }],
    };
    let merged = apply_delta(&sections(), &delta).expect("delta applies");
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].heading, "Steps");
}

#[test]
fn malformed_deltas_are_typed_errors_never_panics() {
    let mk = |changes| SkillDelta {
        skill: "release-checklist".to_string(),
        from_version: "1.0.0".to_string(),
        to_version: "1.0.1".to_string(),
        changes,
    };
    let err = apply_delta(&sections(), &mk(vec![])).expect_err("empty delta");
    assert!(matches!(err, DeltaError::NoChanges), "{err:?}");

    let err = apply_delta(
        &sections(),
        &mk(vec![SectionChange::Changed {
            heading: "Missing".to_string(),
            body: "x".to_string(),
        }]),
    )
    .expect_err("change must target an existing heading");
    assert!(
        matches!(err, DeltaError::HeadingNotFound { ref heading } if heading == "Missing"),
        "{err:?}"
    );

    let err = apply_delta(
        &sections(),
        &mk(vec![SectionChange::Added {
            heading: "Steps".to_string(),
            body: "dup".to_string(),
        }]),
    )
    .expect_err("add must not collide");
    assert!(
        matches!(err, DeltaError::HeadingAlreadyExists { ref heading } if heading == "Steps"),
        "{err:?}"
    );

    let err = apply_delta(
        &sections(),
        &mk(vec![
            SectionChange::Removed {
                heading: "Steps".to_string(),
            },
            SectionChange::Changed {
                heading: "Steps".to_string(),
                body: "x".to_string(),
            },
        ]),
    )
    .expect_err("duplicate change for one heading");
    assert!(matches!(err, DeltaError::DuplicateChange { .. }), "{err:?}");

    let err = apply_delta(
        &sections(),
        &mk(vec![SectionChange::Added {
            heading: String::new(),
            body: "x".to_string(),
        }]),
    )
    .expect_err("empty heading");
    assert!(matches!(err, DeltaError::EmptyHeading), "{err:?}");

    let err = next_version("not-semver").expect_err("bad version");
    assert!(matches!(err, DeltaError::BadVersion { .. }), "{err:?}");
}
