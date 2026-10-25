//! The cache-violation tracker (plan todo 11, jcode SS32).
//!
//! An append-only blake3 chain of the expected stable prefix per turn:
//! each observation compares the expected prefix against the provider's
//! reported cache hit. A shortfall (or a prompt that fails to extend the
//! previous stable prefix) becomes an observable typed ViolationEvent
//! naming the diverged segment and byte offset — never a silent cost
//! spike.

mod common;

use austringer_core::context::{
    CacheReport, CacheReportFeed, CacheViolationTracker, ContextSpec, EffortLevel, EffortSettings,
    InMemoryFeed, PromptBuilder, RenderedPrompt, SegmentName, SlotSegment, TurnObservation,
    ViolationKind,
};
use austringer_core::event::{ContentBlock, ContentHash, Event, RecordedEvent, Sequence};

fn medium() -> EffortSettings {
    EffortSettings {
        level: EffortLevel::Medium,
        thinking_budget: None,
    }
}

fn user_event(seq: u64, text: &str) -> RecordedEvent {
    RecordedEvent {
        seq: Sequence(seq),
        event: Event::User {
            blocks: vec![ContentBlock::Text {
                text: text.to_owned(),
            }],
        },
    }
}

fn base_spec(history: Vec<RecordedEvent>, system: &str) -> ContextSpec {
    ContextSpec {
        system: system.to_owned(),
        base_schema: "{\"core_tools\":[]}".to_owned(),
        rules: vec![SlotSegment {
            id: "focus".to_owned(),
            content: "<focus-chain>\nstatus: 0 open / 0 total\n</focus-chain>".to_owned(),
        }],
        skills: vec![],
        repomap: "map".to_owned(),
        history,
        last_user_message: "go".to_owned(),
        effort: medium(),
        beta_effort: None,
    }
}

fn render(history: Vec<RecordedEvent>, system: &str) -> RenderedPrompt {
    let spec = base_spec(history, system);
    PromptBuilder::render(&spec).expect("render")
}

fn observe_healthy(
    tracker: &mut CacheViolationTracker,
    turn: u64,
    previous: Option<&RenderedPrompt>,
    rendered: &RenderedPrompt,
) {
    let common = previous.map(|prev| {
        austringer_core::context::common_prefix_len(&prev.prompt, &rendered.prompt) as u64
    });
    let violations = tracker.observe(TurnObservation {
        turn: Sequence(turn),
        rendered,
        common_prefix_with_previous: common,
        report: Some(CacheReport {
            turn: Sequence(turn),
            cache_read_bytes: rendered.stable_prefix_bytes() as u64,
        }),
    });
    assert!(
        violations.is_empty(),
        "turn {turn}: unexpected {violations:?}"
    );
}

#[test]
fn healthy_session_builds_append_only_chain_without_violations() {
    let mut tracker = CacheViolationTracker::new();
    let mut history = vec![];
    let mut previous: Option<RenderedPrompt> = None;
    for turn in 1..=4u64 {
        let rendered = render(history.clone(), "agent");
        observe_healthy(&mut tracker, turn, previous.as_ref(), &rendered);
        previous = Some(rendered);
        history.push(user_event(turn, &format!("u{turn}")));
        history.push(RecordedEvent {
            seq: Sequence(turn),
            event: Event::Assistant {
                blocks: vec![ContentBlock::Text {
                    text: format!("a{turn}"),
                }],
            },
        });
    }
    assert_eq!(tracker.violations().len(), 0);
    let chain = tracker.chain();
    assert_eq!(chain.len(), 4);
    assert_eq!(chain[0].turn, Sequence(1));
    assert_eq!(chain[3].turn, Sequence(4));
    // The chain is append-only: earlier entries survive later turns.
    let first_hash = chain[0].expected_prefix_hash;
    let _ = tracker.observe(TurnObservation {
        turn: Sequence(5),
        rendered: &render(vec![], "agent"),
        common_prefix_with_previous: Some(0),
        report: None,
    });
    assert_eq!(tracker.chain()[0].expected_prefix_hash, first_hash);
    assert_eq!(tracker.chain().len(), 5);
}

