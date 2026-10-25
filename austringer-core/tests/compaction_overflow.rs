//! The overflow-recovery rule — THE critical clause (plan todo 13,
//! Cline lesson): a deterministic basic-compaction fallback that NEVER
//! depends on another successful LLM call.

mod common;

use std::sync::Arc;

use austringer_core::compaction::engine::{
    CompactionCause, CompactionDeps, CompactionEngine, CompactionInput,
};
use austringer_core::compaction::overflow::{ContextOverflow, basic_compact};
use austringer_core::compaction::segments::SegmentStore;
use austringer_core::compaction::{CompactionConfig, SegmentDetail};
use austringer_core::eviction::CondenseError;
use common::compactionkit::{
    CountingCondenser, FailingCondenser, facts_session, seeded_session, session_model,
    session_tool_returns,
};

fn store(name: &str) -> SegmentStore {
    SegmentStore::new(common::TempSession::new(name).path())
}

#[test]
fn basic_compact_is_deterministic_and_carries_doctrine_with_zero_llm() {
    let events = facts_session();
    let table = session_tool_returns(&events);
    let overflow = ContextOverflow::new(210_000, 200_000);
    let compacted = basic_compact(&events, &table, &Default::default(), &overflow);
    assert!(compacted.fallback_used, "the fallback records its nature");
    let context = compacted.render_context();
    assert!(
        context.contains("the API port is 8085"),
        "fallback context carries the port decision"
    );
    assert!(
        context.contains("Postgres 15"),
        "fallback context carries the database decision"
    );
    assert!(
        context.contains("checkout returns 500"),
        "fallback context carries the unresolved bug"
    );
    assert!(
        context.contains("cache TTL is 90 seconds"),
        "fallback context carries the retained recent turn"
    );
    assert!(
        compacted.through_seq.0 == 36,
        "the whole session is sealed through its last event"
    );
    // Determinism: same input, byte-identical output.
    let again = basic_compact(&events, &table, &Default::default(), &overflow);
    assert_eq!(compacted.render_context(), again.render_context());
}

#[tokio::test]
async fn condenser_error_still_produces_a_valid_compacted_session() {
    // THE Cline clause: the condensing model fails; recovery does not.
    let events = facts_session();
    let table = session_tool_returns(&events);
    let model = session_model();
    let segment_store = store("overflow-condenser-err");
    let config = CompactionConfig {
        segment_detail: SegmentDetail::Minimal,
        ..CompactionConfig::default()
    };
    let engine = CompactionEngine::new(config);
    let mut sink = common::compactionkit::RecordingSink::after(&events);
    let cancel = common::compactionkit::CancelSeam::new();
    let condenser = common::compactionkit::session_condenser(&model, Arc::new(FailingCondenser));
    let mut deps = CompactionDeps {
        sink: &mut sink,
        condenser: Some(condenser),
        extractor: None,
        segments: &segment_store,
        cancel: &cancel.token,
    };
    let input = CompactionInput {
        events: &events,
        tool_returns: &table,
        model: &model,
        used_tokens: 180_000,
        window_tokens: 200_000,
    };
    let result = engine
        .compact(input, CompactionCause::Auto, &mut deps)
        .await
        .expect("condenser failure must fall back, not fail");
    let compacted = result.compacted.expect("fallback produced a session");
    assert!(compacted.fallback_used);
    assert!(
        compacted.render_context().contains("the API port is 8085"),
        "zero-LLM recovery still carries the facts"
    );
}

#[tokio::test]
async fn overflow_cause_never_touches_the_condenser_seam() {
    let events = facts_session();
    let table = session_tool_returns(&events);
    let model = session_model();
    let counting = Arc::new(CountingCondenser::new());
    let segment_store = store("overflow-never-llm");
    let engine = CompactionEngine::new(CompactionConfig::default());
    let mut sink = common::compactionkit::RecordingSink::after(&events);
    let cancel = common::compactionkit::CancelSeam::new();
    let condenser = common::compactionkit::session_condenser(&model, counting.clone());
    let mut deps = CompactionDeps {
        sink: &mut sink,
        condenser: Some(condenser),
        extractor: None,
        segments: &segment_store,
        cancel: &cancel.token,
    };
    let input = CompactionInput {
        events: &events,
        tool_returns: &table,
        model: &model,
        used_tokens: 240_000,
        window_tokens: 200_000,
    };
    let result = engine
        .compact(
            input,
            CompactionCause::Overflow(ContextOverflow::new(240_000, 200_000)),
            &mut deps,
        )
        .await
        .expect("overflow compaction succeeds deterministically");
    assert_eq!(
        counting.call_count(),
        0,
        "the LLM seam must never be called"
    );
    let compacted = result.compacted.expect("overflow produced a session");
    assert!(
        compacted.fallback_used,
        "overflow lands on the deterministic path"
    );
}

#[tokio::test]
async fn overflow_with_corrupt_event_stream_is_typed() {
    let mut events = facts_session();
    // Corrupt: sequence jump (5 -> 7 after removing seq 6).
    events.remove(5);
    let table = session_tool_returns(&events);
    let model = session_model();
    let segment_store = store("overflow-corrupt");
    let engine = CompactionEngine::new(CompactionConfig::default());
    let mut sink = common::compactionkit::RecordingSink::after(&events);
    let cancel = common::compactionkit::CancelSeam::new();
    let mut deps = CompactionDeps {
        sink: &mut sink,
        condenser: None,
        extractor: None,
        segments: &segment_store,
        cancel: &cancel.token,
    };
    let input = CompactionInput {
        events: &events,
        tool_returns: &table,
        model: &model,
        used_tokens: 240_000,
        window_tokens: 200_000,
    };
    let err = engine
        .compact(
            input,
            CompactionCause::Overflow(ContextOverflow::new(240_000, 200_000)),
            &mut deps,
        )
        .await
        .expect_err("corrupt stream must be rejected typed");
    assert!(
        matches!(
            err,
            austringer_core::compaction::CompactionError::CorruptStream { .. }
        ),
        "expected CorruptStream, got {err:?}"
    );
}

#[test]
fn fallback_folds_retained_tool_results_via_t12_markers() {
    let events = seeded_session(5, 60);
    let table = session_tool_returns(&events);
    let compacted = basic_compact(
        &events,
        &table,
        &Default::default(),
        &ContextOverflow::new(1, 1),
    );
    let folded = compacted
        .retained
        .iter()
        .filter(|entry| {
            matches!(&entry.event, austringer_core::event::Event::ToolResult { blocks, .. }
                if blocks.iter().any(|b| matches!(b,
                    austringer_core::event::content::ContentBlock::Text { text }
                        if text.starts_with("<folded_tool_output "))))
        })
        .count();
    assert!(folded > 0, "T12 folding applied inside the retained window");
    for entry in &compacted.retained {
        if let austringer_core::event::Event::ToolResult { blocks, .. } = &entry.event {
            for block in blocks {
                if let austringer_core::event::content::ContentBlock::Text { text } = block
                    && text.starts_with("<folded_tool_output ")
                {
                    assert!(
                        text.contains("return_code="),
                        "folded markers never drop return codes"
                    );
                }
            }
        }
    }
}

#[test]
fn seam_error_type_is_reused_from_t12() {
    let error = CondenseError("boom".to_owned());
    assert!(error.to_string().contains("boom"));
}
