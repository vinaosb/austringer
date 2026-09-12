//! Acceptance scenarios for the budget subsystem (plan todo 25).
//!
//! Scripted spend streams prove the plan's acceptance criterion — a
//! simulated cost stream triggers thresholds EXACTLY (Warning at the first
//! record that crosses ≥80%, Stop at ≥100%, never early, never late) — plus
//! the rollup tree, the zero-ceiling refusal, and the mid-tool stop
//! ordering (a Stop only ever surfaces after the spend that completed the
//! tool is fully recorded). Pacing acceptance lives in `budget_pacing.rs`.

use std::collections::BTreeMap;

use harness_core::budget::{
    BudgetCap, BudgetPeriod, BudgetSignal, BudgetSignalSink, BudgetSpec, Dimension, MicroUsd,
    RollupNode, Spend, SpendLedger, TokenCount,
};

/// Signal sink that records everything it receives (the mock consumer).
#[derive(Default)]
struct Recorder {
    signals: Vec<BudgetSignal>,
}

impl BudgetSignalSink for Recorder {
    fn on_signal(&mut self, signal: BudgetSignal) {
        self.signals.push(signal);
    }
}

/// Test-only: `Ok` extraction without the unwrap/expect/panic verbs —
/// an `Err` prints and yields `None` so the caller's `let-else` returns.
fn ok<T, E: std::fmt::Debug>(r: Result<T, E>) -> Option<T> {
    match r {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("unexpected Ok-expected error: {e:?}");
            None
        }
    }
}

/// Test-only mirror of [`ok`] for `Err` expectations.
fn err<T: std::fmt::Debug, E>(r: Result<T, E>) -> Option<E> {
    match r {
        Err(e) => Some(e),
        Ok(v) => {
            eprintln!("unexpected Err-expected value: {v:?}");
            None
        }
    }
}

fn tokens_only_cap(period: BudgetPeriod, cap: u64) -> BTreeMap<BudgetPeriod, BudgetCap> {
    BTreeMap::from([(
        period,
        BudgetCap {
            money: None,
            tokens: Some(TokenCount(cap)),
        },
    )])
}

#[test]
fn warning_fires_exactly_at_the_80_percent_crossing() {
    let spec = BudgetSpec::from_caps(tokens_only_cap(BudgetPeriod::Task, 100));
    let Some(mut ledger) = ok(SpendLedger::new(spec)) else {
        return;
    };
    let mut sink = Recorder::default();

    for spent in [79_u64, 1, 19, 1] {
        assert!(ledger.record(Spend::unpriced(spent), &mut sink).is_ok());
    }

    let warnings: Vec<_> = sink
        .signals
        .iter()
        .filter(|s| matches!(s, BudgetSignal::Warning(w) if w.dimension == Dimension::Tokens))
        .collect();
    let stops: Vec<_> = sink
        .signals
        .iter()
        .filter(|s| matches!(s, BudgetSignal::Stop(_)))
        .collect();
    // 79 -> nothing; 80 -> Warning; 99 -> nothing; 100 -> Stop.
    assert_eq!(warnings.len(), 1, "warning must fire exactly once");
    assert_eq!(stops.len(), 1, "stop must fire exactly once");
    let BudgetSignal::Warning(w) = warnings[0] else {
        eprintln!("filtered to warnings above: {:?}", warnings[0]);
        return;
    };
    assert_eq!(w.threshold_pct, 80);
    assert_eq!(w.consumed, 80);
    assert_eq!(w.cap, 100);
    assert_eq!(w.period, BudgetPeriod::Task);
    // Ordering: the warning precedes the stop.
    let warn_at = sink
        .signals
        .iter()
        .position(|s| matches!(s, BudgetSignal::Warning(_)));
    let stop_at = sink
        .signals
        .iter()
        .position(|s| matches!(s, BudgetSignal::Stop(_)));
    assert!(warn_at.is_some() && stop_at.is_some() && warn_at < stop_at);
}

#[test]
fn money_dimension_thresholds_fire_exactly() {
    let caps = BTreeMap::from([(
        BudgetPeriod::Task,
        BudgetCap {
            money: Some(MicroUsd(1_000_000)),
            tokens: None,
        },
    )]);
    let Some(mut ledger) = ok(SpendLedger::new(BudgetSpec::from_caps(caps))) else {
        return;
    };
    let mut sink = Recorder::default();

    for micros in [799_999_u128, 1, 199_999, 1] {
        assert!(ledger.record(Spend::priced(1, micros), &mut sink).is_ok());
    }

    assert_eq!(sink.signals.len(), 2, "exactly one warning + one stop");
    let BudgetSignal::Warning(w) = &sink.signals[0] else {
        eprintln!("first signal must be the warning: {:?}", sink.signals[0]);
        return;
    };
    assert_eq!((w.consumed, w.cap), (800_000, 1_000_000));
    let BudgetSignal::Stop(s) = &sink.signals[1] else {
        eprintln!("second signal must be the stop: {:?}", sink.signals[1]);
        return;
    };
    assert_eq!(s.exhaustion().consumed, 1_000_000);
    assert_eq!(s.exhaustion().dimension, Dimension::Money);
}

