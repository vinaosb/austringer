//! The compaction engine: hooks, epochs, telemetry, seal, cancel
//! (plan todo 13).

mod common;

use std::sync::Arc;

use austringer_core::agent_loop::CancelToken;
use austringer_core::compaction::condense::ModelId;
use austringer_core::compaction::engine::{
    CompactionCause, CompactionDeps, CompactionEngine, CompactionInput, CompactionSink,
};
use austringer_core::compaction::segments::SegmentStore;
use austringer_core::compaction::telemetry::{
    SealBreach, SealGuard, SealStatus, TelemetryOutcome, ThresholdBasis,
};
use austringer_core::compaction::{CompactionConfig, SegmentDetail, Threshold};
use austringer_core::context::EpochTransition;
use austringer_core::event::{Event, RecordedEvent, Sequence};
use common::compactionkit::{
    CancelSeam, MockExtractor, RecordingSink, ScriptedCondenser, facts_session, session_condenser,
    session_model, session_tool_returns,
};

fn store(name: &str) -> SegmentStore {
    SegmentStore::new(common::TempSession::new(name).path())
}

fn engine(detail: SegmentDetail) -> CompactionEngine {
    CompactionEngine::new(CompactionConfig {
        segment_detail: detail,
        produce_handoff: true,
        ..CompactionConfig::default()
    })
}

#[tokio::test]
async fn doctrine_compaction_emits_hooks_epoch_and_telemetry_in_order() {
    let events = facts_session();
    let table = session_tool_returns(&events);
    let model = session_model();
    let segment_store = store("engine-happy");
    let engine = engine(SegmentDetail::Standard);
    let mut sink = RecordingSink::after(&events);
    let cancel = CancelSeam::new();
    let condenser = session_condenser(&model, Arc::new(ScriptedCondenser));
    let mut deps = CompactionDeps {
        sink: &mut sink,
        condenser: Some(condenser),
        extractor: Some(Arc::new(MockExtractor)),
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
        .expect("doctrine compaction succeeds");

    // Hook order: PreCompact (System) -> PostCompact (Condensation) ->
    // telemetry (System marker).
    let kinds: Vec<&str> = sink
        .appended
        .iter()
        .map(|event| match event {
            Event::System { text } if text.starts_with("<pre_compact ") => "pre",
            Event::System { text } if text.starts_with("<compaction_telemetry") => "telemetry",
            Event::Condensation { .. } => "post",
            _ => "other",
        })
        .collect();
    assert_eq!(
        kinds,
        vec!["pre", "post", "telemetry"],
        "hook order is exact"
    );
    assert!(
        !sink
            .appended
            .iter()
            .any(|event| matches!(event, Event::Condensation { .. }
            if event == &Event::Condensation { summary: String::new(), through_seq: Sequence(0) })),
        "placeholder condensation never appended"
    );

    // Epoch transition (T11) rides the outcome for the ledger.
    assert_eq!(
        result.epoch_transition,
        Some(EpochTransition::CompactionCompleted {
            through_seq: Sequence(36)
        })
    );

    // Telemetry: outcome auto, basis percent, honest before/after counts.
    assert!(matches!(result.telemetry.outcome, TelemetryOutcome::Auto));
    assert!(matches!(
        result.telemetry.basis,
        ThresholdBasis::Percent(75)
    ));
    assert!(result.telemetry.before_tokens > 0);
    assert!(result.telemetry.after_tokens > 0);
    assert!(result.telemetry.after_tokens < result.telemetry.before_tokens);
    assert!(!result.telemetry.fallback_used);

    // Doctrine summary content, not counts.
    let compacted = result
        .compacted
        .as_ref()
        .expect("compacted session present");
    let context = compacted.render_context();
    assert!(context.contains("the API port is 8085"));
    assert!(context.contains("Postgres 15"));
    assert!(context.contains("checkout returns 500"));

    // Seal guard starts intact with the session watermark.
    let guard = result.seal_guard.as_ref().expect("seal guard present");
    assert_eq!(guard.watermark(), Sequence(36));
    assert_eq!(guard.status(), SealStatus::Intact);

    // Segments + handoff produced.
    assert!(!result.segments.is_empty(), "segment files written");
    assert!(result.handoff.is_some(), "handoff bundle produced");

    // Milestone detected from the event stream.
    assert!(result.milestone.is_some());
}

#[tokio::test]
async fn manual_cause_is_the_typed_compact_entry() {
    let events = facts_session();
    let table = session_tool_returns(&events);
    let model = session_model();
    let segment_store = store("engine-manual");
    let engine = engine(SegmentDetail::Minimal);
    let mut sink = RecordingSink::after(&events);
    let cancel = CancelSeam::new();
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
        used_tokens: 10_000,
        window_tokens: 200_000,
    };
    let result = engine
        .compact(input, CompactionCause::Manual, &mut deps)
        .await
        .expect("manual compaction succeeds below threshold too");
    assert!(matches!(result.telemetry.outcome, TelemetryOutcome::Manual));
    let pre = &sink.appended[0];
    assert!(
        matches!(pre, Event::System { text } if text.contains("cause=\"manual\"")),
        "the pre-compact marker names the manual cause: {pre:?}"
    );
}

