//! QA scenario printers (plan todo 16 evidence): run with
//! `cargo test -p harness-tools --test qa_scenarios -- --ignored --nocapture`
//! and paste the output into the evidence file. Each printer drives one
//! plan-named QA channel end-to-end through the public API.

use harness_tools::edit::{LineEdit, apply_edits_path};
use harness_tools::hunks::{entities_from_defs, stage_hunks};
use harness_tools::staged_apply::{EntityDecision, ReplacementDecision, apply_staged};
use harness_tools::strict::{MemoryLedger, ReadLedger, enforce_read_scope};
use harness_tools::{LineTable, ToolsError};
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

fn defs_of(table: &LineTable) -> Vec<harness_ast::Definition> {
    let analysis = harness_ast::Analyzer::new()
        .analyze_bytes(harness_ast::LanguageId::Rust, table.as_bytes().to_vec())
        .unwrap_or_else(|e| panic!("{e}"));
    analysis.definitions
}

#[test]
#[ignore = "QA evidence printer"]
fn qa_happy_three_hunk_selective_with_fringe() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    let file = dir.path().join("fixture.rs");
    std::fs::write(&file, fixture()).unwrap_or_else(|e| panic!("{e}"));
    let old = LineTable::from_string(fixture());
    let new_text = fixture()
        .replace("let a1 = 1;", "let a1 = 111;")
        .replace("let a3 = 3;", "let a3 = 333;")
        .replace("let b3 = 3 * 2;", "let b3 = 33 * 2;\n    let c3 = 333;");
    let new = LineTable::from_string(new_text);
    let groups = stage_hunks(&old, &new, &entities_from_defs(&old, &defs_of(&old)));
    println!(
        "groups: {:?}",
        groups
            .iter()
            .map(|g| g
                .entity
                .as_ref()
                .map_or("(top)".to_owned(), |e| e.name.clone()))
            .collect::<Vec<_>>()
    );
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
    std::fs::write(&file, outcome.table.as_bytes()).unwrap_or_else(|e| panic!("{e}"));
    for (index, fringe) in outcome.fringes.iter().enumerate() {
        println!("--- fringe {index} ---\n{}", fringe.render());
    }
    let on_disk = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{e}"));
    println!("--- file contents after ---\n{on_disk}");
    assert!(on_disk.contains("let a1 = 111;") && on_disk.contains("let a3 = 333;"));
    assert!(on_disk.contains("let b3 = 3 * 2;") && !on_disk.contains("c3"));
}

#[test]
#[ignore = "QA evidence printer"]
fn qa_drift_small_shift_reanchors_large_shift_rejects() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    let file = dir.path().join("drift.rs");
    let original = fixture();
    std::fs::write(&file, &original).unwrap_or_else(|e| panic!("{e}"));
    let table = LineTable::from_string(original.clone());
    let anchor =
        harness_tools::anchors::Hashline::compute(&table, 4).unwrap_or_else(|e| panic!("{e}"));

    let small = format!("// shift 1\n// shift 2\n// shift 3\n{original}");
    std::fs::write(&file, &small).unwrap_or_else(|e| panic!("{e}"));
    let edit = LineEdit {
        line: 4,
        old_text: "    let a1 = 1;".to_owned(),
        new_text: "    let a1 = REANCHORED;".to_owned(),
        hash: Some(anchor.clone()),
    };
    let outcome = apply_edits_path(&file, std::slice::from_ref(&edit), None);
    match &outcome {
        Ok(result) => {
            println!(
                "--- small shift: re-anchored, changed_lines {:?} ---",
                result.changed_lines
            );
            println!("{}", result.fringe.render());
        }
        Err(error) => panic!("small shift must re-anchor: {error:?}"),
    }
    let after = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{e}"));
    println!(
        "--- file line 10 after ---\n{}",
        after.lines().nth(9).unwrap_or_default()
    );

    let noise: String = (1..=400).map(|n| format!("// noise {n}\n")).collect();
    std::fs::write(&file, format!("{noise}{original}")).unwrap_or_else(|e| panic!("{e}"));
    let big = apply_edits_path(&file, std::slice::from_ref(&edit), None);
    match &big {
        Err(ToolsError::AnchorDrift {
            anchor_line,
            candidates,
        }) => {
            println!(
                "--- large shift: typed rejection anchor_line={anchor_line} candidates={candidates:?} ---"
            );
        }
        other => panic!("large shift must typed-reject, got {other:?}"),
    }
    let untouched = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        untouched.contains("let a1 = 1;"),
        "rejected edit must not corrupt"
    );
    println!(
        "--- file untouched after rejection: {} ---",
        untouched.contains("let a1 = 1;")
    );
}

#[test]
#[ignore = "QA evidence printer"]
fn qa_strict_mode_rejects_unread_region() {
    let mut ledger = MemoryLedger::new();
    ledger.record_read(Path::new("src/app.rs"), 5, 20);
    let read_range = 22..=26;
    match enforce_read_scope(
        &ledger,
        Path::new("src/app.rs"),
        std::slice::from_ref(&read_range),
    ) {
        Err(ToolsError::UnreadRegion { path, missing }) => {
            println!(
                "--- strict mode rejection ---\npath: {}\nmissing: {missing:?}",
                path.display()
            );
        }
        other => panic!("expected UnreadRegion, got {other:?}"),
    }
    ledger.record_read(Path::new("src/app.rs"), 21, 30);
    assert!(
        enforce_read_scope(
            &ledger,
            Path::new("src/app.rs"),
            std::slice::from_ref(&read_range)
        )
        .is_ok()
    );
    println!("--- after reading 21-30: edit allowed ---");
}
