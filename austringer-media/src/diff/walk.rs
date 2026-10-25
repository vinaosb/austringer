//! The recursive comparison walker (private). Caps, RFC 6901 pointer
//! building, ordered/unordered array semantics, and epsilon-aware number
//! comparison live here. No textual diff, no unbounded allocation: the
//! walk aborts typed past either cap.

use std::cmp::Ordering;

use serde_json::{Map, Number, Value};

use super::types::{
    Addition, Change, MAX_DIFF_DEPTH, MAX_DIFF_NODES, Removal, StructuralDiff, Tolerances,
};
use crate::error::MediaError;

/// Traversal state: output, cap accounting, and the current pointer
/// (pushed/popped around each child visit; the current depth is implied by
/// the pointer's token count and tracked explicitly for the cap).
struct Walk<'a> {
    tolerances: &'a Tolerances,
    out: StructuralDiff,
    path: String,
    depth: usize,
    nodes: usize,
}

/// Compare two canonical trees; every difference lands in the report with
/// its exact RFC 6901 pointer.
pub(super) fn run(
    expected: &Value,
    actual: &Value,
    tolerances: &Tolerances,
) -> Result<StructuralDiff, MediaError> {
    let mut walk = Walk {
        tolerances,
        out: StructuralDiff::default(),
        path: String::new(),
        depth: 0,
        nodes: 0,
    };
    node(&mut walk, expected, actual)?;
    Ok(walk.out)
}

/// One compared node pair: +2 on the node counter (one per tree).
fn node(walk: &mut Walk<'_>, expected: &Value, actual: &Value) -> Result<(), MediaError> {
    walk.nodes += 2;
    if walk.nodes > MAX_DIFF_NODES || walk.depth > MAX_DIFF_DEPTH {
        return Err(MediaError::DiffTooLarge);
    }
    if walk.tolerances.ignored_at.contains(walk.path.as_str()) {
        return Ok(());
    }
    match (expected, actual) {
        (Value::Object(expected_map), Value::Object(actual_map)) => {
            objects(walk, expected_map, actual_map)
        }
        (Value::Array(expected_items), Value::Array(actual_items)) => {
            arrays(walk, expected_items, actual_items)
        }
        (Value::Number(expected_number), Value::Number(actual_number)) => {
            if !numbers_equal(
                expected_number,
                actual_number,
                walk.tolerances.float_epsilon,
            ) {
                changed(walk, expected, actual);
            }
            Ok(())
        }
        _ => {
            if expected != actual {
                changed(walk, expected, actual);
            }
            Ok(())
        }
    }
}

fn changed(walk: &mut Walk<'_>, expected: &Value, actual: &Value) {
    walk.out.changed.push(Change {
        path: walk.path.clone(),
        expected: expected.clone(),
        actual: actual.clone(),
    });
}

/// RFC 6901 token escaping: `~` becomes `~0`, `/` becomes `~1`.
fn escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// Push one child token onto the pointer; returns the truncate mark.
fn push_child(walk: &mut Walk<'_>, token: &str) -> usize {
    let mark = walk.path.len();
    walk.path.push('/');
    walk.path.push_str(&escape(token));
    mark
}

fn objects(
    walk: &mut Walk<'_>,
    expected_map: &Map<String, Value>,
    actual_map: &Map<String, Value>,
) -> Result<(), MediaError> {
    for (key, expected_value) in expected_map {
        if let Some(actual_value) = actual_map.get(key) {
            let mark = push_child(walk, key);
            walk.depth += 1;
            node(walk, expected_value, actual_value)?;
            walk.depth -= 1;
            walk.path.truncate(mark);
        } else {
            let mark = push_child(walk, key);
            // A key present only on the expected side still honors
            // ignored_at: an ignored pointer's subtree is skipped
            // entirely, whichever side carries it.
            if !walk.tolerances.ignored_at.contains(walk.path.as_str()) {
                walk.out.removed.push(Removal {
                    path: walk.path.clone(),
                    expected: expected_value.clone(),
                });
            }
            walk.path.truncate(mark);
        }
    }
    for (key, actual_value) in actual_map {
        if !expected_map.contains_key(key) {
            let mark = push_child(walk, key);
            if !walk.tolerances.ignored_at.contains(walk.path.as_str()) {
                walk.out.added.push(Addition {
                    path: walk.path.clone(),
                    actual: actual_value.clone(),
                });
            }
            walk.path.truncate(mark);
        }
    }
    Ok(())
}

