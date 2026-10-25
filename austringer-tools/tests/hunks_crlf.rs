//! CRLF behavior of semantic hunk staging: identical grouping under
//! CRLF-vs-LF re-encoding, and CRLF-preserving staged application.

use austringer_tools::LineTable;
use austringer_tools::hunks::{entities_from_defs, stage_hunks};
use austringer_tools::staged_apply::{EntityDecision, apply_staged};
use std::path::Path;

fn defs_of(table: &LineTable) -> Vec<austringer_ast::Definition> {
    let analyzer = austringer_ast::Analyzer::new();
    let analysis = analyzer
        .analyze_bytes(austringer_ast::LanguageId::Rust, table.as_bytes().to_vec())
        .unwrap_or_else(|e| panic!("{e}"));
    analysis.definitions
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
    let mut state = seed | 1;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    for _ in 0..edits {
        let target = (next() % lines.len() as u64) as usize;
        match next() % 3 {
            0 => lines[target] = format!("    // rewritten {}", next() % 1000),
            1 => lines.insert(
                target,
                format!("    let ins_{} = {};", next() % 1000, target),
            ),
            _ => {
                let _ = lines.remove(target);
            }
        }
    }
    lines.join("\n") + "\n"
}
#[test]
fn crlf_and_lf_files_stage_identically() {
    let old_text = corpus(6, 5);
    let new_text = mutate(&old_text, 99, 5);
    let entities_of = |t: &LineTable| entities_from_defs(t, &defs_of(t));
    let old_lf = LineTable::from_string(old_text.clone());
    let new_lf = LineTable::from_string(new_text.clone());
    let old_crlf = LineTable::from_string(old_text.replace('\n', "\r\n"));
    let new_crlf = LineTable::from_string(new_text.replace('\n', "\r\n"));
    let groups_lf = stage_hunks(&old_lf, &new_lf, &entities_of(&old_lf));
    let groups_crlf = stage_hunks(&old_crlf, &new_crlf, &entities_of(&old_crlf));
    assert_eq!(groups_lf.len(), groups_crlf.len());
    for (lf, crlf_group) in groups_lf.iter().zip(&groups_crlf) {
        assert_eq!(
            lf.entity.as_ref().map(|e| e.name.clone()),
            crlf_group.entity.as_ref().map(|e| e.name.clone())
        );
        assert_eq!(lf.replacements.len(), crlf_group.replacements.len());
    }
}

#[test]
fn staged_application_preserves_crlf_endings() {
    let old_text = corpus(4, 4);
    let new_text = old_text.replace("let v1_0 = 0;", "let v1_0 = 100;");
    let old = LineTable::from_string(old_text.replace('\n', "\r\n"));
    let new = LineTable::from_string(new_text.replace('\n', "\r\n"));
    let entities = entities_from_defs(&old, &defs_of(&old));
    let groups = stage_hunks(&old, &new, &entities);
    let outcome = apply_staged(
        Path::new("crlf.rs"),
        &old,
        &new,
        &groups,
        &vec![EntityDecision::Yes; groups.len()],
        None,
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    let text = outcome.table.as_str();
    assert!(
        text.contains("let v1_0 = 100;\r\n"),
        "new line must be CRLF: {text:?}"
    );
    assert!(text.starts_with("// corpus\r\n"));
}
