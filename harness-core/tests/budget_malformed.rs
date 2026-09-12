//! Malformed-input adversarials (todo 25): negative caps, inverted period
//! stacks, and u128 overflow attempts — every one a typed rejection with
//! ledger state preserved.

use std::collections::BTreeMap;

use harness_core::budget::{
    BudgetCap, BudgetPeriod, BudgetSignal, BudgetSignalSink, BudgetSpec, InvalidSpec, LedgerError,
    MicroUsd, RawCap, Spend, SpendLedger, TokenCount,
};

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

#[test]
fn negative_caps_are_rejected_typed_at_the_config_boundary() {
    let raw = BTreeMap::from([(
        BudgetPeriod::Task,
        RawCap {
            money: Some(-1),
            tokens: Some(-5),
        },
    )]);
    let Some(rejection) = err(BudgetSpec::try_from_raw(&raw)) else {
        return;
    };
    assert_eq!(
        rejection,
        InvalidSpec::Negative {
            period: BudgetPeriod::Task,
            field: "money",
        }
    );

    let raw_tokens_only = BTreeMap::from([(
        BudgetPeriod::Task,
        RawCap {
            money: None,
            tokens: Some(-5),
        },
    )]);
    assert!(matches!(
        BudgetSpec::try_from_raw(&raw_tokens_only),
        Err(InvalidSpec::Negative { .. })
    ));
}

#[test]
fn inverted_period_caps_are_rejected_typed() {
    // A task cap larger than its enclosing session cap is inverted.
    let caps = BTreeMap::from([
        (
            BudgetPeriod::Task,
            BudgetCap {
                money: None,
                tokens: Some(TokenCount(100)),
            },
        ),
        (
            BudgetPeriod::Session,
            BudgetCap {
                money: None,
                tokens: Some(TokenCount(50)),
            },
        ),
    ]);
    assert_eq!(
        BudgetSpec::from_caps(caps).validate(),
        Err(InvalidSpec::InvertedPeriods {
            inner: BudgetPeriod::Task,
            outer: BudgetPeriod::Session,
            field: "tokens",
        })
    );
    // Same rule for the money dimension, session vs day.
    let caps = BTreeMap::from([
        (
            BudgetPeriod::Session,
            BudgetCap {
                money: Some(MicroUsd(5_000)),
                tokens: None,
            },
        ),
        (
            BudgetPeriod::Day,
            BudgetCap {
                money: Some(MicroUsd(4_999)),
                tokens: None,
            },
        ),
    ]);
    assert!(matches!(
        BudgetSpec::from_caps(caps).validate(),
        Err(InvalidSpec::InvertedPeriods { .. })
    ));
    // Equal caps are fine (non-strict ordering).
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
                tokens: Some(TokenCount(50)),
            },
        ),
    ]);
    assert_eq!(BudgetSpec::from_caps(caps).validate(), Ok(()));
}

#[test]
fn u128_overflow_attempts_are_rejected_typed_and_state_preserved() {
    let caps = BTreeMap::from([(
        BudgetPeriod::Task,
        BudgetCap {
            money: Some(MicroUsd(u128::MAX)),
            tokens: None,
        },
    )]);
    let Some(mut ledger) = ok(SpendLedger::new(BudgetSpec::from_caps(caps))) else {
        return;
    };
    let mut sink = Recorder::default();
    // A single maximal spend: crosses 80% and 100% in one record —
    // Warning then Stop, in order, both delivered.
    assert!(
        ledger
            .record(Spend::priced(1, u128::MAX), &mut sink)
            .is_ok()
    );
    assert!(matches!(
        sink.signals.first(),
        Some(BudgetSignal::Warning(_))
    ));
    assert!(matches!(sink.signals.last(), Some(BudgetSignal::Stop(_))));
    // The overflow attempt: one more micro-USD cannot be booked.
    assert_eq!(
        ledger.record(Spend::priced(1, 1), &mut sink),
        Err(LedgerError::Overflow {
            period: BudgetPeriod::Task,
            field: "money",
        })
    );
    // State preserved: no new signals from the refused record.
    assert_eq!(sink.signals.len(), 2);
}

#[test]
fn token_dimension_overflow_is_rejected_typed() {
    let caps = BTreeMap::from([(
        BudgetPeriod::Task,
        BudgetCap {
            money: None,
            tokens: Some(TokenCount(u64::MAX)),
        },
    )]);
    let Some(mut ledger) = ok(SpendLedger::new(BudgetSpec::from_caps(caps))) else {
        return;
    };
    let mut sink = Recorder::default();
    assert!(
        ledger
            .record(Spend::unpriced(u64::MAX - 1), &mut sink)
            .is_ok()
    );
    assert_eq!(
        ledger.record(Spend::unpriced(5), &mut sink),
        Err(LedgerError::Overflow {
            period: BudgetPeriod::Task,
            field: "tokens",
        })
    );
}
