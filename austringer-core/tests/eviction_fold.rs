//! Folding, pointers, return codes, the single volatile error slot, and
//! the pre-compact memory flush (plan todo 12).
//!
//! Under test: Tier2 folding replaces tool outputs with blake3 content
//! pointers + return codes inside parseable `<folded_tool_output>`
//! markers; folding REFUSES (typed) when the return code is unknown —
//! codes are never dropped; the latest error REPLACES the previous in the
//! LLM-visible projection (errors never stack); salient facts flush to
//! the memory hook N tokens BEFORE the compact threshold.

mod common;

use austringer_core::event::content::ContentBlock;
use austringer_core::event::{ContentHash, Event, Sequence};
use austringer_core::eviction::{
    CondensateBlock, ErrorObservation, ErrorSlot, EvictionEngine, EvictionError, FoldedToolOutput,
    ReturnCode, fold_history,
};
use common::evictionkit::{self, EviRng, TestDeps};

#[test]
fn folded_marker_round_trips_and_carries_code_and_pointer() {
    let folded = FoldedToolOutput {
        call_id: "call-7".to_owned(),
        tool: "execute_command".to_owned(),
        content_hash: Some(ContentHash::from_bytes(b"payload")),
        return_code: ReturnCode(127),
        original_bytes: 1_024,
    };
    let marker = folded.render();
    assert!(marker.starts_with("<folded_tool_output "));
    assert!(marker.ends_with("/>"));
    assert!(marker.contains("return_code=\"127\""));
    assert!(marker.contains("content=\"blake3:"));
    let parsed = FoldedToolOutput::parse(&marker).expect("marker parses back");
    assert_eq!(parsed, folded);
}

#[test]
fn folding_never_drops_return_codes_even_across_unicode_payloads() {
    let history = evictionkit::unicode_tool_history(&mut EviRng::new(77), 40);
    let table = evictionkit::tool_returns(&history);
    let folded =
        fold_history(&history, &evictionkit::config(8_000), &table).expect("folding succeeds");
    let mut folded_count = 0;
    for recorded in &folded {
        if let Event::ToolResult { blocks, .. } = &recorded.event {
            for block in blocks {
                let ContentBlock::Text { text: marker } = block else {
                    continue;
                };
                if !marker.starts_with("<folded_tool_output ") {
                    continue;
                }
                folded_count += 1;
                let parsed =
                    FoldedToolOutput::parse(marker).unwrap_or_else(|e| panic!("parse: {e}"));
                let expected = table.0.get(&parsed.call_id).map(|code| code.0);
                assert_eq!(Some(parsed.return_code.0), expected);
                assert!(
                    parsed.content_hash.is_some(),
                    "hard-clear markers carry the blake3 pointer"
                );
                assert_ne!(parsed.original_bytes, 0);
            }
        }
    }
    assert!(folded_count > 0, "the bloated history has foldable results");
}

#[test]
fn folding_refuses_typed_when_a_return_code_is_unknown() {
    let history = evictionkit::unicode_tool_history(&mut EviRng::new(78), 40);
    let mut table = evictionkit::tool_returns(&history);
    let oldest_middle_call = history
        .iter()
        .filter(|recorded| matches!(recorded.event, Event::ToolResult { .. }))
        .find_map(|recorded| match &recorded.event {
            Event::ToolResult { call_id, .. } => Some(call_id.clone()),
            _ => None,
        })
        .expect("fixture has tool results");
    table.0.remove(&oldest_middle_call);
    match fold_history(&history, &evictionkit::config(8_000), &table) {
        Err(EvictionError::MissingReturnCode { call_id }) => {
            assert_eq!(call_id, oldest_middle_call);
        }
        Err(other) => panic!("expected MissingReturnCode, got {other:?}"),
        Ok(_) => panic!("folding must refuse when a code is unknown"),
    }
}

#[test]
fn error_slot_replaces_never_stacks() {
    let mut slot = ErrorSlot::new();
    slot.observe(ErrorObservation {
        seq: Sequence(4),
        code: 500,
        message: "first failure".to_owned(),
    });
    slot.observe(ErrorObservation {
        seq: Sequence(9),
        code: 429,
        message: "second failure".to_owned(),
    });
    slot.observe(ErrorObservation {
        seq: Sequence(15),
        code: 503,
        message: "third failure".to_owned(),
    });
    let rendered = slot.render();
    assert!(rendered.contains("third failure"));
    assert!(rendered.contains("code=\"503\""));
    assert!(!rendered.contains("first failure"));
    assert!(!rendered.contains("second failure"));
    assert_eq!(
        rendered.matches("<volatile_error").count(),
        1,
        "exactly one volatile error block — stacking is a doctrine violation"
    );
    assert_eq!(slot.latest().map(|error| error.seq), Some(Sequence(15)));
}

#[tokio::test]
async fn pre_compact_memory_flush_fires_n_tokens_before_the_threshold() {
    // Window tuned so usage lands strictly between flush-at and trigger:
    // eviction has NOT triggered yet, but the flush must already fire.
    let history = evictionkit::bloated_tool_history(&mut EviRng::new(99), 30);
    let (config, input) = evictionkit::flush_window_input(&history);
    let usage = input.used_tokens();
    let flush_at = config.budget.flush_threshold_tokens();
    let trigger = config.budget.trigger_tokens();
    assert!(
        usage >= flush_at && usage < trigger,
        "fixture must land in the flush band: usage={usage} flush_at={flush_at} trigger={trigger}"
    );
    let mut deps = TestDeps::new();
    let outcome = evictionkit::default_engine()
        .evict(&input, &config, deps.deps())
        .await
        .expect("eviction succeeds");
    assert!(!outcome.triggered, "usage below trigger: no eviction");
    let flush = outcome.memory_flush.as_ref().expect("flush fired");
    assert!(!flush.facts.is_empty());
    assert!(flush.facts.iter().all(|fact| fact.contains("FACT:")));
    assert_eq!(
        deps.memory.flushed.len(),
        1,
        "the hook received exactly one flush"
    );
}

#[test]
fn condensate_block_renders_an_active_plan_style_typed_block() {
    let condensate = CondensateBlock {
        through_seq: Sequence(41),
        summary: "user asked for eviction; tests pass".to_owned(),
        source_hash: ContentHash::from_bytes(b"span"),
    };
    let rendered = condensate.render();
    assert!(rendered.starts_with("<active_plan "));
    assert!(rendered.ends_with("</active_plan>"));
    assert!(rendered.contains("through_seq=\"41\""));
    assert!(rendered.contains("user asked for eviction"));
}