#[test]
fn threshold_basis_evaluation_is_exact() {
    assert!(Threshold::Percent(75).fired(150_001, 200_000).is_some());
    assert!(
        Threshold::Percent(75).fired(150_000, 200_000).is_some(),
        "fires at the boundary"
    );
    assert!(Threshold::Percent(75).fired(149_999, 200_000).is_none());
    assert!(Threshold::TokensUsed(1_000).fired(1_000, 9_999).is_some());
    assert!(Threshold::TokensUsed(1_000).fired(999, 9_999).is_none());
    assert!(
        Threshold::Remaining(5_000)
            .fired(195_000, 200_000)
            .is_some()
    );
    assert!(
        Threshold::Remaining(5_000)
            .fired(194_999, 200_000)
            .is_none()
    );
    let engine = CompactionEngine::new(CompactionConfig::default());
    assert_eq!(
        engine.should_compact(180_000, 200_000),
        Some(ThresholdBasis::Percent(75))
    );
    assert_eq!(engine.should_compact(10_000, 200_000), None);
}

#[tokio::test]
async fn cancel_mid_compaction_discards_state_and_records_canceled() {
    let events = facts_session();
    let table = session_tool_returns(&events);
    let model = session_model();
    let segment_store = store("engine-cancel");
    let engine = engine(SegmentDetail::Minimal);
    let mut sink = RecordingSink::after(&events);
    let cancel = CancelToken::new();
    cancel.cancel();
    let condenser = session_condenser(&model, Arc::new(ScriptedCondenser));
    let mut deps = CompactionDeps {
        sink: &mut sink,
        condenser: Some(condenser),
        extractor: None,
        segments: &segment_store,
        cancel: &cancel,
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
        .expect("cancel is a clean outcome, not an error");
    assert!(result.compacted.is_none(), "no compacted session on cancel");
    assert!(matches!(
        result.telemetry.outcome,
        TelemetryOutcome::Canceled
    ));
    assert!(result.segments.is_empty(), "no segment artifacts survive");
    assert!(
        !segment_store.compaction_dir().exists(),
        "directory untouched"
    );
    // Clean restartability: a fresh run on the same store succeeds.
    drop(deps);
    let mut sink2 = RecordingSink::after(&events);
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
        events: &events,
        tool_returns: &table,
        model: &model,
        used_tokens: 160_000,
        window_tokens: 200_000,
    };
    let rerun = engine
        .compact(input2, CompactionCause::Auto, &mut deps2)
        .await
        .expect("restart after cancel succeeds");
    assert!(rerun.compacted.is_some());
}

#[test]
fn seal_guard_flags_sealed_region_edits_with_seq_range() {
    let mut guard = SealGuard::new(1, Sequence(36));
    let live = RecordedEvent {
        seq: Sequence(40),
        event: Event::User {
            blocks: vec![austringer_core::event::content::ContentBlock::Text {
                text: "new turn".to_owned(),
            }],
        },
    };
    assert!(guard.observe(std::slice::from_ref(&live)).is_empty());
    assert_eq!(guard.status(), SealStatus::Intact);
    let sealed_rewrite = RecordedEvent {
        seq: Sequence(12),
        event: Event::User {
            blocks: vec![austringer_core::event::content::ContentBlock::Text {
                text: "rewritten history".to_owned(),
            }],
        },
    };
    let breaches = guard.observe(std::slice::from_ref(&sealed_rewrite));
    assert_eq!(breaches.len(), 1);
    let SealBreach {
        compaction_id,
        watermark,
        offending,
    } = &breaches[0];
    assert_eq!(*compaction_id, 1);
    assert_eq!(*watermark, Sequence(36));
    assert_eq!(*offending, (Sequence(12), Sequence(12)));
    assert_eq!(guard.status(), SealStatus::Breached);
    let marker = breaches[0].render();
    assert!(marker.starts_with("<seal_breach "));
    assert!(marker.contains("offending=\"12..12\""));
}

