//! Repetition inspector tests (todo 7): the detector trips EXACTLY at the
//! Nth identical call inside the window (2 pass, 3rd blocked with
//! model-feedback suggesting variation), window semantics for non-adjacent
//! repeats, eviction, identity excludes call_id, and argument-key order
//! irrelevance.

use austringer_core::agent_loop::PendingCall;
use austringer_core::inspectors::repetition::{RepetitionConfig, RepetitionInspector};
use austringer_core::inspectors::verdict::{CallInspector, Verdict};

fn identical(tool: &str, arguments: serde_json::Value, id: usize) -> PendingCall {
    PendingCall {
        call_id: format!("call-{id}"),
        tool: tool.to_owned(),
        arguments,
    }
}

fn read(path: &str, id: usize) -> PendingCall {
    identical("read_file", serde_json::json!({ "path": path }), id)
}

async fn inspect(inspector: &mut RepetitionInspector, call: &PendingCall) -> Verdict {
    inspector.inspect(call).await.expect("inspect")
}

#[test]
fn default_config_trips_at_three_within_thirty_two() {
    let config = RepetitionConfig::default();
    assert_eq!(config.threshold, 3);
    assert_eq!(config.window, 32);
}

#[tokio::test]
async fn trips_exactly_at_third_identical_call() {
    let mut inspector = RepetitionInspector::new(RepetitionConfig::default());
    // Two identical calls pass...
    assert_eq!(
        inspect(&mut inspector, &read("same.txt", 1)).await,
        Verdict::Allow {
            authoritative: false
        }
    );
    assert_eq!(
        inspect(&mut inspector, &read("same.txt", 2)).await,
        Verdict::Allow {
            authoritative: false
        }
    );
    // ...the third identical call is blocked with intervention feedback.
    match inspect(&mut inspector, &read("same.txt", 3)).await {
        Verdict::Intervene { reason } => {
            assert!(reason.contains("repetition intervention"), "{reason}");
            assert!(reason.contains("read_file"), "names the tool: {reason}");
            assert!(reason.contains("3 times"), "names the count: {reason}");
            assert!(reason.contains("vary"), "suggests variation: {reason}");
        }
        other => panic!("expected Intervene, got {other:?}"),
    }
    // Still tripped on the fourth: the streak stays over threshold.
    assert!(matches!(
        inspect(&mut inspector, &read("same.txt", 4)).await,
        Verdict::Intervene { .. }
    ));
}

#[tokio::test]
async fn window_counts_non_adjacent_repeats() {
    let mut inspector = RepetitionInspector::new(RepetitionConfig::default());
    assert!(matches!(
        inspect(&mut inspector, &read("a", 1)).await,
        Verdict::Allow { .. }
    ));
    assert!(matches!(
        inspect(&mut inspector, &read("a", 2)).await,
        Verdict::Allow { .. }
    ));
    assert!(
        matches!(
            inspect(&mut inspector, &read("b", 3)).await,
            Verdict::Allow { .. }
        ),
        "a different call does not itself trip"
    );
    // The third `a` inside the window trips even though it was not
    // consecutive with the first two.
    assert!(matches!(
        inspect(&mut inspector, &read("a", 4)).await,
        Verdict::Intervene { .. }
    ));
    // Occurrence-based window semantics: even strict alternation trips
    // when the third occurrence of one call lands inside the window
    // (call 5 is the 3rd `x`), while four alternations stay clean.
    let mut alternating = RepetitionInspector::new(RepetitionConfig::default());
    for id in 0..4 {
        let path = if id % 2 == 0 { "x" } else { "y" };
        assert!(
            matches!(
                inspect(&mut alternating, &read(path, id)).await,
                Verdict::Allow { .. }
            ),
            "two occurrences each must pass (id {id})"
        );
    }
    assert!(matches!(
        inspect(&mut alternating, &read("x", 4)).await,
        Verdict::Intervene { .. }
    ));
}

