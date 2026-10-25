//! Retry-with-jitter tests: exponential backoff shape, cap, seeded
//! determinism (the flaky-test adversarial probe), and injected-jitter
//! exactness.

use std::time::Duration;

use austringer_providers::retry::{FixedJitter, JitterSource, RetryPolicy, SeededJitter};

fn policy() -> RetryPolicy {
    RetryPolicy {
        max_attempts: 5,
        initial_backoff: Duration::from_millis(100),
        max_backoff: Duration::from_millis(1000),
        multiplier: 4.0,
    }
}

#[test]
fn backoff_is_equal_jittered_exponential_and_capped() {
    let mut top = FixedJitter::new([1.0]);
    let p = policy();
    // Equal jitter: uniform in [base/2, base); at jitter=1.0 the top of the
    // window is just under base, so assert the <= shape against the caps.
    assert!(p.backoff_for(1, &mut top) <= Duration::from_millis(100));
    assert!(p.backoff_for(1, &mut top) > Duration::from_millis(50));
    assert!(p.backoff_for(2, &mut top) <= Duration::from_millis(400));
    assert!(p.backoff_for(3, &mut top) <= Duration::from_millis(1000));
    // 100 * 4^3 = 6400ms capped at 1000ms.
    assert!(p.backoff_for(4, &mut top) <= Duration::from_millis(1000));
    assert_eq!(p.backoff_for(9, &mut top), Duration::from_millis(1000));
}

#[test]
fn zero_jitter_halves_the_backoff() {
    let mut bottom = FixedJitter::new([0.0]);
    let p = policy();
    assert!(p.backoff_for(1, &mut bottom) >= Duration::from_millis(49));
    assert!(p.backoff_for(1, &mut bottom) < Duration::from_millis(51));
}

#[test]
fn seeded_jitter_is_deterministic_across_instances() {
    // Flaky-class probe: three fresh instances off the same seed produce
    // byte-identical sequences.
    for _ in 0..3 {
        let mut a = SeededJitter::new(42);
        let mut b = SeededJitter::new(42);
        for _ in 0..64 {
            assert_eq!(a.next_jitter(), b.next_jitter());
        }
    }
}

#[test]
fn seeded_jitter_sequences_differ_across_seeds() {
    let mut a = SeededJitter::new(1);
    let mut b = SeededJitter::new(2);
    let differs = (0..64).any(|_| a.next_jitter() != b.next_jitter());
    assert!(differs, "different seeds must diverge");
}

#[test]
fn seeded_jitter_stays_in_half_open_unit_window() {
    let mut j = SeededJitter::new(7);
    for _ in 0..1000 {
        let v = j.next_jitter();
        assert!((0.0..1.0).contains(&v), "jitter {v} outside [0,1)");
    }
}

#[test]
fn fixed_jitter_exhausts_to_a_stable_value() {
    let mut j = FixedJitter::new([0.1, 0.9]);
    assert_eq!(j.next_jitter(), 0.1);
    assert_eq!(j.next_jitter(), 0.9);
    // Repeats the final value forever: no hidden state, no RNG fallback.
    assert_eq!(j.next_jitter(), 0.9);
    assert_eq!(j.next_jitter(), 0.9);
}
