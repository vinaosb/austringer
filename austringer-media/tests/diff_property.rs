//! M5 property tests: a deterministic seeded xorshift64* loop injects
//! add/remove/mutate at RANDOM pointers of RANDOM trees - the engine must
//! report the EXACT injected pointer (and nothing else) every iteration.
//! The seed is pinned; no clock, no fs, no network => zero flake.

use std::collections::BTreeSet;

use austringer_media::diff::{Tolerances, structural_diff};
use serde_json::{Value, json};

/// PINNED SEED - deterministic repro; do not change without re-recording.
const SEED: u64 = 0x00C0_FFE0_0000_0005;
const ITERATIONS: usize = 240;

/// xorshift64* - hand-rolled (no proptest/rand in the workspace).
struct XorShift(u64);

impl XorShift {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n.max(1) as u64) as usize
    }
}

fn exact() -> Tolerances {
    Tolerances {
        float_epsilon: 0.0,
        unordered_at: BTreeSet::new(),
        ignored_at: BTreeSet::new(),
    }
}

fn scalar(rng: &mut XorShift, counter: &mut u64) -> Value {
    *counter += 1;
    match rng.below(5) {
        0 => Value::String(format!("s{}", *counter)),
        1 => Value::from(-(*counter as i64)),
        2 => Value::Bool((*counter).is_multiple_of(2)),
        3 => Value::from(*counter),
        _ => Value::Null,
    }
}

fn build_tree(rng: &mut XorShift, depth: usize, counter: &mut u64) -> Value {
    if depth >= 5 || rng.below(6) == 0 {
        return scalar(rng, counter);
    }
    if rng.below(3) == 0 {
        let n = rng.below(3) + 1;
        return Value::Array(
            (0..n)
                .map(|_| build_tree(rng, depth + 1, counter))
                .collect(),
        );
    }
    let mut map = serde_json::Map::new();
    for j in 0..rng.below(3) + 1 {
        // RFC 6901 special characters appear in ~half the keys, so the
        // property loop exercises pointer escaping too.
        let key = match j % 4 {
            0 => format!("a/b{j}"),
            1 => format!("m~n{j}"),
            _ => format!("k{j}"),
        };
        map.insert(key, build_tree(rng, depth + 1, counter));
    }
    Value::Object(map)
}

/// Root wrapper: add/remove need an object at the root regardless of what
/// the random builder produced.
fn random_document(rng: &mut XorShift, counter: &mut u64) -> Value {
    json!({"root": build_tree(rng, 0, counter)})
}

fn escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn pointer_string(tokens: &[String]) -> String {
    let mut path = String::new();
    for token in tokens {
        path.push('/');
        path.push_str(&escape(token));
    }
    path
}

/// Pointer to a random OBJECT node, descending only through object keys:
/// array-index removal shifts siblings (multiple diffs by design), so the
/// add/remove injections land on object keys only.
fn random_object_pointer(rng: &mut XorShift, tree: &mut Value) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cursor = tree;
    loop {
        let Value::Object(map) = cursor else {
            return tokens;
        };
        if rng.below(2) == 0 {
            return tokens;
        }
        let child_objects: Vec<String> = map
            .iter()
            .filter(|(_, value)| matches!(value, Value::Object(_)))
            .map(|(key, _)| key.clone())
            .collect();
        if child_objects.is_empty() {
            return tokens;
        }
        let key = child_objects[rng.below(child_objects.len())].clone();
        tokens.push(key.clone());
        cursor = map.get_mut(&key).unwrap();
    }
}

/// Pointer to a random LEAF node (scalars and empty containers alike).
fn random_leaf_pointer(rng: &mut XorShift, tree: &mut Value) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cursor = tree;
    loop {
        match cursor {
            Value::Object(map) if !map.is_empty() => {
                let keys: Vec<String> = map.keys().cloned().collect();
                let key = keys[rng.below(keys.len())].clone();
                tokens.push(key.clone());
                cursor = map.get_mut(&key).unwrap();
            }
            Value::Array(items) if !items.is_empty() => {
                let idx = rng.below(items.len());
                tokens.push(idx.to_string());
                cursor = &mut items[idx];
            }
            _ => return tokens,
        }
    }
}