#[test]
fn multi_period_session_fires_each_period_at_its_own_points() {
    // Task 50 / Session 200 / Day 1000 tokens; 20 spends of 10 tokens each.
    let caps = BTreeMap::from([
        (
            BudgetPeriod::Task,
            BudgetCap {
                money: None,
                tokens: Some(TokenCount(50)),
            },
        ),
        (
            BudgetPeriod::Session,
            BudgetCap {
                money: None,
                tokens: Some(TokenCount(200)),
            },
        ),
        (
            BudgetPeriod::Day,
            BudgetCap {
                money: None,
                tokens: Some(TokenCount(1000)),
            },
        ),
    ]);
    let Some(mut ledger) = ok(SpendLedger::new(BudgetSpec::from_caps(caps))) else {
        return;
    };
    let mut sink = Recorder::default();
    let mut fired_after_spend: Vec<(usize, &'static str, BudgetPeriod)> = Vec::new();
    for i in 1..=20_u64 {
        assert!(ledger.record(Spend::unpriced(10), &mut sink).is_ok());
        for s in &sink.signals[fired_after_spend.len()..] {
            let kind = if matches!(s, BudgetSignal::Warning(_)) {
                "warn"
            } else {
                "stop"
            };
            fired_after_spend.push((i as usize, kind, s.period()));
        }
    }
    // Task warns at 40 (spend #4), stops at 50 (#5); Session warns at 160
    // (#16), stops at 200 (#20); Day never reaches its 800-token warn line.
    assert_eq!(
        fired_after_spend,
        [
            (4, "warn", BudgetPeriod::Task),
            (5, "stop", BudgetPeriod::Task),
            (16, "warn", BudgetPeriod::Session),
            (20, "stop", BudgetPeriod::Session),
        ]
    );
}

#[test]
fn rollup_tree_aggregates_child_spend_to_the_parent() {
    // parent(own 100 tok / 1_000_000 µUSD)
    //   ├─ child-a(own 40 / 400_000) ── grandchild(10 / 100_000)
    //   └─ child-b(own 25 / 250_000)
    let grandchild = RollupNode::leaf("grandchild", Spend::priced(10, 100_000));
    let child_a = RollupNode::branch("child-a", Spend::priced(40, 400_000), vec![grandchild]);
    let child_b = RollupNode::leaf("child-b", Spend::priced(25, 250_000));
    let parent = RollupNode::branch(
        "parent",
        Spend::priced(100, 1_000_000),
        vec![child_a, child_b],
    );

    let Some(total) = ok(parent.total()) else {
        return;
    };
    assert_eq!(total.tokens, TokenCount(175));
    assert_eq!(total.money(), MicroUsd(1_750_000));

    // Per-subagent spend visible at the parent (each child with its subtree).
    let Some(children) = ok(parent.child_spend()) else {
        return;
    };
    let visible: Vec<(&str, u128, u64)> = children
        .iter()
        .map(|(label, spend)| (*label, spend.money().0, spend.tokens.0))
        .collect();
    assert_eq!(
        visible,
        [("child-a", 500_000, 50), ("child-b", 250_000, 25)]
    );
}

#[test]
fn zero_ceiling_refuses_inference_with_a_typed_message() {
    let caps = BTreeMap::from([(
        BudgetPeriod::Task,
        BudgetCap {
            money: Some(MicroUsd(0)),
            tokens: Some(TokenCount(0)),
        },
    )]);
    let Some(ledger) = ok(SpendLedger::new(BudgetSpec::from_caps(caps))) else {
        return;
    };
    let Some(refusal) = err(ledger.preflight()) else {
        return;
    };
    let msg = refusal.to_string();
    assert!(
        msg.contains("money") && msg.contains("task"),
        "typed refusal must name the dimension and period: {msg}"
    );
}

#[test]
fn stop_surfaces_only_after_the_completing_tools_spend_is_recorded() {
    // Graceful-turn semantics: a Stop never aborts a running tool. The
    // signal is delivered by the record() that books the tool's full cost —
    // the consumer loop finishes the tool, records its spend, and only then
    // sees Stop, whose consumed figure already includes that tool.
    let spec = BudgetSpec::from_caps(tokens_only_cap(BudgetPeriod::Task, 100));
    let Some(mut ledger) = ok(SpendLedger::new(spec)) else {
        return;
    };
    let mut sink = Recorder::default();

    for _ in 0..4 {
        assert!(ledger.record(Spend::unpriced(10), &mut sink).is_ok());
    }
    assert!(!sink.signals.iter().any(|s| s.is_stop()));

    // The final tool runs to completion; its 65-token cost is recorded in
    // full, and the Stop arrives inside that same record — never before.
    assert!(ledger.record(Spend::unpriced(65), &mut sink).is_ok());
    let stop_count = sink.signals.iter().filter(|s| s.is_stop()).count();
    assert_eq!(stop_count, 1, "exactly one stop");
    let BudgetSignal::Stop(reason) = &sink.signals[sink.signals.len() - 1] else {
        eprintln!(
            "last signal must be the stop: {:?}",
            sink.signals[sink.signals.len() - 1]
        );
        return;
    };
    assert_eq!(
        reason.exhaustion().consumed,
        105,
        "consumed includes the tool"
    );
}
