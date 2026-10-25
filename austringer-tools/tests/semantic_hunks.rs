//! Stage-E tests: semantic hunk staging — imara-diff Histogram grouped by
//! tree-sitter entities, y/n/e decisions, per-replacement expansion, and
//! the never-spans-entities property over a seeded corpus (incl. CRLF).

use austringer_tools::hunks::{entities_from_defs, stage_hunks};
use austringer_tools::staged_apply::{EntityDecision, ReplacementDecision, apply_staged};
use austringer_tools::{LineTable, ToolsError};
use std::path::Path;

fn fixture() -> String {
    let mut text = String::from("// header comment\n\n");
    for n in 1..=3 {
        text.push_str(&format!("fn part_{n}() {{\n"));
        text.push_str(&format!("    let a{n} = {n};\n"));
        text.push_str(&format!("    let mid{n} = {n} + {n};\n"));
        text.push_str(&format!("    let b{n} = {n} * 2;\n"));
        text.push_str("}\n\n");
    }
    text
}

fn defs_of(table: &LineTable) -> Vec<austringer_ast::Definition> {
    let analyzer = austringer_ast::Analyzer::new();
    let analysis = analyzer
        .analyze_bytes(austringer_ast::LanguageId::Rust, table.as_bytes().to_vec())
        .unwrap_or_else(|e| panic!("{e}"));
    analysis.definitions
}

#[test]
fn three_hunk_edit_stages_per_entity_and_applies_selectively() {
    let old = LineTable::from_string(fixture());
    // Changes: part_1.a1; part_3 gets TWO replacements separated by the
    // unchanged mid3 line (R1: a3, R2: b3 rewrite + c3 insertion). part_2
    // untouched.
    let new_text = fixture()
        .replace("let a1 = 1;", "let a1 = 111;")
        .replace("let a3 = 3;", "let a3 = 333;")
        .replace("let b3 = 3 * 2;", "let b3 = 33 * 2;\n    let c3 = 333;");
    let new = LineTable::from_string(new_text);
    let entities = entities_from_defs(&old, &defs_of(&old));
    let groups = stage_hunks(&old, &new, &entities);

    let labels: Vec<&str> = groups
        .iter()
        .map(|g| g.entity.as_ref().map_or("(top-level)", |e| e.name.as_str()))
        .collect();
    assert_eq!(labels, vec!["part_1", "part_3"], "{labels:?}");
    assert_eq!(groups[1].replacements.len(), 2);

    // y / n / e: part_1 accepted whole; part_3 expanded — R1 accepted,
    // R2 rejected.
    let decisions = vec![
        EntityDecision::Yes,
        EntityDecision::Expand(vec![ReplacementDecision::Yes, ReplacementDecision::No]),
    ];
    let outcome = apply_staged(
        Path::new("fixture.rs"),
        &old,
        &new,
        &groups,
        &decisions,
        None,
    )
    .unwrap_or_else(|e| panic!("{e:?}"));

    assert!(outcome.table.as_str().contains("let a1 = 111;"));
    assert!(outcome.table.as_str().contains("let a3 = 333;"));
    // R2 rejected: original b3 kept, c3 never inserted.
    assert!(outcome.table.as_str().contains("let b3 = 3 * 2;"));
    assert!(!outcome.table.as_str().contains("c3"));
    // Untouched entity survives verbatim.
    assert!(outcome.table.as_str().contains("let mid2 = 2 + 2;"));
    // One bounded fringe per accepted change.
    assert_eq!(outcome.fringes.len(), 2);
    for fringe in &outcome.fringes {
        let rendered = fringe.render();
        assert!(rendered.contains("fixture.rs"), "{rendered}");
    }
}

#[test]
fn decisions_must_match_group_shape_typed() {
    let old = LineTable::from_string(fixture());
    // Two replacements inside part_1 (separated by the unchanged mid1).
    let new_text = fixture()
        .replace("let a1 = 1;", "let a1 = 111;")
        .replace("let b1 = 1 * 2;", "let b1 = 111 * 2;");
    let new = LineTable::from_string(new_text);
    let entities = entities_from_defs(&old, &defs_of(&old));
    let groups = stage_hunks(&old, &new, &entities);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].replacements.len(), 2);
    let err = apply_staged(
        Path::new("f.rs"),
        &old,
        &new,
        &groups,
        &[EntityDecision::Expand(vec![ReplacementDecision::Yes])],
        None,
    )
    .unwrap_err();
    assert!(
        matches!(
            err,
            ToolsError::InvalidDecisions {
                group: 0,
                expected: 2,
                got: 1
            }
        ),
        "got {err:?}"
    );
}

