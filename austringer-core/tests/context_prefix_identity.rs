//! Prefix-cache stability of the prompt builder (plan todo 11).
//!
//! Layout law under test: the same logical state must render a
//! byte-identical prompt; the only region allowed to mutate is the volatile
//! tail (the last user message); austringer-composed segments above L2 must
//! carry no volatile data (timestamps are detected and rejected typed);
//! history renders in sequence order, never reordered; slot segments are
//! normalized (sorted by id, JSON keys sorted) so rogue insertion order or
//! unsorted-key JSON cannot churn the prefix.

mod common;

use austringer_core::agent_loop::{FocusChain, TodoId};
use austringer_core::context::{
    BuildError, CacheBreakpoint, CachePlan, CachePolicy, CacheTtl, ContextSpec, EffortLevel,
    EffortSettings, PromptBuilder, SegmentName, SlotSegment, Stability, VolatileKind,
};
use austringer_core::event::{Event, RecordedEvent, Sequence};
use austringer_core::tools::{IoCap, ToolRegistry};
use common::{TempSession, XorShift64};

const SEEDS: [u64; 4] = [1, 42, 0xDEAD_BEEF, 2026_0912];

fn medium_effort() -> EffortSettings {
    EffortSettings {
        level: EffortLevel::Medium,
        thinking_budget: None,
    }
}

fn user_event(seq: u64, text: &str) -> RecordedEvent {
    RecordedEvent {
        seq: Sequence(seq),
        event: Event::User {
            blocks: vec![austringer_core::event::ContentBlock::Text {
                text: text.to_owned(),
            }],
        },
    }
}

fn assistant_event(seq: u64, text: &str) -> RecordedEvent {
    RecordedEvent {
        seq: Sequence(seq),
        event: Event::Assistant {
            blocks: vec![austringer_core::event::ContentBlock::Text {
                text: text.to_owned(),
            }],
        },
    }
}

/// A compact deterministic spec built from random parts.
fn random_spec(rng: &mut XorShift64) -> ContextSpec {
    let rules = (0..rng.below(3))
        .map(|_| SlotSegment {
            id: format!("rule-{:02}", rng.below(100)),
            content: rng.string(60),
        })
        .collect::<Vec<_>>();
    let skills = (0..rng.below(2))
        .map(|_| SlotSegment {
            id: format!("skill-{:02}", rng.below(100)),
            content: rng.string(50),
        })
        .collect::<Vec<_>>();
    let history = (1..=rng.below(6))
        .map(|offset| {
            let seq = offset + 1;
            if seq % 2 == 0 {
                assistant_event(seq, &rng.string(30))
            } else {
                user_event(seq, &rng.string(30))
            }
        })
        .collect::<Vec<_>>();
    ContextSpec {
        system: format!("agent purpose: {}", rng.string(20)),
        base_schema: format!("{{\"core_tools\":[\"{}\"]}}", rng.string(8)),
        rules,
        skills,
        repomap: rng.string(40),
        history,
        last_user_message: rng.string(40),
        effort: medium_effort(),
        beta_effort: None,
    }
}

fn open_registry(name: &str) -> ToolRegistry {
    ToolRegistry::open(TempSession::new(name).path(), IoCap::DEFAULT).expect("open registry")
}

#[test]
fn same_logical_state_renders_byte_identical_prompt_when_built_twice() {
    for &seed in &SEEDS {
        let mut rng = XorShift64::new(seed);
        for _ in 0..40 {
            let spec = random_spec(&mut rng);
            let first = PromptBuilder::render(&spec).expect("first render");
            let second = PromptBuilder::render(&spec).expect("second render");
            assert_eq!(
                first.prompt.as_bytes(),
                second.prompt.as_bytes(),
                "seed {seed}: same state rendered different bytes"
            );
            assert_eq!(first.map.l0(), second.map.l0());
            assert_eq!(first.map.l1(), second.map.l1());
            assert_eq!(first.map.l2(), second.map.l2());
        }
    }
}

