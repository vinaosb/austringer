//! M5 tolerance and cap tests: unordered_at reordering escape, ignored_at
//! subtree skips, the depth cap (64) and node cap (100k) tripping the typed
//! DiffTooLarge variant. Hermetic: no network, no fs writes, no tools.

use harness_media::diff::{MAX_DIFF_DEPTH, MAX_DIFF_NODES, Tolerances, structural_diff};
use harness_media::error::MediaError;
use serde_json::{Value, json};

fn diff_of(
    expected: &Value,
    actual: &Value,
    tolerances: &Tolerances,
) -> harness_media::diff::StructuralDiff {
    structural_diff(expected, actual, tolerances).unwrap()
}

/// Exact base tolerance plus named unordered array pointers.
fn unordered(pointers: &[&str]) -> Tolerances {
    let mut t = Tolerances::default();
    for p in pointers {
        t.unordered_at.insert((*p).into());
    }
    t
}

/// Exact base tolerance plus named ignored subtree pointers.
fn ignored(pointers: &[&str]) -> Tolerances {
    let mut t = Tolerances::default();
    for p in pointers {
        t.ignored_at.insert((*p).into());
    }
    t
}

#[test]
fn unordered_pointers_are_order_insensitive_under_shuffle() {
    // Given: the plan's glTF-materials use case under /materials.
    let t = unordered(&["/materials"]);
    let expected = json!({"materials": [
        {"name": "red", "baseColorFactor": [1.0, 0.0, 0.0, 1.0]},
        {"name": "green", "baseColorFactor": [0.0, 1.0, 0.0, 1.0]},
        {"name": "blue", "baseColorFactor": [0.0, 0.0, 1.0, 1.0]},
    ]});
    // When: the array arrives reversed (containers legitimately reorder).
    let reversed = json!({"materials": [
        {"name": "blue", "baseColorFactor": [0.0, 0.0, 1.0, 1.0]},
        {"name": "green", "baseColorFactor": [0.0, 1.0, 0.0, 1.0]},
        {"name": "red", "baseColorFactor": [1.0, 0.0, 0.0, 1.0]},
    ]});
    // Then: empty diff - unordered_at is the ONLY reordering escape.
    assert!(diff_of(&expected, &reversed, &t).is_empty());
    // And: duplicates follow MULTISET semantics (not set, not positional).
    let tags = unordered(&["/tags"]);
    assert!(
        diff_of(
            &json!({"tags": [1, 2, 2, 3]}),
            &json!({"tags": [2, 3, 1, 2]}),
            &tags
        )
        .is_empty()
    );
    assert_eq!(
        diff_of(&json!({"tags": [1, 2]}), &json!({"tags": [2, 2]}), &tags)
            .changed
            .len(),
        1
    );
    // And: sibling ordered arrays stay order-sensitive (escape is scoped).
    let out = diff_of(
        &json!({"m": [1, 2], "materials": []}),
        &json!({"m": [2, 1], "materials": []}),
        &t,
    );
    assert_eq!(out.changed.len(), 2);
}

#[test]
fn unordered_element_mutation_reports_a_sub_pointer() {
    // Given: one material's factor mutated AND the array reversed.
    let t = unordered(&["/materials"]);
    let expected = json!({"materials": [
        {"name": "red", "baseColorFactor": [1.0, 0.0]},
        {"name": "blue", "baseColorFactor": [0.0, 1.0]},
    ]});
    let actual = json!({"materials": [
        {"name": "blue", "baseColorFactor": [0.0, 1.0]},
        {"name": "red", "baseColorFactor": [0.5, 0.0]},
    ]});
    // When: diffed.
    let out = diff_of(&expected, &actual, &t);
    // Then: the difference surfaces INSIDE the array at the ACTUAL index of
    // the mutated element - not as a whole-array change.
    assert_eq!(out.changed.len(), 1);
    assert_eq!(out.changed[0].path, "/materials/1/baseColorFactor/0");
    assert_eq!(out.changed[0].expected, json!(1.0));
    assert_eq!(out.changed[0].actual, json!(0.5));
    // And: float epsilon still applies inside unordered pairing. A factor
    // drifted by 1e-12 pairs up under the reversed order and passes at eps
    // 1e-9, but trips at exact epsilon with the sub-pointer.
    let drifted = json!({"materials": [
        {"name": "blue", "baseColorFactor": [0.0, 1.0]},
        {"name": "red", "baseColorFactor": [1.000000000001, 0.0]},
    ]});
    let eps = Tolerances {
        float_epsilon: 1e-9,
        ..t.clone()
    };
    assert!(diff_of(&expected, &drifted, &eps).is_empty());
    let out = diff_of(&expected, &drifted, &t);
    assert_eq!(out.changed.len(), 1);
    assert_eq!(out.changed[0].path, "/materials/1/baseColorFactor/0");
}

