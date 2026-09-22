//! M5 structural-diff acceptance tests: exact-pointer reporting, scalar and
//! type semantics, null-vs-missing distinction, RFC 6901 escaping, canonical
//! key order, ordered-exact arrays, non-finite pinning, and the
//! human-readable report. Hermetic: no network, no fs writes, no tools.

use std::collections::BTreeSet;

use harness_media::diff::{structural_diff, StructuralDiff, Tolerances};
use serde_json::{json, Value};

/// Diff helper (tests may unwrap; house rule bans it in src only).
fn diff(expected: &Value, actual: &Value, tolerances: &Tolerances) -> StructuralDiff {
    structural_diff(expected, actual, tolerances).unwrap()
}

/// Exact comparison, no reordering escapes.
fn exact() -> Tolerances {
    Tolerances {
        float_epsilon: 0.0,
        unordered_at: BTreeSet::new(),
        ignored_at: BTreeSet::new(),
    }
}

#[test]
fn identical_trees_produce_an_empty_diff() {
    // Given: one canonical tree and a deep clone of itself.
    let tree = json!({
        "asset": {"version": "2.0"},
        "meshes": [{"primitives": 3}],
        "scale": 0.5
    });
    // When: diffed against the clone.
    let out = diff(&tree, &tree.clone(), &exact());
    // Then: the diff is empty - round-trip PASS is SEMANTIC equivalence,
    // never byte identity.
    assert!(out.is_empty());
    assert!(out.added.is_empty() && out.removed.is_empty() && out.changed.is_empty());
}

#[test]
fn single_token_change_reports_one_entry_at_exact_pointer() {
    // Given: one token changed deep inside the tree.
    let expected = json!({"asset": {"version": "2.0"}, "scene": 0, "names": ["a", "b"]});
    let mut actual = expected.clone();
    actual["asset"]["version"] = json!("2.1");
    // When: diffed.
    let out = diff(&expected, &actual, &exact());
    // Then: exactly ONE changed entry total, at the exact RFC 6901 pointer,
    // carrying both values (misleading-success probe: non-empty on a
    // single-token change).
    assert_eq!(out.added.len() + out.removed.len() + out.changed.len(), 1);
    assert_eq!(out.changed[0].path, "/asset/version");
    assert_eq!(out.changed[0].expected, json!("2.0"));
    assert_eq!(out.changed[0].actual, json!("2.1"));
}

#[test]
fn added_and_removed_keys_report_exact_pointers_and_values() {
    // Given: one key dropped, one key inserted.
    let expected = json!({"keep": 1, "drop": {"nested": true}});
    let mut actual = expected.clone();
    actual.as_object_mut().unwrap().remove("drop");
    actual["new"] = json!([1, 2]);
    // When: diffed.
    let out = diff(&expected, &actual, &exact());
    // Then: whole-subtree Removal and Addition entries at exact pointers.
    assert_eq!(out.removed.len(), 1);
    assert_eq!(out.removed[0].path, "/drop");
    assert_eq!(out.removed[0].expected, json!({"nested": true}));
    assert_eq!(out.added.len(), 1);
    assert_eq!(out.added[0].path, "/new");
    assert_eq!(out.added[0].actual, json!([1, 2]));
    assert!(out.changed.is_empty());
}

#[test]
fn null_is_distinguished_from_missing_and_from_other_types() {
    // Given: expected null vs actual missing key.
    let out = diff(&json!({"a": null}), &json!({}), &exact());
    // Then: a Removal of the null node (null is PRESENT, not absent).
    assert_eq!(out.removed.len(), 1);
    assert_eq!(out.removed[0].path, "/a");
    assert_eq!(out.removed[0].expected, Value::Null);
    // And: expected missing vs actual null is the mirror Addition.
    let out = diff(&json!({}), &json!({"a": null}), &exact());
    assert_eq!(out.added.len(), 1);
    assert_eq!(out.added[0].path, "/a");
    assert_eq!(out.added[0].actual, Value::Null);
    // And: both null is equality; null vs 0 is a type change.
    assert!(diff(&json!({"a": null}), &json!({"a": null}), &exact()).is_empty());
    assert_eq!(diff(&json!({"a": null}), &json!({"a": 0}), &exact()).changed[0].path, "/a");
}