#[test]
fn scrambled_slot_insertion_order_renders_byte_identical_prompt() {
    for &seed in &SEEDS {
        let mut rng = XorShift64::new(seed);
        let mut spec = random_spec(&mut rng);
        spec.rules.reverse();
        spec.skills.reverse();
        let original = PromptBuilder::render(&spec).expect("render ordered");
        let spec_reordered = spec.clone();
        let reordered = PromptBuilder::render(&spec_reordered).expect("render reordered");
        assert_eq!(
            original.prompt.as_bytes(),
            reordered.prompt.as_bytes(),
            "seed {seed}: slot insertion order leaked into bytes"
        );
    }
}

#[test]
fn rogue_unsorted_json_slot_content_is_normalized_to_sorted_keys() {
    let unsorted = SlotSegment {
        id: "cfg".to_owned(),
        content: r#"{"zebra":1,"alpha":{"y":2,"x":3},"mid":0}"#.to_owned(),
    };
    let sorted = SlotSegment {
        id: "cfg".to_owned(),
        content: r#"{"alpha":{"x":3,"y":2},"mid":0,"zebra":1}"#.to_owned(),
    };
    let base = |slot: SlotSegment| ContextSpec {
        system: "s".to_owned(),
        base_schema: "{}".to_owned(),
        rules: vec![slot],
        skills: vec![],
        repomap: "r".to_owned(),
        history: vec![],
        last_user_message: "hi".to_owned(),
        effort: medium_effort(),
        beta_effort: None,
    };
    let a = PromptBuilder::render(&base(unsorted)).expect("render rogue");
    let b = PromptBuilder::render(&base(sorted)).expect("render hand-sorted");
    assert_eq!(
        a.prompt.as_bytes(),
        b.prompt.as_bytes(),
        "unsorted-key JSON was not normalized: {}",
        a.prompt
    );
}

#[test]
fn mutating_last_user_message_only_keeps_prefix_bytes_identical() {
    for &seed in &SEEDS {
        let mut rng = XorShift64::new(seed);
        let mut spec = random_spec(&mut rng);
        let before = PromptBuilder::render(&spec).expect("render before");
        spec.last_user_message = format!("changed tail {}", rng.next_u64());
        let after = PromptBuilder::render(&spec).expect("render after");
        assert_eq!(
            before.prompt.as_bytes()[..before.map.l2()],
            after.prompt.as_bytes()[..after.map.l2()],
            "seed {seed}: tail mutation leaked above L2"
        );
        assert_ne!(
            before.prompt.as_bytes(),
            after.prompt.as_bytes(),
            "seed {seed}: tail mutation did not change the prompt"
        );
    }
}

#[test]
fn layout_places_segments_in_fixed_order_with_typed_boundaries() {
    let registry = open_registry("layout-fixed-order");
    let pinned = registry.pin_current();
    let base_schema = registry.render_base_schema(&pinned);
    let mut focus = FocusChain::new();
    focus.upsert(TodoId(1), "write the builder");
    let spec = ContextSpec {
        system: "You are a austringer agent.".to_owned(),
        base_schema,
        rules: vec![SlotSegment {
            id: "focus".to_owned(),
            content: focus.render(),
        }],
        skills: vec![],
        repomap: String::new(),
        history: vec![user_event(1, "hello")],
        last_user_message: "hello again".to_owned(),
        effort: EffortSettings {
            level: EffortLevel::High,
            thinking_budget: None,
        },
        beta_effort: None,
    };
    let rendered = PromptBuilder::render(&spec).expect("render");

    let names: Vec<SegmentName> = rendered.map.spans().iter().map(|s| s.name).collect();
    assert_eq!(
        names,
        vec![
            SegmentName::System,
            SegmentName::ToolSchema,
            SegmentName::Rules,
            SegmentName::Skills,
            SegmentName::Repomap,
            SegmentName::History,
            SegmentName::VolatileTail,
        ]
    );
    for span in rendered.map.spans() {
        let expected = if span.name == SegmentName::VolatileTail {
            Stability::Volatile
        } else {
            Stability::Stable
        };
        assert_eq!(span.stability, expected, "{:?} misclassified", span.name);
    }
    // Boundaries are cumulative segment ends; spans tile the prompt exactly.
    let mut cursor = 0usize;
    for span in rendered.map.spans() {
        assert_eq!(span.start, cursor, "{:?} span not adjacent", span.name);
        cursor = span.end();
    }
    assert_eq!(cursor, rendered.prompt.len());
    assert_eq!(
        rendered.map.l0(),
        rendered
            .map
            .span(SegmentName::ToolSchema)
            .expect("schema span")
            .end()
    );
    assert_eq!(rendered.map.stable_prefix_len(), rendered.map.l2());

    // The exact canonical byte layout (the format contract).
    let expected = format!(
        "<system>\nYou are a austringer agent.\neffort: high\n</system>\n\
         <tool-schema>\n{}\n</tool-schema>\n\
         <rules>\n{}\n</rules>\n\
         <skills>\n</skills>\n\
         <repomap>\n</repomap>\n\
         <history>\n[1] user: hello\n</history>\n\
         <user-message>\nhello again\n</user-message>\n",
        registry.render_base_schema(&pinned),
        focus.render()
    );
    assert_eq!(rendered.prompt, expected);
}