#[test]
fn provider_cache_shortfall_names_diverged_segment_and_offset() {
    let mut tracker = CacheViolationTracker::new();
    let first = render(vec![], "agent");
    observe_healthy(&mut tracker, 1, None, &first);
    let grown = render(vec![user_event(1, "u1")], "agent");
    let common = austringer_core::context::common_prefix_len(&first.prompt, &grown.prompt) as u64;
    // The provider only reused bytes up to L0 (rules+ onwards diverged
    // from its perspective).
    let shortfall = grown.map.l0() as u64;
    let violations = tracker.observe(TurnObservation {
        turn: Sequence(2),
        rendered: &grown,
        common_prefix_with_previous: Some(common),
        report: Some(CacheReport {
            turn: Sequence(2),
            cache_read_bytes: shortfall,
        }),
    });
    assert_eq!(violations.len(), 1);
    let event = &violations[0];
    assert_eq!(event.turn, Sequence(2));
    assert_eq!(
        event.kind,
        ViolationKind::CacheMiss {
            expected_bytes: common,
            reported_bytes: shortfall,
        }
    );
    assert_eq!(event.diverged_segment, Some(SegmentName::Rules));
    assert_eq!(event.byte_offset, shortfall as u64);
    // The violation is recorded in the append-only log, not dropped.
    assert_eq!(tracker.violations().len(), 1);
}

#[test]
fn chain_break_when_prompt_fails_to_extend_previous_stable_prefix() {
    let mut tracker = CacheViolationTracker::new();
    let first = render(vec![], "agent");
    observe_healthy(&mut tracker, 1, None, &first);
    // A frozen mutation shrinks the common prefix below the previous
    // stable-prefix length: the layout law itself was broken.
    let mutated = render(vec![], "agent with a changed system segment");
    let common = austringer_core::context::common_prefix_len(&first.prompt, &mutated.prompt) as u64;
    assert!(common < first.stable_prefix_bytes() as u64);
    let violations = tracker.observe(TurnObservation {
        turn: Sequence(2),
        rendered: &mutated,
        common_prefix_with_previous: Some(common),
        report: None,
    });
    assert_eq!(violations.len(), 1);
    let ViolationKind::ChainBreak {
        expected_common,
        actual_common,
    } = violations[0].kind
    else {
        panic!("expected ChainBreak, got {:?}", violations[0].kind);
    };
    assert_eq!(expected_common, first.cacheable_prefix_bytes() as u64);
    assert_eq!(actual_common, common);
    assert_eq!(violations[0].byte_offset, common);
    assert!(violations[0].diverged_segment.is_some());
}

#[test]
fn first_observation_establishes_chain_without_cold_start_violation() {
    let mut tracker = CacheViolationTracker::new();
    let first = render(vec![], "agent");
    // A cold cache reads zero bytes on the very first turn; that is the
    // baseline write, not a violation.
    let violations = tracker.observe(TurnObservation {
        turn: Sequence(1),
        rendered: &first,
        common_prefix_with_previous: None,
        report: Some(CacheReport {
            turn: Sequence(1),
            cache_read_bytes: 0,
        }),
    });
    assert!(violations.is_empty());
    assert_eq!(tracker.chain().len(), 1);
    assert_eq!(
        tracker.chain()[0].expected_prefix_hash,
        first.stable_prefix_hash
    );
    assert_eq!(tracker.chain()[0].cache_read_bytes, Some(0));
}

#[test]
fn in_memory_feed_yields_reports_in_order_then_none() {
    let reports = vec![
        CacheReport {
            turn: Sequence(1),
            cache_read_bytes: 0,
        },
        CacheReport {
            turn: Sequence(2),
            cache_read_bytes: 900,
        },
    ];
    let mut feed: Box<dyn CacheReportFeed> = Box::new(InMemoryFeed::new(reports));
    assert_eq!(feed.next_report().map(|r| r.turn), Some(Sequence(1)));
    assert_eq!(feed.next_report().map(|r| r.cache_read_bytes), Some(900));
    assert!(feed.next_report().is_none());
    assert!(feed.next_report().is_none());
}

#[test]
fn tracker_and_events_serialize_stably() {
    let mut tracker = CacheViolationTracker::new();
    let first = render(vec![], "agent");
    observe_healthy(&mut tracker, 1, None, &first);
    let grown = render(vec![user_event(1, "u1")], "agent");
    let common = austringer_core::context::common_prefix_len(&first.prompt, &grown.prompt) as u64;
    let _ = tracker.observe(TurnObservation {
        turn: Sequence(2),
        rendered: &grown,
        common_prefix_with_previous: Some(common),
        report: Some(CacheReport {
            turn: Sequence(2),
            cache_read_bytes: 10,
        }),
    });
    let encoded = serde_json::to_string(&tracker).expect("serialize tracker");
    let restored: CacheViolationTracker = serde_json::from_str(&encoded).expect("restore");
    assert_eq!(restored.chain().len(), 2);
    assert_eq!(restored.violations().len(), 1);
    assert_eq!(
        restored.chain()[0].expected_prefix_hash,
        ContentHash::from_bytes(&first.prompt.as_bytes()[..first.map.l2()])
    );
}
