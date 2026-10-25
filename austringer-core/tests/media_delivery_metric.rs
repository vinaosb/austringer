//! Prefix-identity metric tests (plan M8): the byte-prefix identity
//! definition for consecutive-turn prompts and its window aggregation.

use austringer_core::media_delivery::{prefix_identity, window_prefix_identity};

#[test]
fn extending_prompt_is_full_identity() {
    assert_eq!(prefix_identity(b"abc", b"abcd"), 1.0);
    assert_eq!(
        prefix_identity(b"user: hi\n", b"user: hi\nassistant: ok"),
        1.0
    );
}

#[test]
fn empty_previous_is_full_identity() {
    assert_eq!(prefix_identity(b"", b"anything"), 1.0);
}

#[test]
fn divergence_point_drops_the_tail_ratio() {
    // "abc" vs "abXc": common prefix 2 of 3 previous bytes.
    assert!((prefix_identity(b"abc", b"abXc") - 2.0 / 3.0).abs() < 1e-12);
    assert_eq!(prefix_identity(b"abc", b"xyz"), 0.0);
}

#[test]
fn identity_measures_against_the_previous_prompt() {
    // Shrink: current is a strict prefix of previous -> 3 of 4 shared.
    assert!((prefix_identity(b"abcd", b"abc") - 0.75).abs() < 1e-12);
    assert!((prefix_identity(b"abcd", b"abXd") - 0.5).abs() < 1e-12);
}

#[test]
fn window_mean_over_consecutive_pairs() {
    let perfect: Vec<String> = (1..=5).map(|n| "x".repeat(n)).collect();
    assert_eq!(window_prefix_identity(&perfect), 1.0);
    // One mid-window divergence: prefix "ab" shared, then histories fork.
    let forked = vec!["ab".to_owned(), "abc".to_owned(), "abXt".to_owned()];
    // pair(1,2)=1.0; pair(2,3)= common "ab" (2) / len("abc") (3).
    let expected = (1.0 + 2.0 / 3.0) / 2.0;
    assert!((window_prefix_identity(&forked) - expected).abs() < 1e-12);
}

#[test]
fn degenerate_windows_are_full_identity() {
    assert_eq!(window_prefix_identity(&[]), 1.0);
    assert_eq!(window_prefix_identity(&["only".to_owned()]), 1.0);
}
