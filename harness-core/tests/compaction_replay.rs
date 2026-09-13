//! Replay-equivalence — the plan's acceptance criterion for todo 13: a
//! compacted session CONTINUES the task without re-asking known facts.
//!
//! Scripted flow: a session establishes facts early (outside the
//! retained tail) -> compaction runs -> the next turn's prompt is built
//! from the compacted context -> a mock inference seam answers a
//! pre-compaction-facts question FROM that prompt, proving the facts
//! were carried. Both paths are pinned: the doctrine-condensed path and
//! the zero-LLM deterministic fallback.

mod common;

use std::sync::Arc;

use common::compactionkit::{
    CancelSeam, FailingCondenser, MockInference, RecordingSink, ScriptedCondenser, facts_session,
    session_condenser, session_model, session_tool_returns,
};
use harness_core::compaction::engine::{
    CompactionCause, CompactionDeps, CompactionEngine, CompactionInput,
};
use harness_core::compaction::segments::SegmentStore;
use harness_core::compaction::{CompactionConfig, SegmentDetail};

async fn compacted_prompt(condenser_fails: bool, name: &str) -> String {
    let events = facts_session();
    let table = session_tool_returns(&events);
    let model = session_model();
    let segment_store = SegmentStore::new(common::TempSession::new(name).path());
    let engine = CompactionEngine::new(CompactionConfig {
        segment_detail: SegmentDetail::Standard,
        ..CompactionConfig::default()
    });
    let mut sink = RecordingSink::after(&events);
    let cancel = CancelSeam::new();
    let inner: Arc<dyn harness_core::compaction::condense::DoctrineCondenser> = if condenser_fails {
        Arc::new(FailingCondenser)
    } else {
        Arc::new(ScriptedCondenser)
    };
    let condenser = session_condenser(&model, inner);
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
        used_tokens: 160_000,
        window_tokens: 200_000,
    };
    let result = engine
        .compact(input, CompactionCause::Auto, &mut deps)
        .await
        .expect("compaction succeeds");
    result
        .compacted
        .expect("compacted session present")
        .render_context()
}

#[tokio::test]
async fn doctrine_compacted_session_answers_pre_compaction_facts() {
    let prompt = compacted_prompt(false, "replay-doctrine").await;
    let seam = MockInference::new(&prompt);
    // The new turn references pre-compaction facts; the answer must come
    // from the compacted context, not from re-asking the user.
    assert!(
        seam.carries("the API port is 8085"),
        "the compacted prompt carries the port fact"
    );
    assert!(seam.carries("Postgres 15"));
    assert!(seam.carries("checkout returns 500"));
    // A fresh fact from the retained tail survives too.
    assert!(seam.carries("cache TTL is 90 seconds"));
}

#[tokio::test]
async fn fallback_compacted_session_still_answers_known_facts() {
    let prompt = compacted_prompt(true, "replay-fallback").await;
    let seam = MockInference::new(&prompt);
    assert!(
        seam.carries("the API port is 8085"),
        "zero-LLM fallback still carries the facts"
    );
    assert!(seam.carries("Postgres 15"));
    assert!(seam.carries("cache TTL is 90 seconds"));
}

#[tokio::test]
async fn second_compaction_of_the_continued_session_still_carries_facts() {
    // Compact, continue with new turns, compact again: the doctrine
    // facts survive the second fold (they live in the summary).
    let events = facts_session();
    let table = session_tool_returns(&events);
    let model = session_model();
    let segment_store = SegmentStore::new(common::TempSession::new("replay-twice").path());
    let engine = CompactionEngine::new(CompactionConfig {
        segment_detail: SegmentDetail::Minimal,
        ..CompactionConfig::default()
    });
    let mut sink = RecordingSink::after(&events);
    let cancel = CancelSeam::new();
    let condenser = session_condenser(&model, Arc::new(ScriptedCondenser));
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
        used_tokens: 160_000,
        window_tokens: 200_000,
    };
    let first = engine
        .compact(input, CompactionCause::Auto, &mut deps)
        .await
        .expect("first compaction");
    let summary = first
        .compacted
        .as_ref()
        .expect("first compacted session")
        .summary
        .clone();

    // The continued session: summary event + fresh turns.
    let mut continued = vec![harness_core::event::RecordedEvent {
        seq: harness_core::event::Sequence(37),
        event: harness_core::event::Event::System {
            text: summary.clone(),
        },
    }];
    for turn in 0..24u64 {
        let seq = 38 + turn * 2;
        continued.push(harness_core::event::RecordedEvent {
            seq: harness_core::event::Sequence(seq),
            event: harness_core::event::Event::User {
                blocks: vec![harness_core::event::content::ContentBlock::Text {
                    text: format!("continuation turn {turn}"),
                }],
            },
        });
        continued.push(harness_core::event::RecordedEvent {
            seq: harness_core::event::Sequence(seq + 1),
            event: harness_core::event::Event::Assistant {
                blocks: vec![harness_core::event::content::ContentBlock::Text {
                    text: format!("continuation answer {turn}"),
                }],
            },
        });
    }
    let table2 = session_tool_returns(&continued);
    let mut sink2 = RecordingSink::after(&continued);
    let cancel2 = CancelSeam::new();
    let condenser2 = session_condenser(&model, Arc::new(ScriptedCondenser));
    let mut deps2 = CompactionDeps {
        sink: &mut sink2,
        condenser: Some(condenser2),
        extractor: None,
        segments: &segment_store,
        cancel: &cancel2.token,
    };
    let input2 = CompactionInput {
        events: &continued,
        tool_returns: &table2,
        model: &model,
        used_tokens: 190_000,
        window_tokens: 200_000,
    };
    let second = engine
        .compact(input2, CompactionCause::Auto, &mut deps2)
        .await
        .expect("second compaction");
    let context = second
        .compacted
        .as_ref()
        .expect("second compacted session")
        .render_context();
    let seam = MockInference::new(&context);
    assert!(
        seam.carries("the API port is 8085"),
        "the fact survives the second fold via the carried summary"
    );
}