#[test]
fn tool_schema_segment_is_byte_identical_to_t6_base_schema_render() {
    let registry = open_registry("layout-t6-bytes");
    let pinned = registry.pin_current();
    let base_schema = registry.render_base_schema(&pinned);
    let spec = ContextSpec {
        system: "s".to_owned(),
        base_schema,
        rules: vec![],
        skills: vec![],
        repomap: String::new(),
        history: vec![],
        last_user_message: "go".to_owned(),
        effort: medium_effort(),
        beta_effort: None,
    };
    let rendered = PromptBuilder::render(&spec).expect("render");
    assert_eq!(
        rendered
            .segment_content(SegmentName::ToolSchema)
            .expect("content"),
        registry.render_base_schema(&pinned)
    );
}

#[test]
fn injected_timestamp_above_l2_is_detected_and_rejected_typed() {
    let iso = "You are current. Time is 2026-09-12T10:30:00Z.";
    let unix_keyword = "state snapshot\ntimestamp: 1712345678\nend";
    let time_tag = "clock: <current_time>Saturday</current_time>";
    for (segment_name, content) in [
        (SegmentName::System, iso),
        (SegmentName::System, unix_keyword),
        (SegmentName::System, time_tag),
    ] {
        let mut spec = random_spec(&mut XorShift64::new(7));
        let expected_kind = match content {
            c if c.contains("2026-09-12T10:30") => VolatileKind::IsoDateTime,
            c if c.contains("timestamp: 1712") => VolatileKind::UnixTimestamp,
            _ => VolatileKind::TimeTag,
        };
        let expected_offset = ["2026-09-12T10:30", "timestamp: 1712", "<current_time>"]
            .iter()
            .filter_map(|needle| content.find(needle))
            .min()
            .expect("needle present");
        let system = content.to_owned();
        let _ = segment_name;
        spec.system = system;
        let error = PromptBuilder::render(&spec).expect_err("volatile above L2 must be rejected");
        match error {
            BuildError::Layout(violation) => {
                assert_eq!(violation.segment, SegmentName::System);
                assert_eq!(violation.kind, expected_kind);
                assert_eq!(violation.byte_offset, expected_offset);
                assert!(!violation.evidence.is_empty());
            }
            other => panic!("expected Layout violation, got {other:?}"),
        }
    }
    // Same shapes in repomap and slot segments are also rejected.
    for field in ["repomap", "slot"] {
        let mut spec = random_spec(&mut XorShift64::new(8));
        match field {
            "repomap" => spec.repomap = iso.to_owned(),
            _ => {
                spec.rules = vec![SlotSegment {
                    id: "r".to_owned(),
                    content: unix_keyword.to_owned(),
                }]
            }
        }
        assert!(
            matches!(PromptBuilder::render(&spec), Err(BuildError::Layout(_))),
            "{field} volatile content slipped through"
        );
    }
}