#[tokio::test]
async fn segment_write_failure_records_failed_telemetry_and_errors() {
    let events = facts_session();
    let table = session_tool_returns(&events);
    let model = session_model();
    // A FILE where the compaction DIRECTORY must go: every write fails.
    let blocked_root = common::TempSession::new("engine-blocked");
    let index_path = blocked_root.path().join("compaction");
    std::fs::write(&index_path, b"not a directory").expect("blocker file");
    let segment_store = SegmentStore::new(blocked_root.path());
    let engine = engine(SegmentDetail::Minimal);
    let mut sink = RecordingSink::after(&events);
    let cancel = CancelSeam::new();
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
        used_tokens: 160_000,
        window_tokens: 200_000,
    };
    let err = engine
        .compact(input, CompactionCause::Auto, &mut deps)
        .await
        .expect_err("segment failure is a typed error");
    assert!(
        matches!(
            err,
            austringer_core::compaction::CompactionError::Segment(_)
        ),
        "got {err:?}"
    );
    assert!(
        sink.appended.iter().any(|event| matches!(event,
            Event::System { text } if text.contains("outcome=\"failed\""))),
        "a Failed telemetry marker is appended before the error surfaces"
    );
}

#[tokio::test]
async fn same_model_condensing_rejects_model_swaps_typed() {
    let session = ModelId::new("anthropic/claude-x");
    let swapped = ModelId::new("cheap/fast-small");
    let condenser = session_condenser(&session, Arc::new(ScriptedCondenser));
    let events = facts_session();
    let request = austringer_core::compaction::condense::DoctrineCondenseRequest {
        span: &events[..6],
        session_model: &swapped,
        prompt: austringer_core::compaction::condense::DEFAULT_CONDENSE_PROMPT,
    };
    let refusal = condenser
        .condense(request)
        .await
        .expect_err("different-model request must be rejected");
    let austringer_core::compaction::condense::CondenseRefusal::ModelMismatch(mismatch) = refusal
    else {
        panic!("expected ModelMismatch");
    };
    assert_eq!(mismatch.session_model, session);
    assert_eq!(mismatch.requested_model, swapped);
    let reason = mismatch.to_string();
    assert!(
        reason.contains("same-model"),
        "reason documents the doctrine"
    );
    assert!(
        reason.contains("dialect"),
        "reason names the dialect mismatch"
    );

    // Engine level: a condenser registered for another session's model
    // falls back deterministically instead of silently condensing.
    let table = session_tool_returns(&events);
    let segment_store = store("engine-model-swap");
    let engine = engine(SegmentDetail::Minimal);
    let mut sink = RecordingSink::after(&events);
    let cancel = CancelSeam::new();
    let wrong_model = ModelId::new("other/session");
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
        model: &wrong_model,
        used_tokens: 160_000,
        window_tokens: 200_000,
    };
    let result = engine
        .compact(input, CompactionCause::Auto, &mut deps)
        .await
        .expect("mismatch falls back, never blocks");
    assert!(
        result
            .compacted
            .as_ref()
            .is_some_and(|compacted| compacted.fallback_used),
        "the swap lands on the deterministic fallback"
    );
}

#[tokio::test]
async fn event_log_writer_implements_the_sink_seam() {
    let session = common::TempSession::new("engine-sink");
    let mut writer = austringer_core::log::EventLogWriter::open(
        session.path(),
        austringer_core::log::FsyncPolicy::NeverForTests,
    )
    .expect("writer opens");
    let seq = writer
        .append(Event::System {
            text: "probe".to_owned(),
        })
        .expect("append works");
    let sink: &mut dyn CompactionSink = &mut writer;
    let appended = sink
        .append(Event::System {
            text: "via seam".to_owned(),
        })
        .expect("sink append works");
    assert_eq!(appended, seq.next().expect("monotonic"));
}
