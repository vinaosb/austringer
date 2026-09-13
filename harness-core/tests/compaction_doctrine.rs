//! Doctrine selection: what compaction MUST preserve (plan todo 13).
//!
//! Under test: the Anthropic doctrine — architectural decisions,
//! unresolved bugs, ~5 recent files, the last ~10 turns — is a
//! STRUCTURAL selection over the event stream (content extracted
//! verbatim, never counted), subagent summaries are capped at the
//! configured 1–2k-token budget through omission-marked trims (never a
//! bare cut), and the protected head window survives.

mod common;

use common::compactionkit::{facts_session, seeded_session};
use harness_core::compaction::doctrine::{DoctrineConfig, DoctrineSummary, select};
use harness_core::event::{Event, RecordedEvent, Sequence};

#[test]
fn selection_preserves_doctrine_content_verbatim() {
    let events = facts_session();
    let summary = select(&events, &DoctrineConfig::default());
    assert!(
        summary
            .architectural_decisions
            .iter()
            .any(|line| line.contains("the API port is 8085")),
        "the port decision must be carried verbatim, got {:?}",
        summary.architectural_decisions
    );
    assert!(
        summary
            .architectural_decisions
            .iter()
            .any(|line| line.contains("Postgres 15")),
        "the database decision must be carried verbatim"
    );
    assert!(
        summary
            .unresolved_bugs
            .iter()
            .any(|line| line.contains("checkout returns 500")),
        "the unresolved bug must be carried verbatim"
    );
}

#[test]
fn selection_keeps_last_ten_turns_and_protected_head() {
    let events = facts_session();
    let summary = select(&events, &DoctrineConfig::default());
    // The last 10 user-anchored turns of the fixture start at seq 14;
    // the early facts (seq 1..=3) must be OUTSIDE the retained window
    // (they survive via doctrine, not via the tail).
    let first_retained = summary.retained.first().map(|entry| entry.seq.0);
    assert_eq!(first_retained, Some(14), "the retained window is exact");
    assert!(
        summary
            .retained
            .last()
            .is_some_and(|entry| entry.seq == Sequence(36)),
        "retained window ends at the session tail"
    );
    assert!(
        summary.head.iter().any(|entry| entry.seq == Sequence(1)),
        "the protected head keeps the first event"
    );
    assert!(
        summary.retained.iter().any(
            |entry| matches!(&entry.event, Event::ToolCall { tool, .. } if tool == "edit_lines")
        ),
        "retained tail keeps the recent edit"
    );
}

#[test]
fn recent_files_are_the_last_five_unique_paths() {
    let events = facts_session();
    let summary = select(&events, &DoctrineConfig::default());
    assert_eq!(summary.recent_files.len(), 4, "fixture touches 4 files");
    assert_eq!(
        summary.recent_files.last().map(String::as_str),
        Some("src/checkout/mod.rs")
    );
    let unique: std::collections::BTreeSet<_> = summary.recent_files.iter().cloned().collect();
    assert_eq!(unique.len(), summary.recent_files.len(), "no duplicates");
}

#[test]
fn subagent_summaries_are_capped_with_omission_markers_never_bare_cut() {
    let events = facts_session();
    let config = DoctrineConfig {
        subagent_cap_tokens: 1_000,
        ..DoctrineConfig::default()
    };
    let summary = select(&events, &config);
    assert_eq!(summary.subagent_summaries.len(), 1);
    let capped = &summary.subagent_summaries[0];
    assert!(
        capped.contains("<subagent_summary"),
        "the cap keeps the typed opening"
    );
    assert!(
        capped.contains("[trimmed "),
        "the cap is an omission-marked trim, not a bare cut"
    );
    let tokens = (capped.len() as u64).div_ceil(4);
    assert!(
        tokens <= config.subagent_cap_tokens,
        "cap {tokens} exceeds budget"
    );
}

#[test]
fn selection_survives_unicode_and_huge_single_turns_across_seeds() {
    for seed in [7_u64, 42, 9_281] {
        let events = seeded_session(seed, 60);
        let config = DoctrineConfig {
            subagent_cap_tokens: 1_500,
            ..DoctrineConfig::default()
        };
        let summary = select(&events, &config);
        assert!(
            summary
                .architectural_decisions
                .iter()
                .any(|line| line.contains(&format!("seed fact {seed}/"))),
            "seed {seed}: marker decisions survive"
        );
        assert!(
            summary
                .unresolved_bugs
                .iter()
                .any(|line| line.contains("🐛")),
            "seed {seed}: unicode bug markers survive"
        );
        let render = summary.render();
        assert!(render.contains("<doctrine>"), "seed {seed}: typed render");
        let huge = summary
            .retained
            .iter()
            .find(|entry| {
                matches!(&entry.event, Event::User { blocks } if blocks.iter().any(|block|
                    matches!(block, harness_core::event::content::ContentBlock::Text { text }
                        if text.contains("huge turn survives trimmed"))))
            })
            .map(|entry| entry.event.clone())
            .unwrap_or_else(|| panic!("seed {seed}: the huge turn is retained"));
        let huge_text = match huge {
            Event::User { blocks } => blocks
                .iter()
                .filter_map(|block| match block {
                    harness_core::event::content::ContentBlock::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<String>(),
            _ => String::new(),
        };
        assert!(
            huge_text.contains("[trimmed "),
            "seed {seed}: the huge turn is trimmed with an omission marker"
        );
        assert!(
            huge_text.len() < 8_000,
            "seed {seed}: the trimmed turn is bounded"
        );
    }
}

#[test]
fn render_lists_doctrine_sections_with_content_not_counts() {
    let events = facts_session();
    let summary = select(&events, &DoctrineConfig::default());
    let render = DoctrineSummary::render(&summary);
    assert!(render.contains("architectural_decisions"));
    assert!(render.contains("unresolved_bugs"));
    assert!(render.contains("recent_files"));
    assert!(render.contains("the API port is 8085"));
    assert!(render.contains("checkout returns 500"));
    // No naive truncation: the render is the complete doctrine block.
    assert!(render.ends_with("</doctrine>"));
}

#[test]
fn selection_is_total_on_tiny_sessions() {
    let tiny: Vec<RecordedEvent> = vec![RecordedEvent {
        seq: Sequence(1),
        event: Event::User {
            blocks: vec![harness_core::event::content::ContentBlock::Text {
                text: "DECISION: start small".to_owned(),
            }],
        },
    }];
    let summary = select(&tiny, &DoctrineConfig::default());
    assert_eq!(summary.retained.len(), 1);
    assert_eq!(
        summary.architectural_decisions,
        vec!["DECISION: start small".to_owned()]
    );
}