#[test]
fn negative_numbers_compare_exactly_and_minus_zero_equals_zero() {
    // Given: matching negatives.
    assert!(diff(&json!({"t": -5}), &json!({"t": -5}), &exact()).is_empty());
    // When: -5 drifts to -4 (epsilon cannot absorb a gross step).
    let out = diff(&json!({"t": -5}), &json!({"t": -4}), &Tolerances {
        float_epsilon: 1e-9,
        ..exact()
    });
    // Then: exact pointer with both signed values.
    assert_eq!(out.changed[0].path, "/t");
    assert_eq!(out.changed[0].expected, json!(-5));
    assert_eq!(out.changed[0].actual, json!(-4));
    // And: IEEE -0.0 == 0.0 (|difference| = 0, exact at epsilon 0).
    assert!(diff(&json!({"z": -0.0}), &json!({"z": 0.0}), &exact()).is_empty());
}

#[test]
fn float_epsilon_absorbs_print_cycle_error_at_1e_9() {
    // Given: the f32->f64 print-cycle pair from the plan.
    let expected = json!({"factor": 0.3});
    let actual = json!({"factor": 0.1 + 0.2});
    // Then: passes at eps 1e-9 (plan acceptance) ...
    assert!(diff(&expected, &actual, &Tolerances { float_epsilon: 1e-9, ..exact() }).is_empty());
    // ... and the plan's literal pair - 0.1+0.2 vs the spelled-out
    // 0.30000000000000004 - is the SAME f64, so it passes at eps 1e-9 AND
    // at exact epsilon.
    let spelled = json!({"factor": 0.30000000000000004});
    assert!(diff(&spelled, &actual, &Tolerances { float_epsilon: 1e-9, ..exact() }).is_empty());
    assert!(diff(&spelled, &actual, &exact()).is_empty());
    // But 0.3 vs 0.1+0.2 at eps 0 is a real changed entry at the pointer.
    let out = diff(&expected, &actual, &exact());
    assert_eq!(out.changed.len(), 1);
    assert_eq!(out.changed[0].path, "/factor");
    // And epsilon never hides a gross mismatch.
    let out = diff(&json!({"f": 0.3}), &json!({"f": 0.9}), &Tolerances {
        float_epsilon: 1e-9,
        ..exact()
    });
    assert_eq!(out.changed.len(), 1);
}

#[test]
fn integer_and_float_representations_compare_semantically() {
    // Given: 1 vs 1.0 - same semantic number in different representations.
    assert!(diff(&json!({"n": 1}), &json!({"n": 1.0}), &exact()).is_empty());
    // When: 2 vs 3.
    let out = diff(&json!({"n": 2}), &json!({"n": 3}), &Tolerances {
        float_epsilon: 1e-9,
        ..exact()
    });
    // Then: changed (u64/i64/f64 all meet in f64 space).
    assert_eq!(out.changed.len(), 1);
}

#[test]
fn type_mismatches_report_changed_with_both_subtrees() {
    // Given: same pointer, different JSON kinds.
    let out = diff(&json!({"v": "text"}), &json!({"v": 5}), &exact());
    assert_eq!(out.changed[0].path, "/v");
    assert_eq!(out.changed[0].expected, json!("text"));
    assert_eq!(out.changed[0].actual, json!(5));
    // And container-vs-scalar, container-vs-container-kind, bool-vs-number.
    assert_eq!(diff(&json!({"v": [1]}), &json!({"v": 1}), &exact()).changed.len(), 1);
    assert_eq!(diff(&json!({"v": [1]}), &json!({"v": {"1": 1}}), &exact()).changed.len(), 1);
    assert_eq!(diff(&json!({"v": true}), &json!({"v": 1}), &exact()).changed.len(), 1);
    // And unequal strings.
    assert_eq!(diff(&json!({"v": "a"}), &json!({"v": "b"}), &exact()).changed.len(), 1);
}