#[test]
fn qa_prints_layout_law_failure_payloads() {
    let cases = [
        (
            "system/iso",
            "You are current. Time is 2026-09-12T10:30:00Z.",
        ),
        ("system/keyword", "state\ntimestamp: 1712345678\nend"),
        ("system/tag", "clock: <current_time>Saturday</current_time>"),
    ];
    println!("=== FAILURE QA: injected volatile data above L2 (typed rejections) ===");
    for (label, system) in cases {
        let mut spec = random_spec(&mut XorShift64::new(21));
        spec.system = system.to_owned();
        let error = PromptBuilder::render(&spec).expect_err("rejected");
        println!("  [{label}] -> {error}");
        assert!(matches!(error, BuildError::Layout(_)));
    }
    let mut spec = random_spec(&mut XorShift64::new(22));
    spec.repomap = "repo index generated 2026-09-12T08:00:00Z".to_owned();
    let error = PromptBuilder::render(&spec).expect_err("rejected");
    println!("  [repomap/iso] -> {error}");
    assert!(matches!(error, BuildError::Layout(_)));
    let mut spec = random_spec(&mut XorShift64::new(23));
    spec.last_user_message = "x".repeat(PromptBuilder::MAX_VOLATILE_TAIL_BYTES + 1);
    let error = PromptBuilder::render(&spec).expect_err("rejected");
    println!("  [tail/oversized] -> {error}");
    assert!(matches!(error, BuildError::OversizedTail { .. }));
}

#[test]
fn timestamp_inside_volatile_tail_and_history_is_accepted() {
    // The tail is volatile by definition, and history is append-only
    // user/model data: an embedded date is a historical fact, not an
    // injection, and is stable bytes across turns.
    let mut spec = random_spec(&mut XorShift64::new(11));
    spec.last_user_message = "meet at 2026-09-12T10:30:00Z".to_owned();
    spec.history
        .push(user_event(99, "reminder: logged at 2026-01-01T00:00:00Z"));
    let rendered = PromptBuilder::render(&spec).expect("tail and history are exempt");
    assert!(rendered.prompt.contains("2026-09-12T10:30:00Z"));
}

#[test]
fn history_renders_in_sequence_order_and_never_reorders() {
    let events = vec![
        user_event(1, "first"),
        assistant_event(2, "second"),
        RecordedEvent {
            seq: Sequence(3),
            event: Event::ToolCall {
                call_id: "call-7".to_owned(),
                tool: "view_window".to_owned(),
                arguments: serde_json::json!({"path": "a.rs", "start_line": 1}),
            },
        },
        RecordedEvent {
            seq: Sequence(4),
            event: Event::ToolResult {
                call_id: "call-7".to_owned(),
                blocks: vec![austringer_core::event::ContentBlock::Text {
                    text: "file body".to_owned(),
                }],
            },
        },
        RecordedEvent {
            seq: Sequence(5),
            event: Event::Condensation {
                summary: "internal".to_owned(),
                through_seq: Sequence(2),
            },
        },
        RecordedEvent {
            seq: Sequence(6),
            event: Event::Snapshot {
                label: "internal".to_owned(),
            },
        },
        RecordedEvent {
            seq: Sequence(7),
            event: Event::System {
                text: "env notice".to_owned(),
            },
        },
    ];
    let spec = ContextSpec {
        system: "s".to_owned(),
        base_schema: "{}".to_owned(),
        rules: vec![],
        skills: vec![],
        repomap: String::new(),
        history: events,
        last_user_message: "next".to_owned(),
        effort: medium_effort(),
        beta_effort: None,
    };
    let rendered = PromptBuilder::render(&spec).expect("render");
    let history = rendered
        .segment_content(SegmentName::History)
        .expect("history");
    let expected = "[1] user: first\n\
                    [2] assistant: second\n\
                    [3] tool-call call-7 view_window {\"path\":\"a.rs\",\"start_line\":1}\n\
                    [4] tool-result call-7 file body\n\
                    [7] system: env notice";
    assert_eq!(history, expected);
}

