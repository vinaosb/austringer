//! Incremental staleness handling (plan todo 17): mtime+size-driven refresh
//! must touch only changed docs across modify/delete/rename, must surface a
//! typed error when another writer holds the index lock, and must reject a
//! directory holding a foreign tantivy schema.

mod common;

use std::fs;

use common::{XorShift64, write_file, write_synth_corpus};
use harness_search::{CodeIndex, SearchError};

fn temp_pair(label: &str) -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let outer = tempfile::tempdir().expect("tempdir");
    let corpus = outer.path().join("corpus");
    let index_dir = outer.path().join(label);
    fs::create_dir_all(&corpus).expect("mkdir corpus");
    fs::create_dir_all(&index_dir).expect("mkdir index");
    (outer, corpus, index_dir)
}

const COUNT: u32 = 30;

#[test]
fn incremental_refresh_touches_only_changed_files() {
    let (_keep, corpus, index_dir) = temp_pair("stale-modify");
    let mut rng = XorShift64::new(0xBADC0DE);
    write_synth_corpus(&corpus, COUNT, &mut rng);
    let index = CodeIndex::open(&index_dir).expect("open");

    let initial = index.refresh(&corpus).expect("initial refresh");
    assert_eq!(initial.added, COUNT);
    assert_eq!(initial.docs_indexed, COUNT as usize);

    let no_op = index.refresh(&corpus).expect("no-op refresh");
    assert_eq!(no_op.touched(), 0, "unchanged corpus touches nothing");
    assert_eq!(no_op.unchanged, COUNT);

    write_file(
        &corpus,
        "mod_007/file_00007.rs",
        "pub fn fresh_symbol_v2() -> u32 { 42 }\n",
    );
    let updated = index.refresh(&corpus).expect("incremental refresh");
    assert_eq!(updated.updated, 1, "exactly one doc re-indexed");
    assert_eq!(updated.touched(), 1);
    assert_eq!(updated.unchanged, COUNT - 1);
    assert_eq!(updated.docs_indexed, COUNT as usize);

    let hits = index.search("fresh_symbol_v2", 5).expect("search");
    assert_eq!(hits.len(), 1, "new content is searchable");
    assert_eq!(hits[0].path, "mod_007/file_00007.rs");
}

#[test]
fn deleted_files_drop_out_of_results() {
    let (_keep, corpus, index_dir) = temp_pair("stale-delete");
    let mut rng = XorShift64::new(0xD00D);
    write_synth_corpus(&corpus, COUNT, &mut rng);
    let index = CodeIndex::open(&index_dir).expect("open");
    index.refresh(&corpus).expect("refresh");
    assert_eq!(index.search("widget_8", 5).expect("search").len(), 1);

    fs::remove_file(corpus.join("mod_008/file_00008.rs")).expect("delete file");
    let report = index.refresh(&corpus).expect("refresh after delete");
    assert_eq!(report.removed, 1);
    assert_eq!(report.docs_indexed, COUNT as usize - 1);

    let hits = index.search("widget_8", 5).expect("search");
    assert!(hits.is_empty(), "deleted file must leave the index");
}

#[test]
fn renamed_files_move_without_duplicate_docs() {
    let (_keep, corpus, index_dir) = temp_pair("stale-rename");
    let mut rng = XorShift64::new(0x1234_ABCD);
    write_synth_corpus(&corpus, COUNT, &mut rng);
    let index = CodeIndex::open(&index_dir).expect("open");
    index.refresh(&corpus).expect("refresh");

    fs::rename(
        corpus.join("mod_009/file_00009.rs"),
        corpus.join("mod_009/moved_nine.rs"),
    )
    .expect("rename file");
    let report = index.refresh(&corpus).expect("refresh after rename");
    assert_eq!(report.removed, 1, "old path removed");
    assert_eq!(report.added, 1, "new path added");
    assert_eq!(report.touched(), 2);
    assert_eq!(report.docs_indexed, COUNT as usize, "no duplicate docs");

    let hits = index.search("widget_9", 5).expect("search");
    assert_eq!(hits.len(), 1, "exactly one doc for renamed file");
    assert_eq!(hits[0].path, "mod_009/moved_nine.rs");
}

#[test]
fn concurrent_writer_conflict_returns_typed_lock_error() {
    let (_keep, corpus, index_dir) = temp_pair("stale-lock");
    write_file(&corpus, "a.rs", "pub fn lock_marker_a() {}\n");
    let index = CodeIndex::open(&index_dir).expect("open");
    index.refresh(&corpus).expect("refresh");

    // Hold the index's write lock exactly as a competing refresher would.
    let guard: tantivy::IndexWriter<tantivy::TantivyDocument> =
        tantivy::Index::open_in_dir(&index_dir)
            .expect("open raw")
            .writer_with_num_threads(1, 16_000_000)
            .expect("hold raw writer");

    write_file(&corpus, "b.rs", "pub fn lock_marker_b() {}\n");
    let second = CodeIndex::open(&index_dir).expect("second open");
    let err = second
        .refresh(&corpus)
        .expect_err("locked refresh must fail");
    assert!(
        matches!(err, SearchError::IndexLocked(_)),
        "expected IndexLocked, got {err:?}"
    );

    drop(guard);
    let recovered = second.refresh(&corpus).expect("refresh after lock release");
    assert_eq!(recovered.added, 1);
    assert_eq!(second.search("lock_marker_b", 5).expect("search").len(), 1);
}

#[test]
fn foreign_schema_is_rejected_as_incompatible() {
    let outer = tempfile::tempdir().expect("tempdir");
    let index_dir = outer.path().join("foreign");
    fs::create_dir_all(&index_dir).expect("mkdir");
    let mut builder = tantivy::schema::Schema::builder();
    builder.add_text_field("alien", tantivy::schema::TEXT);
    tantivy::Index::create_in_dir(&index_dir, builder.build()).expect("foreign index");

    let err = CodeIndex::open(&index_dir).expect_err("foreign schema must be rejected");
    assert!(
        matches!(err, SearchError::IncompatibleIndex(_)),
        "expected IncompatibleIndex, got {err:?}"
    );
}