#[test]
fn cross_entity_change_is_split_at_entity_boundaries() {
    // A change that rewrites across the boundary between part_1's last
    // lines and part_2's first lines must stage as pieces inside each
    // entity — never one hunk spanning both.
    let old = LineTable::from_string(fixture());
    let new_text = fixture().replace(
        "    let b1 = 1 * 2;\n}\n\nfn part_2() {",
        "    let b1 = 999;\n}\n\nfn part_2_renamed() {",
    );
    let new = LineTable::from_string(new_text);
    let entities = entities_from_defs(&old, &defs_of(&old));
    let groups = stage_hunks(&old, &new, &entities);
    for group in &groups {
        let Some(entity) = &group.entity else {
            continue;
        };
        for replacement in &group.replacements {
            assert!(
                replacement.old_start >= entity.first_line
                    && (replacement.old_end == 0 || replacement.old_end <= entity.last_line),
                "replacement {replacement:?} spans outside entity {entity:?}"
            );
        }
    }
    let names: Vec<&str> = groups
        .iter()
        .filter_map(|g| g.entity.as_ref().map(|e| e.name.as_str()))
        .collect();
    assert!(
        names.contains(&"part_1") && names.contains(&"part_2"),
        "{names:?}"
    );
}

struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

fn corpus(functions: usize, body: usize) -> String {
    let mut text = String::from("// corpus\n");
    for f in 1..=functions {
        text.push_str(&format!("fn f{f}() {{\n"));
        for b in 0..body {
            text.push_str(&format!("    let v{f}_{b} = {b};\n"));
        }
        text.push_str("}\n");
    }
    text
}

fn mutate(text: &str, seed: u64, edits: usize) -> String {
    let mut rng = XorShift(seed | 1);
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    for _ in 0..edits {
        let target = rng.below(lines.len());
        match rng.below(3) {
            0 => lines[target] = format!("    // rewritten {}", rng.below(1000)),
            1 => lines.insert(
                target,
                format!("    let ins_{} = {};", rng.below(1000), target),
            ),
            _ => {
                let _ = lines.remove(target);
            }
        }
    }
    lines.join("\n") + "\n"
}

#[test]
fn seeded_property_hunk_groups_never_span_entities() {
    for seed in [7u64, 42, 1337, 90210] {
        for crlf in [false, true] {
            let old_text = corpus(12, 6);
            let new_text = mutate(&old_text, seed, 8);
            let old_table = LineTable::from_string(if crlf {
                old_text.replace('\n', "\r\n")
            } else {
                old_text.clone()
            });
            let new_table = LineTable::from_string(if crlf {
                new_text.replace('\n', "\r\n")
            } else {
                new_text.clone()
            });
            let entities = entities_from_defs(&old_table, &defs_of(&old_table));
            let groups = stage_hunks(&old_table, &new_table, &entities);
            assert!(
                !groups.is_empty(),
                "seed {seed} crlf {crlf}: expected changes"
            );
            for group in &groups {
                match &group.entity {
                    Some(entity) => {
                        for replacement in &group.replacements {
                            assert!(
                                replacement.old_end == 0
                                    || (replacement.old_start >= entity.first_line
                                        && replacement.old_end <= entity.last_line),
                                "seed {seed} crlf {crlf}: replacement {replacement:?} spans entity {entity:?}"
                            );
                        }
                    }
                    None => {
                        for replacement in &group.replacements {
                            for entity in &entities {
                                let overlaps = replacement.old_end != 0
                                    && replacement.old_start <= entity.last_line
                                    && replacement.old_end >= entity.first_line;
                                assert!(
                                    !overlaps,
                                    "seed {seed} crlf {crlf}: top-level replacement {replacement:?} overlaps entity {entity:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