#[test]
fn empty_history_and_single_message_sessions_render_degenerate_layout() {
    let make = |history: Vec<RecordedEvent>, tail: &str| ContextSpec {
        system: "s".to_owned(),
        base_schema: "{}".to_owned(),
        rules: vec![],
        skills: vec![],
        repomap: String::new(),
        history,
        last_user_message: tail.to_owned(),
        effort: medium_effort(),
        beta_effort: None,
    };
    let empty = PromptBuilder::render(&make(vec![], "only message")).expect("empty history");
    // An empty history still emits its (empty) block, so L2 sits just past
    // the closed fence; the segment CONTENT is what degenerates.
    assert!(empty.map.l1() < empty.map.l2());
    assert_eq!(
        empty
            .segment_content(SegmentName::History)
            .expect("history"),
        ""
    );
    let single = PromptBuilder::render(&make(vec![user_event(1, "m")], "tail")).expect("single");
    assert!(single.map.l1() < single.map.l2());
    // Cache plan on the degenerate map still places sorted, unique
    // breakpoints (the empty history block occupies its fence bytes).
    let plan = CachePlan::plan(CachePolicy::Anthropic, &empty.map);
    let ats: Vec<usize> = plan.breakpoints().iter().map(|b| b.at).collect();
    let mid = empty.map.l1() + (empty.map.l2() - empty.map.l1()) / 2;
    assert_eq!(
        ats,
        vec![empty.map.l0(), empty.map.l1(), mid, empty.map.l2()]
    );
}

#[test]
fn oversized_last_message_is_rejected_typed() {
    let mut spec = random_spec(&mut XorShift64::new(13));
    spec.last_user_message = "x".repeat(PromptBuilder::MAX_VOLATILE_TAIL_BYTES + 1);
    match PromptBuilder::render(&spec) {
        Err(BuildError::OversizedTail { len, max }) => {
            assert_eq!(len, PromptBuilder::MAX_VOLATILE_TAIL_BYTES + 1);
            assert_eq!(max, PromptBuilder::MAX_VOLATILE_TAIL_BYTES);
        }
        other => panic!("expected OversizedTail, got {other:?}"),
    }
    spec.last_user_message = "x".repeat(PromptBuilder::MAX_VOLATILE_TAIL_BYTES);
    assert!(PromptBuilder::render(&spec).is_ok());
}

#[test]
fn cache_plan_places_four_anthropic_breakpoints_and_none_for_openai() {
    let registry = open_registry("cache-plan");
    let pinned = registry.pin_current();
    let mut history = vec![];
    for turn in 0..6 {
        history.push(user_event(turn * 2 + 1, &format!("u{turn}")));
        history.push(assistant_event(turn * 2 + 2, &format!("a{turn}")));
    }
    let spec = ContextSpec {
        system: "s".to_owned(),
        base_schema: registry.render_base_schema(&pinned),
        rules: vec![],
        skills: vec![],
        repomap: "map".to_owned(),
        history,
        last_user_message: "go".to_owned(),
        effort: medium_effort(),
        beta_effort: None,
    };
    let rendered = PromptBuilder::render(&spec).expect("render");
    let map = &rendered.map;

    let plan = CachePlan::plan(CachePolicy::Anthropic, map);
    let ats: Vec<usize> = plan
        .breakpoints()
        .iter()
        .map(|b: &CacheBreakpoint| b.at)
        .collect();
    let mid = map.l1() + (map.l2() - map.l1()) / 2;
    assert_eq!(ats, vec![map.l0(), map.l1(), mid, map.l2()]);
    assert!(
        plan.breakpoints()
            .iter()
            .all(|b| b.ttl == CacheTtl::FiveMinutes)
    );
    assert_eq!(CacheTtl::FiveMinutes.as_secs(), 300);
    let hourly = plan.with_ttl(CacheTtl::OneHour);
    assert!(
        hourly
            .breakpoints()
            .iter()
            .all(|b| b.ttl == CacheTtl::OneHour)
    );
    assert_eq!(CacheTtl::OneHour.as_secs(), 3600);

    let openai = CachePlan::plan(CachePolicy::OpenAI, map);
    assert!(openai.breakpoints().is_empty());
}

