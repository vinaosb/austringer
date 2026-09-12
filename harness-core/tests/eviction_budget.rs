//! Context budget trigger arithmetic and the deterministic token proxy
//! (plan todo 12).

mod common;

use common::evictionkit;
use harness_core::eviction::{ContextBudget, DegenerateInput, estimate_tokens};

#[test]
fn trigger_fires_at_seventy_five_percent_of_the_window_by_default() {
    let budget = ContextBudget::for_window(100_000);
    assert_eq!(budget.window_tokens, 100_000);
    assert_eq!(budget.trigger_percent, 75);
    assert_eq!(budget.trigger_tokens(), 75_000);
    assert_eq!(budget.target_percent, 60);
    assert_eq!(budget.target_tokens(), 60_000);
}

#[test]
fn flush_threshold_sits_n_tokens_before_the_compact_trigger() {
    let budget = ContextBudget::for_window(100_000).with_flush_lead(2_000);
    assert_eq!(budget.flush_lead_tokens, 2_000);
    assert_eq!(budget.flush_threshold_tokens(), 73_000);
    // Lead larger than the trigger head saturates at zero, never wraps.
    let oversized = ContextBudget::for_window(100).with_flush_lead(1_000);
    assert_eq!(oversized.flush_threshold_tokens(), 0);
}

#[test]
fn zero_window_is_the_only_degenerate_budget() {
    assert_eq!(
        ContextBudget::for_window(0).degenerate(),
        Some(DegenerateInput::ZeroWindow)
    );
    assert_eq!(ContextBudget::for_window(1).degenerate(), None);
}

#[test]
fn token_estimate_is_deterministic_and_byte_proportional() {
    assert_eq!(estimate_tokens(""), 0);
    assert_eq!(estimate_tokens("abcd"), 1);
    assert_eq!(estimate_tokens("abcde"), 2);
    // Unicode: byte length drives the estimate, not char count (é = 2
    // bytes each, 4 bytes total → 1 token).
    assert_eq!(estimate_tokens("éé"), 1);
    for text in ["hello world", "перевыполнение", "日本語テキスト"] {
        let first = estimate_tokens(text);
        let second = estimate_tokens(text);
        assert_eq!(first, second);
        assert!(first > 0);
    }
    let _cfg: ContextBudget = evictionkit::config(1_000).budget;
}