#[tokio::test]
async fn window_eviction_forgets_old_repeats() {
    let mut inspector = RepetitionInspector::new(RepetitionConfig {
        threshold: 3,
        window: 3,
    });
    assert!(matches!(
        inspect(&mut inspector, &read("a", 1)).await,
        Verdict::Allow { .. }
    ));
    assert!(matches!(
        inspect(&mut inspector, &read("a", 2)).await,
        Verdict::Allow { .. }
    ));
    assert!(matches!(
        inspect(&mut inspector, &read("b", 3)).await,
        Verdict::Allow { .. }
    ));
    assert!(matches!(
        inspect(&mut inspector, &read("c", 4)).await,
        Verdict::Allow { .. }
    ));
    // The two old `a`s fell out of the 3-wide window; this `a` counts 1.
    assert!(matches!(
        inspect(&mut inspector, &read("a", 5)).await,
        Verdict::Allow { .. }
    ));
}

#[tokio::test]
async fn call_id_is_not_part_of_identity() {
    let mut inspector = RepetitionInspector::new(RepetitionConfig::default());
    let args = serde_json::json!({ "path": "same.txt" });
    assert!(matches!(
        inspect(&mut inspector, &identical("read_file", args.clone(), 1)).await,
        Verdict::Allow { .. }
    ));
    assert!(matches!(
        inspect(&mut inspector, &identical("read_file", args.clone(), 2)).await,
        Verdict::Allow { .. }
    ));
    assert!(
        matches!(
            inspect(&mut inspector, &identical("read_file", args, 999)).await,
            Verdict::Intervene { .. }
        ),
        "distinct correlation ids do not break identity"
    );
}

#[tokio::test]
async fn argument_key_order_is_irrelevant_to_identity() {
    let mut inspector = RepetitionInspector::new(RepetitionConfig::default());
    let first = serde_json::json!({ "path": "f.txt", "offset": 0 });
    let second = serde_json::json!({ "offset": 0, "path": "f.txt" });
    assert!(matches!(
        inspect(&mut inspector, &identical("read_file", first, 1)).await,
        Verdict::Allow { .. }
    ));
    assert!(matches!(
        inspect(&mut inspector, &identical("read_file", second, 2)).await,
        Verdict::Allow { .. }
    ));
    let third = serde_json::json!({ "path": "f.txt", "offset": 0 });
    assert!(
        matches!(
            inspect(&mut inspector, &identical("read_file", third, 3)).await,
            Verdict::Intervene { .. }
        ),
        "canonically-equal arguments are the same call"
    );
}

#[tokio::test]
async fn different_arguments_do_not_accumulate() {
    let mut inspector = RepetitionInspector::new(RepetitionConfig::default());
    assert!(matches!(
        inspect(&mut inspector, &read("a", 1)).await,
        Verdict::Allow { .. }
    ));
    assert!(matches!(
        inspect(&mut inspector, &read("a", 2)).await,
        Verdict::Allow { .. }
    ));
    assert!(matches!(
        inspect(&mut inspector, &read("other", 3)).await,
        Verdict::Allow { .. }
    ));
    assert!(matches!(
        inspect(&mut inspector, &read("other", 4)).await,
        Verdict::Allow { .. }
    ));
    // Different tool with the same arguments is a different call too.
    assert!(matches!(
        inspect(
            &mut inspector,
            &identical("grep", serde_json::json!({ "path": "a" }), 5)
        )
        .await,
        Verdict::Allow { .. }
    ));
}

#[tokio::test]
async fn custom_thresholds() {
    let mut tight = RepetitionInspector::new(RepetitionConfig {
        threshold: 2,
        window: 32,
    });
    assert!(matches!(
        inspect(&mut tight, &read("t", 1)).await,
        Verdict::Allow { .. }
    ));
    assert!(matches!(
        inspect(&mut tight, &read("t", 2)).await,
        Verdict::Intervene { .. }
    ));

    let mut loose = RepetitionInspector::new(RepetitionConfig {
        threshold: 5,
        window: 32,
    });
    for id in 1..=4 {
        assert!(
            matches!(
                inspect(&mut loose, &read("t", id)).await,
                Verdict::Allow { .. }
            ),
            "below threshold, call {id} passes"
        );
    }
    assert!(matches!(
        inspect(&mut loose, &read("t", 5)).await,
        Verdict::Intervene { .. }
    ));
}
