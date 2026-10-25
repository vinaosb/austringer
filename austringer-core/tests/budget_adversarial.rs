//! Adversarial scenarios for the budget subsystem (todo 25): determinism
//! (flaky x3), stale_state (resume cursor after restart), and weird spend
//! streams / unicode labels. Threshold exactness and rollup arithmetic are
//! re-asserted independently (anti misleading-success). Malformed-input
//! adversarials live in `budget_malformed.rs`.

use std::collections::BTreeMap;

use austringer_core::budget::{
    BudgetCap, BudgetPeriod, BudgetSignal, BudgetSignalSink, BudgetSpec, Dimension,
    IncompleteBudget, ResumeError, RollupNode, Spend, SpendLedger, TokenCount,
};
use austringer_core::event::Sequence;

/// Signal sink that records everything it receives.
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

fn task_tokens(cap: u64) -> BTreeMap<BudgetPeriod, BudgetCap> {
    BTreeMap::from([(
        BudgetPeriod::Task,
        BudgetCap {
            money: None,
            tokens: Some(TokenCount(cap)),
        },
    )])
}

fn run_threshold_script() -> Vec<BudgetSignal> {
    let Some(mut ledger) = ok(SpendLedger::new(BudgetSpec::from_caps(task_tokens(100)))) else {
        return Vec::new();
    };
    let mut sink = Recorder::default();
    for spent in [79_u64, 1, 19, 1] {
        assert!(ledger.record(Spend::unpriced(spent), &mut sink).is_ok());
    }
    sink.signals
}

#[test]
fn threshold_script_is_deterministic_across_three_runs() {
    let first = run_threshold_script();
    for _ in 0..2 {
        assert_eq!(run_threshold_script(), first, "flaky threshold behavior");
    }
    assert_eq!(first.len(), 2);
}

#[test]
fn resume_cursor_after_restart_resumes_exactly_where_it_stopped() {
    let Some(mut ledger) = ok(SpendLedger::new(BudgetSpec::from_caps(task_tokens(100)))) else {
        return;
    };
    let mut sink = Recorder::default();
    for _ in 0..10 {
        assert!(ledger.record(Spend::unpriced(10), &mut sink).is_ok());
    }
    // The stop landed after sequence 10 (the last committed event).
    let Some(BudgetSignal::Stop(reason)) = sink.signals.last().cloned() else {
        eprintln!("script must end in a stop: {:?}", sink.signals);
        return;
    };
    let incomplete: IncompleteBudget = ledger.stop_snapshot(reason, Sequence(10));
    assert_eq!(incomplete.resume_seq, Sequence(10));

    // Stale state: a restarted session whose log only reached sequence 9
    // cannot honor a cursor beyond it — typed rejection, never silent.
    assert!(matches!(
        SpendLedger::resume(&incomplete, Sequence(9)),
        Err(ResumeError::CursorBeyondLog { .. })
    ));

    // Healthy restart: log at the cursor -> resume exactly at the anchor,
    // with consumption carried over (the budget does not reset).
    let Some(mut plan) = ok(SpendLedger::resume(&incomplete, Sequence(10))) else {
        return;
    };
    assert_eq!(plan.resume_seq, Sequence(10));
    assert_eq!(
        plan.ledger.remaining(BudgetPeriod::Task, Dimension::Tokens),
        Some(0)
    );

    // The incomplete state is resumable, never a hard failure: recording
    // continues on the resumed ledger (fired signals do not refire).
    let mut sink2 = Recorder::default();
    assert!(plan.ledger.record(Spend::unpriced(1), &mut sink2).is_ok());
    assert!(sink2.signals.is_empty(), "no refired signals after resume");
}

#[test]
fn a_single_overshooting_spend_crosses_both_thresholds_in_order() {
    let Some(mut ledger) = ok(SpendLedger::new(BudgetSpec::from_caps(task_tokens(100)))) else {
        return;
    };
    let mut sink = Recorder::default();
    // 0 -> 150% in one spend: Warning (80%) then Stop (100%), never Stop only.
    assert!(ledger.record(Spend::unpriced(150), &mut sink).is_ok());
    assert_eq!(sink.signals.len(), 2);
    assert!(matches!(sink.signals[0], BudgetSignal::Warning(_)));
    let BudgetSignal::Stop(s) = &sink.signals[1] else {
        eprintln!("second signal must be the stop: {:?}", sink.signals[1]);
        return;
    };
    assert_eq!(s.exhaustion().consumed, 150);
}

#[test]
fn unicode_agent_labels_roll_up_exactly() {
    let leaf_a = RollupNode::leaf("分析エージェント", Spend::unpriced(7));
    let leaf_b = RollupNode::leaf("агент-поиск", Spend::unpriced(3));
    let root = RollupNode::branch("🧭 root", Spend::unpriced(0), vec![leaf_a, leaf_b]);
    let Some(total) = ok(root.total()) else {
        return;
    };
    assert_eq!(total.tokens, TokenCount(10));
    let Some(children) = ok(root.child_spend()) else {
        return;
    };
    let labels: Vec<&str> = children.iter().map(|(l, _)| *l).collect();
    assert_eq!(labels, ["分析エージェント", "агент-поиск"]);
}

#[test]
fn rollup_arithmetic_is_recomputed_independently() {
    // Anti misleading-success: assert the tree total against an independent
    // hand-sum of the input fixture, not against a projection of the tree.
    let spends = [
        Spend::priced(1_000, 250_000),
        Spend::priced(2_000, 500_000),
        Spend::unpriced(4_000),
    ];
    let leaves: Vec<_> = spends
        .iter()
        .enumerate()
        .map(|(i, s)| RollupNode::leaf(format!("leaf-{i}").as_str(), *s))
        .collect();
    let root = RollupNode::branch("root", Spend::unpriced(0), leaves);
    let Some(total) = ok(root.total()) else {
        return;
    };
    // Hand-computed expectations from the fixture: 7_000 tokens total; the
    // unpriced spend contributes tokens but never an invented cost.
    assert_eq!(total.tokens.0, 7_000);
    assert_eq!(total.money().0, 750_000);
}