fn arrays(
    walk: &mut Walk<'_>,
    expected_items: &[Value],
    actual_items: &[Value],
) -> Result<(), MediaError> {
    if walk.tolerances.unordered_at.contains(walk.path.as_str()) {
        return unordered_array(walk, expected_items, actual_items);
    }
    for (index, (expected_value, actual_value)) in
        expected_items.iter().zip(actual_items.iter()).enumerate()
    {
        descend_into(walk, &index.to_string(), expected_value, actual_value)?;
    }
    let common = expected_items.len().min(actual_items.len());
    for (index, expected_value) in expected_items.iter().enumerate().skip(common) {
        let mark = push_child(walk, &index.to_string());
        if !walk.tolerances.ignored_at.contains(walk.path.as_str()) {
            walk.out.removed.push(Removal {
                path: walk.path.clone(),
                expected: expected_value.clone(),
            });
        }
        walk.path.truncate(mark);
    }
    for (index, actual_value) in actual_items.iter().enumerate().skip(common) {
        let mark = push_child(walk, &index.to_string());
        if !walk.tolerances.ignored_at.contains(walk.path.as_str()) {
            walk.out.added.push(Addition {
                path: walk.path.clone(),
                actual: actual_value.clone(),
            });
        }
        walk.path.truncate(mark);
    }
    Ok(())
}

/// Compare a child pair at `token` (shared by the ordered path).
fn descend_into(
    walk: &mut Walk<'_>,
    token: &str,
    expected: &Value,
    actual: &Value,
) -> Result<(), MediaError> {
    let mark = push_child(walk, token);
    walk.depth += 1;
    node(walk, expected, actual)?;
    walk.depth -= 1;
    walk.path.truncate(mark);
    Ok(())
}

/// Order-insensitive array comparison: canonical-serialization multiset
/// matching consumes equal elements; leftovers pair up in sorted canonical
/// order and are compared recursively under the ACTUAL element's index, so
/// a mutated element surfaces at a sub-pointer, not as a whole-array
/// change. Identical-but-reordered elements are consumed without further
/// traversal (counted once per pair).
fn unordered_array(
    walk: &mut Walk<'_>,
    expected_items: &[Value],
    actual_items: &[Value],
) -> Result<(), MediaError> {
    let mut expected_sorted: Vec<(String, usize)> = expected_items
        .iter()
        .enumerate()
        .map(|(index, value)| (value.to_string(), index))
        .collect();
    let mut actual_sorted: Vec<(String, usize)> = actual_items
        .iter()
        .enumerate()
        .map(|(index, value)| (value.to_string(), index))
        .collect();
    expected_sorted.sort();
    actual_sorted.sort();

    let mut expected_leftovers: Vec<usize> = Vec::new();
    let mut actual_leftovers: Vec<usize> = Vec::new();
    let mut expected_cursor = 0;
    let mut actual_cursor = 0;
    while expected_cursor < expected_sorted.len() && actual_cursor < actual_sorted.len() {
        match expected_sorted[expected_cursor]
            .0
            .cmp(&actual_sorted[actual_cursor].0)
        {
            Ordering::Equal => {
                // Consumed as an identical pair: count 2 nodes, no entries.
                walk.nodes += 2;
                if walk.nodes > MAX_DIFF_NODES {
                    return Err(MediaError::DiffTooLarge);
                }
                expected_cursor += 1;
                actual_cursor += 1;
            }
            Ordering::Less => {
                expected_leftovers.push(expected_sorted[expected_cursor].1);
                expected_cursor += 1;
            }
            Ordering::Greater => {
                actual_leftovers.push(actual_sorted[actual_cursor].1);
                actual_cursor += 1;
            }
        }
    }
    while expected_cursor < expected_sorted.len() {
        expected_leftovers.push(expected_sorted[expected_cursor].1);
        expected_cursor += 1;
    }
    while actual_cursor < actual_sorted.len() {
        actual_leftovers.push(actual_sorted[actual_cursor].1);
        actual_cursor += 1;
    }

    for (leftover_index, expected_index) in expected_leftovers.iter().enumerate() {
        if let Some(actual_index) = actual_leftovers.get(leftover_index) {
            descend_into(
                walk,
                &actual_index.to_string(),
                &expected_items[*expected_index],
                &actual_items[*actual_index],
            )?;
        } else {
            let mark = push_child(walk, &expected_index.to_string());
            walk.out.removed.push(Removal {
                path: walk.path.clone(),
                expected: expected_items[*expected_index].clone(),
            });
            walk.path.truncate(mark);
        }
    }
    for actual_index in actual_leftovers.iter().skip(expected_leftovers.len()) {
        let mark = push_child(walk, &actual_index.to_string());
        walk.out.added.push(Addition {
            path: walk.path.clone(),
            actual: actual_items[*actual_index].clone(),
        });
        walk.path.truncate(mark);
    }
    Ok(())
}

/// Epsilon-aware number equality: exact representation match first, then
/// f64 space with the profile's absolute tolerance. serde_json cannot hold
/// non-finite floats (they never enter canonical trees - see module docs).
fn numbers_equal(expected: &Number, actual: &Number, epsilon: f64) -> bool {
    if expected == actual {
        return true;
    }
    match (expected.as_f64(), actual.as_f64()) {
        (Some(expected_float), Some(actual_float)) => {
            (expected_float - actual_float).abs() <= epsilon
        }
        _ => false,
    }
}
