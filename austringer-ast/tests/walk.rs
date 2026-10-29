//! Walker contract: gitignore-aware, grammar-allowlisted, bounded,
//! deterministic — relative forward-slashed output, typed root errors.

use austringer_ast::walk::{WalkError, WalkLimits, walk_workspace};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn write(dir: &TempDir, relative: &str, bytes: &[u8]) {
    let path = dir.path().join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|e| panic!("{e}"));
    }
    fs::write(path, bytes).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn skips_gitignored_hidden_and_non_grammar_files() {
    let dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    write(&dir, ".gitignore", b"secret/\n");
    write(&dir, "visible.rs", b"fn a() {}\n");
    write(&dir, "notes.txt", b"fn not_source_at_all() {}\n");
    write(&dir, ".hidden.rs", b"fn h() {}\n");
    write(&dir, "secret/hidden.rs", b"fn s() {}\n");
    let files =
        walk_workspace(dir.path(), &WalkLimits::default()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        files,
        vec![PathBuf::from("visible.rs")],
        "gitignored, hidden, and non-grammar files never enter the walk"
    );
}

#[test]
fn file_cap_truncates_after_sorting() {
    let dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    for name in ["c.rs", "a.rs", "b.rs", "e.rs", "d.rs"] {
        write(&dir, name, b"fn x() {}\n");
    }
    let limits = WalkLimits {
        max_files: 3,
        ..WalkLimits::default()
    };
    let files = walk_workspace(dir.path(), &limits).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        files,
        vec![
            PathBuf::from("a.rs"),
            PathBuf::from("b.rs"),
            PathBuf::from("c.rs")
        ]
    );
}

#[test]
fn size_cap_skips_oversized_files() {
    let dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    write(&dir, "small.rs", b"fn a() {}\n");
    write(&dir, "big.rs", &[b'x'; 128]);
    let limits = WalkLimits {
        max_file_bytes: 64,
        ..WalkLimits::default()
    };
    let files = walk_workspace(dir.path(), &limits).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(files, vec![PathBuf::from("small.rs")]);
}

#[test]
fn nested_paths_are_relative_and_forward_slashed() {
    let dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    write(&dir, "nested/deeper/beta.rs", b"fn b() {}\n");
    write(&dir, "alpha.rs", b"fn a() {}\n");
    let files =
        walk_workspace(dir.path(), &WalkLimits::default()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        files,
        vec![
            PathBuf::from("alpha.rs"),
            PathBuf::from("nested/deeper/beta.rs")
        ]
    );
}

#[test]
fn missing_root_is_a_typed_error() {
    let result = walk_workspace(Path::new("no-such-root-zq"), &WalkLimits::default());
    assert!(matches!(result, Err(WalkError::RootMissing(_))));
}

#[test]
fn file_root_is_a_typed_error() {
    let dir = TempDir::new().unwrap_or_else(|e| panic!("{e}"));
    write(&dir, "root.rs", b"fn r() {}\n");
    let result = walk_workspace(&dir.path().join("root.rs"), &WalkLimits::default());
    assert!(matches!(result, Err(WalkError::RootNotDirectory(_))));
}