fn node_at<'a>(tree: &'a mut Value, tokens: &[String]) -> &'a mut Value {
    let mut cursor = tree;
    for token in tokens {
        cursor = match cursor {
            Value::Object(map) => map.get_mut(token).unwrap(),
            Value::Array(items) => &mut items[token.parse::<usize>().unwrap()],
            leaf => leaf,
        };
    }
    cursor
}

#[test]
fn injected_differences_surface_at_the_exact_random_pointer() {
    let mut rng = XorShift(SEED);
    let mut counter = 0_u64;
    for i in 0..ITERATIONS {
        let tree = random_document(&mut rng, &mut counter);
        let control = structural_diff(&tree, &tree.clone(), &exact()).unwrap();
        assert!(
            control.is_empty(),
            "iter {i}: identical trees must diff empty"
        );

        match i % 3 {
            0 => {
                // ADD a unique key into a random object.
                let mut actual = tree.clone();
                let mut tokens = random_object_pointer(&mut rng, &mut actual);
                let key = format!("added{i}");
                node_at(&mut actual, &tokens)
                    .as_object_mut()
                    .unwrap()
                    .insert(key.clone(), json!({"fresh": i}));
                tokens.push(key);
                let out = structural_diff(&tree, &actual, &exact()).unwrap();
                assert_eq!(
                    out.added.len() + out.removed.len() + out.changed.len(),
                    1,
                    "iter {i}"
                );
                assert_eq!(out.added[0].path, pointer_string(&tokens), "iter {i}");
                assert_eq!(out.added[0].actual, json!({"fresh": i}), "iter {i}");
            }
            1 => {
                // REMOVE a random existing object key.
                let mut actual = tree.clone();
                let mut tokens = random_object_pointer(&mut rng, &mut actual);
                let node = node_at(&mut actual, &tokens);
                let keys: Vec<String> = node.as_object().unwrap().keys().cloned().collect();
                let key = keys[rng.below(keys.len())].clone();
                let value = node.as_object_mut().unwrap().remove(&key).unwrap();
                tokens.push(key);
                let out = structural_diff(&tree, &actual, &exact()).unwrap();
                assert_eq!(
                    out.added.len() + out.removed.len() + out.changed.len(),
                    1,
                    "iter {i}"
                );
                assert_eq!(out.removed[0].path, pointer_string(&tokens), "iter {i}");
                assert_eq!(out.removed[0].expected, value, "iter {i}");
            }
            _ => {
                // MUTATE a random leaf to a fresh sentinel string.
                let mut actual = tree.clone();
                let tokens = random_leaf_pointer(&mut rng, &mut actual);
                let sentinel = Value::String(format!("mutated-{i}"));
                let old = node_at(&mut actual, &tokens).clone();
                *node_at(&mut actual, &tokens) = sentinel.clone();
                let out = structural_diff(&tree, &actual, &exact()).unwrap();
                assert_eq!(
                    out.added.len() + out.removed.len() + out.changed.len(),
                    1,
                    "iter {i}"
                );
                assert_eq!(out.changed[0].path, pointer_string(&tokens), "iter {i}");
                assert_eq!(out.changed[0].expected, old, "iter {i}");
                assert_eq!(out.changed[0].actual, sentinel, "iter {i}");
            }
        }
    }
}

#[test]
fn shuffled_unordered_arrays_stay_empty_across_seeded_permutations() {
    let mut rng = XorShift(SEED ^ 0x5A17_0000_0000_0001);
    let mut t = Tolerances::default();
    t.unordered_at.insert("/tags".into());
    for i in 0..100_usize {
        let n = 3 + i % 10;
        let original: Vec<Value> = (0..n).map(|k| Value::String(format!("t{k}"))).collect();
        let mut tags = original.clone();
        for j in (1..tags.len()).rev() {
            let k = rng.below(j + 1);
            tags.swap(j, k);
        }
        if tags == original {
            tags.rotate_left(1);
        }
        let expected = json!({"tags": original});
        let actual = json!({"tags": tags});
        assert!(
            structural_diff(&expected, &actual, &t).unwrap().is_empty(),
            "shuffle {i}"
        );
        let strict = structural_diff(&expected, &actual, &exact()).unwrap();
        assert!(
            !strict.is_empty(),
            "shuffle {i}: permutation must differ without the escape"
        );
    }
}