#[test]
fn pointers_escape_special_tokens_per_rfc6901() {
    // Given: keys containing the two RFC 6901 special characters.
    let expected = json!({"a/b": 1, "m~n": 2});
    let mut actual = expected.clone();
    actual["a/b"] = json!(9);
    actual["m~n"] = json!(8);
    // When: diffed.
    let out = diff(&expected, &actual, &exact());
    // Then: '/' escapes as ~1 and '~' escapes as ~0, in canonical key order.
    let paths: Vec<&str> = out.changed.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(paths, ["/a~1b", "/m~0n"]);
}

#[test]
fn canonical_key_order_means_insertion_order_cannot_matter() {
    // Given: the same object built in opposite insertion orders. serde_json's
    // default Map is a BTreeMap (preserve_order is banned workspace-wide), so
    // a non-canonical key order is UNREPRESENTABLE - this pins that fact.
    let mut a = serde_json::Map::new();
    a.insert("zeta".into(), json!(1));
    a.insert("alpha".into(), json!(2));
    let mut b = serde_json::Map::new();
    b.insert("alpha".into(), json!(2));
    b.insert("zeta".into(), json!(1));
    // When: diffed.
    // Then: empty - key order is canonical by construction.
    assert!(diff(&Value::Object(a), &Value::Object(b), &exact()).is_empty());
}

#[test]
fn arrays_are_ordered_exact_by_default() {
    // Given: a reordered array with NO unordered tolerance.
    let out = diff(&json!({"row": [1, 2, 3]}), &json!({"row": [3, 2, 1]}), &exact());
    // Then: positions 0 and 2 changed; position 1 matched.
    let paths: Vec<&str> = out.changed.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(paths, ["/row/0", "/row/2"]);
    // And length growth/shrink report index-exact entries.
    let out = diff(&json!({"row": [1]}), &json!({"row": [1, 7]}), &exact());
    assert_eq!(out.added[0].path, "/row/1");
    let out = diff(&json!({"row": [1, 7]}), &json!({"row": [1]}), &exact());
    assert_eq!(out.removed[0].path, "/row/1");
}

#[test]
fn nonfinite_floats_cannot_enter_trees_and_surface_as_null() {
    // PINNED DECISION: serde_json cannot hold NaN/inf - Number::from_f64
    // returns None and Value::from maps non-finite floats to Null. Canonical
    // trees therefore never carry non-finite numbers; a writer emitting one
    // lands as Null and diffs as a plain type change at that pointer.
    assert_eq!(serde_json::Number::from_f64(f64::NAN), None);
    assert_eq!(Value::from(f64::NAN), Value::Null);
    assert_eq!(json!(f64::INFINITY), Value::Null);
    let out = diff(&json!({"f": 0.3}), &json!({"f": f64::NAN}), &Tolerances {
        float_epsilon: 1e-9,
        ..exact()
    });
    assert_eq!(out.changed.len(), 1);
    assert_eq!(out.changed[0].path, "/f");
    assert_eq!(out.changed[0].actual, Value::Null);
}

#[test]
fn report_renders_human_readable_lines_with_exact_pointers() {
    // Manual-QA fixture: the rendered diff is printed to stdout and captured
    // verbatim in the task evidence (run with output visible).
    let expected = json!({
        "asset": {"version": "2.0"},
        "materials": [{"name": "red"}],
        "scale": 1.0
    });
    let mut actual = expected.clone();
    actual["asset"]["version"] = json!("2.1");
    actual["scene"] = json!(0);
    actual.as_object_mut().unwrap().remove("scale");
    // When: rendered.
    let out = diff(&expected, &actual, &exact());
    let text = out.to_string();
    println!("--- structural diff (example) ---\n{text}--- end ---");
    // Then: one deterministic line per entry, exact pointers, both values.
    assert!(text.contains("changed /asset/version: expected \"2.0\", actual \"2.1\""));
    assert!(text.contains("added   /scene: 0"));
    assert!(text.contains("removed /scale: 1.0"));
    // And: an empty diff renders its own verdict line.
    let empty = diff(&expected, &expected.clone(), &exact()).to_string();
    assert!(empty.contains("no differences"), "got: {empty}");
}