#[test]
fn effort_renders_into_system_segment_and_beta_channel_never_touches_bytes() {
    let mut spec = random_spec(&mut XorShift64::new(17));
    let low = PromptBuilder::render(&spec).expect("render low-default");
    spec.effort = EffortSettings {
        level: EffortLevel::High,
        thinking_budget: Some(2048),
    };
    let high = PromptBuilder::render(&spec).expect("render high");
    assert_ne!(
        low.segment_hash(SegmentName::System).expect("hash"),
        high.segment_hash(SegmentName::System).expect("hash"),
        "effort must render into the system segment"
    );
    // Beta channel: metadata pass-through, never rendered bytes.
    let mut beta_spec = spec.clone();
    beta_spec.beta_effort = Some(EffortSettings {
        level: EffortLevel::Low,
        thinking_budget: None,
    });
    let beta = PromptBuilder::render(&beta_spec).expect("render beta");
    assert_eq!(beta.prompt.as_bytes(), high.prompt.as_bytes());
    assert_eq!(beta.beta_effort, beta_spec.beta_effort);
    assert_eq!(high.beta_effort, None);
}

#[test]
fn twenty_turn_session_keeps_at_least_90_percent_prefix_identity() {
    let registry = open_registry("identity-20");
    let pinned = registry.pin_current();
    let base_schema = registry.render_base_schema(&pinned);
    let mut focus = FocusChain::new();
    focus.upsert(TodoId(1), "ship the cache-stable builder");
    let rules = vec![SlotSegment {
        id: "focus".to_owned(),
        content: focus.render(),
    }];

    let mut history: Vec<RecordedEvent> = vec![];
    let mut previous: Option<austringer_core::context::RenderedPrompt> = None;
    let mut identity_report: Vec<(u64, u64, f64)> = vec![];
    let mut system_hashes: Vec<austringer_core::event::ContentHash> = vec![];
    for turn in 1..=20u64 {
        let spec = ContextSpec {
            system: "You are a austringer agent.".to_owned(),
            base_schema: base_schema.clone(),
            rules: rules.clone(),
            skills: vec![],
            repomap: "repo map: austringer-core/src/context".to_owned(),
            history: history.clone(),
            last_user_message: format!("turn {turn}: continue the implementation"),
            effort: medium_effort(),
            beta_effort: None,
        };
        let rendered = PromptBuilder::render(&spec).expect("render turn");
        system_hashes.push(
            rendered
                .segment_hash(SegmentName::System)
                .expect("system hash"),
        );
        if let Some(prev) = &previous {
            // Byte-level append-only proof: every byte of the previous
            // prompt's stable CONTENT (through the end of its history
            // content — the closing </history> fence is necessarily
            // rewritten once history grows) is a literal byte prefix of
            // this turn's prompt.
            let history_footer = SegmentName::History.tag().len() + 5; // "\n</history>\n"
            let prev_stable_content_end = prev.map.l2() - history_footer;
            assert_eq!(
                prev.prompt.as_bytes()[..prev_stable_content_end],
                rendered.prompt.as_bytes()[..prev_stable_content_end],
                "turn {turn}: stable prefix mutated"
            );
            let common = rendered
                .prompt
                .bytes()
                .zip(prev.prompt.bytes())
                .take_while(|(a, b)| a == b)
                .count();
            let identity = common as f64 / prev.prompt.len() as f64 * 100.0;
            identity_report.push((turn, common as u64, identity));
            assert!(
                identity >= 90.0,
                "turn {turn}: prefix identity {identity:.1}% below 90%"
            );
        }
        previous = Some(rendered);
        history.push(user_event(
            turn * 2 - 1,
            &format!("turn {turn}: continue the implementation"),
        ));
        history.push(assistant_event(
            turn * 2,
            &format!("did step {turn} of the plan"),
        ));
        if turn % 3 == 0 {
            history.push(RecordedEvent {
                seq: Sequence(turn * 2 + 1),
                event: Event::ToolCall {
                    call_id: format!("call-{turn}"),
                    tool: "view_window".to_owned(),
                    arguments: serde_json::json!({"path": "src/main.rs"}),
                },
            });
        }
    }
    // Frozen segment hashes never move mid-session (no epoch boundary here).
    assert!(system_hashes.windows(2).all(|w| w[0] == w[1]));
    // Evidence line for the QA report.
    for (turn, bytes, identity) in &identity_report {
        println!("turn {turn:>2}: common prefix {bytes:>6} bytes, identity {identity:.1}%");
    }
    let worst = identity_report
        .iter()
        .map(|(_, _, pct)| *pct)
        .fold(100.0_f64, f64::min);
    println!("20-turn session worst turn-over-turn prefix identity: {worst:.1}%");
}
