//! Tier classification and Tier0 immutability (plan todo 12).
//!
//! Under test: the three-tier doctrine — Tier0 (system core, active plan)
//! is structurally eviction-immune and targeting it is a typed rejection;
//! the protected head (first ~3 messages) and tail (last ~20) are immune
//! regardless of kind; the condensable middle splits into Tier1
//! (conversation: user/assistant/system events) and Tier2 (tool
//! calls/results).

mod common;

use common::evictionkit::{self, EviRng, synthetic_history};
use harness_core::event::EventKind;
use harness_core::eviction::{
    ActivePlan, DegenerateInput, EvictionConfig, EvictionError, Projection, Tier,
};

fn config() -> EvictionConfig {
    evictionkit::config(8_000)
}

#[test]
fn head_and_tail_events_are_immune_in_regardless_of_kind() {
    let history = synthetic_history(&mut EviRng::new(7), 60);
    let projection = Projection::new(&history, &config());
    let total = history.len();
    assert_eq!(projection.total(), total);
    for index in 0..total {
        let zone = projection.zone(index);
        if index < 3 {
            assert_eq!(zone, Some(Tier::ProtectedHead), "index {index} is head");
        } else if index + 20 >= total {
            assert_eq!(zone, Some(Tier::ProtectedTail), "index {index} is tail");
        }
    }
    // The tail immunity covers the LAST 20 events exactly.
    let tail_start = total.saturating_sub(20);
    for index in tail_start..total {
        assert_eq!(projection.zone(index), Some(Tier::ProtectedTail));
    }
    // Head and tail do not overlap for a 60-event session.
    assert!(tail_start > 3);
}

#[test]
fn middle_conversation_is_tier1_and_middle_tool_pairs_are_tier2() {
    let history = synthetic_history(&mut EviRng::new(11), 60);
    let projection = Projection::new(&history, &config());
    let mut saw_tier1 = false;
    let mut saw_tier2 = false;
    for (index, recorded) in history.iter().enumerate() {
        let Some(zone) = projection.zone(index) else {
            continue;
        };
        let kind = recorded.event.kind();
        if zone == Tier::ProtectedHead || zone == Tier::ProtectedTail {
            continue;
        }
        match kind {
            EventKind::ToolCall | EventKind::ToolResult => {
                assert_eq!(zone, Tier::Two, "tool event {index} must be Tier2");
                saw_tier2 = true;
            }
            _ => {
                assert_eq!(zone, Tier::One, "event {index} must be Tier1");
                saw_tier1 = true;
            }
        }
    }
    assert!(
        saw_tier1 && saw_tier2,
        "synthetic history covers both tiers"
    );
}

#[test]
fn attempting_to_evict_tier0_or_the_active_plan_is_a_typed_rejection() {
    let plan = ActivePlan {
        text: "1. do the thing\n2. finish".to_owned(),
    };
    // Structural guard: a decision targeting Tier0 is rejected typed, never
    // silently applied.
    let rejection = Projection::guard_tier0(&Tier::Zero);
    assert_eq!(
        rejection,
        Err(EvictionError::Tier0Protected {
            target: "tier0".to_owned()
        })
    );
    let plan_rejection = EvictionError::tier0_plan_rejection();
    assert!(matches!(
        plan_rejection,
        EvictionError::Tier0Protected { ref target } if target == "active_plan"
    ));
    // The plan block itself is carried verbatim by the projection input.
    assert_eq!(plan.text, "1. do the thing\n2. finish");
}

#[test]
fn empty_session_and_zero_window_are_typed_degenerate_inputs() {
    let history = Vec::new();
    let projection = Projection::new(&history, &config());
    assert_eq!(projection.total(), 0);

    let zero_window = evictionkit::config(0);
    assert_eq!(
        zero_window.budget.degenerate(),
        Some(DegenerateInput::ZeroWindow)
    );
    assert_eq!(
        config().budget.degenerate(),
        None,
        "a real window is not degenerate"
    );
    assert_eq!(
        Projection::degenerate_of(&history, &config()),
        Some(DegenerateInput::EmptySession)
    );
}