#[test]
fn unordered_length_changes_report_index_bearing_entries() {
    // Given: unordered arrays whose sizes differ.
    let t = unordered(&["/tags"]);
    // When: expected has one element more.
    let out = diff_of(&json!({"tags": ["a", "b"]}), &json!({"tags": ["a"]}), &t);
    // Then: exact-index Removal of the unmatched element.
    assert_eq!(out.removed.len(), 1);
    assert_eq!(out.removed[0].path, "/tags/1");
    assert_eq!(out.removed[0].expected, json!("b"));
    // When: actual has one element more.
    let out = diff_of(&json!({"tags": ["a"]}), &json!({"tags": ["a", "c"]}), &t);
    // Then: exact-index Addition.
    assert_eq!(out.added.len(), 1);
    assert_eq!(out.added[0].path, "/tags/1");
    assert_eq!(out.added[0].actual, json!("c"));
}

#[test]
fn ignored_pointers_hide_whole_subtrees_and_nothing_else() {
    // Given: a volatile subtree and a stable sibling.
    let t = ignored(&["/volatile"]);
    let expected = json!({"volatile": {"clock": 111}, "stable": 2});
    let actual = json!({"volatile": {"clock": 999, "extra": true}, "stable": 3});
    // When: diffed.
    let out = diff_of(&expected, &actual, &t);
    // Then: every difference inside /volatile vanishes; the sibling reports.
    assert_eq!(out.changed.len(), 1);
    assert_eq!(out.changed[0].path, "/stable");
    // And: ignoring the ROOT pointer yields a vacuous pass (documented).
    assert!(diff_of(&expected, &actual, &ignored(&[""])).is_empty());
}

#[test]
fn depth_cap_trips_typed_beyond_64_levels() {
    // Given: container chains built iteratively (leaf at depth `n`).
    fn nested(depth: usize) -> Value {
        let mut v = Value::Bool(true);
        for _ in 0..depth {
            v = json!({"d": v});
        }
        v
    }
    assert_eq!(MAX_DIFF_DEPTH, 64, "plan M5 pins the depth cap at 64");
    // When: both trees sit AT the cap.
    // Then: the diff succeeds.
    assert!(structural_diff(&nested(64), &nested(64), &Tolerances::default()).is_ok());
    // When: one level deeper on both sides.
    // Then: the exact typed variant trips - never a panic, never silent.
    assert!(matches!(
        structural_diff(&nested(65), &nested(65), &Tolerances::default()),
        Err(MediaError::DiffTooLarge)
    ));
    // And: one-sided subtrees are reported whole (cloned, input-bounded)
    // WITHOUT traversal - the caps bound comparison work, not tree storage.
    let out = diff_of(&json!({}), &nested(65), &Tolerances::default());
    assert_eq!(out.added.len(), 1);
    assert_eq!(out.added[0].path, "/d");
}

#[test]
fn node_cap_trips_typed_beyond_100k_compared_nodes() {
    // Given: two identical flat arrays; each compared pair counts 2 nodes.
    fn array_of(n: usize) -> Value {
        Value::Array((0..n).map(|i| Value::from(i as u64)).collect())
    }
    assert_eq!(MAX_DIFF_NODES, 100_000, "plan M5 pins the node cap at 100k");
    // When: exactly at the cap (2 x (49_999 elements + 1 root pair)).
    // Then: ok.
    assert!(structural_diff(&array_of(49_999), &array_of(49_999), &Tolerances::default()).is_ok());
    // When: one element more (100_002 compared nodes).
    // Then: the exact typed variant trips, quickly, without unbounded work.
    assert!(matches!(
        structural_diff(&array_of(50_000), &array_of(50_000), &Tolerances::default()),
        Err(MediaError::DiffTooLarge)
    ));
}
