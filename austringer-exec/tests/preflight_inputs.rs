//! New-input parsing adversarials (plan todo 22): unicode paths and
//! content, CRLF line endings, and mixed error detection under both.

mod common;

use austringer_exec::preflight::{Outcome, PreFlight, SurfaceAfterTwo};
use common::preflightkit as kit;
use std::path::PathBuf;
use tempfile::TempDir;

fn setup() -> (TempDir, TempDir, austringer_vfs::ShadowStore) {
    let target = TempDir::new().expect("target");
    let store_root = TempDir::new().expect("store");
    let store = kit::store(store_root.path());
    kit::cargo_tree(target.path(), b"fn main() {}\n");
    (target, store_root, store)
}

#[test]
fn unicode_path_and_content_parse_cleanly() {
    let rel = "src/ünïcode/日本語.rs";
    let (target, store_root, store) = setup();
    let good: &[u8] = "pub fn 関数() -> &str {\n    \"🎉 unicode\"\n}\n".as_bytes();
    let edit = austringer_vfs::ShadowEdit::create(PathBuf::from(rel), good.to_vec());

    let mut shadow = store.create_shadow(target.path()).expect("shadow");
    shadow.apply(&[edit]).expect("apply");
    let runner = kit::ScriptedRunner::new(vec![]);
    let rig = kit::Rig::new();

    let report = PreFlight::new(&rig.deps(&runner), &mut SurfaceAfterTwo)
        .run(&mut shadow, &[PathBuf::from(rel)])
        .expect("preflight runs");
    assert!(
        matches!(report.outcome, Outcome::Passed),
        "{:?}",
        report.outcome
    );
    drop(shadow);
    drop(store);
    drop(store_root);
}

#[test]
fn crlf_content_is_parsed_and_crlf_errors_are_caught() {
    let (target, store_root, store) = setup();
    let good_crlf: &[u8] = b"fn main() {\r\n    let x = 1;\r\n}\r\n";

    let mut shadow = store.create_shadow(target.path()).expect("shadow");
    let edit = kit::replace_edit("src/main.rs", good_crlf, b"fn main() {}\n");
    shadow.apply(&[edit]).expect("apply");
    let runner = kit::ScriptedRunner::new(vec![]);
    let rig = kit::Rig::new();

    let report = PreFlight::new(&rig.deps(&runner), &mut SurfaceAfterTwo)
        .run(&mut shadow, &[PathBuf::from("src/main.rs")])
        .expect("preflight runs");
    assert!(
        matches!(report.outcome, Outcome::Passed),
        "valid CRLF rust must pass: {:?}",
        report.outcome
    );
    drop(shadow);

    // And a syntax error inside CRLF content is still caught. Shadow 1
    // was discarded (never committed), so this anchors on the ORIGINAL
    // user-tree bytes.
    let mut shadow2 = store.create_shadow(target.path()).expect("shadow 2");
    let broken_crlf: &[u8] = b"fn main() {\r\n    let x = ;\r\n";
    let edit2 = kit::replace_edit("src/main.rs", broken_crlf, b"fn main() {}\n");
    shadow2.apply(&[edit2]).expect("apply 2");
    let runner2 = kit::ScriptedRunner::new(vec![]);
    let rig2 = kit::Rig::new();

    let report2 = PreFlight::new(&rig2.deps(&runner2), &mut SurfaceAfterTwo)
        .run(&mut shadow2, &[PathBuf::from("src/main.rs")])
        .expect("run 2");
    assert!(
        matches!(report2.outcome, Outcome::Failed(_)),
        "CRLF must not hide syntax errors"
    );
    drop(shadow2);
    drop(store);
    drop(store_root);
}
