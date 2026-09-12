//! Tantivy BM25 index with incremental mtime+size refresh.
//!
//! Concurrency model (documented choice): tantivy's built-in directory write
//! lock (`meta.lock`, OS advisory — `LockFileEx` on Windows, `flock` on Unix)
//! is held only while a refresh is writing; a competing writer gets
//! [`SearchError::IndexLocked`]. Readers are lock-free and see a committed
//! snapshot. The lock is released when the refresh returns, so callers keep
//! indexes open as long as they like.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use tantivy::query::AllQuery;
use tantivy::schema::{Schema, Value};
use tantivy::{DocAddress, Index, IndexReader, ReloadPolicy, TantivyDocument, TantivyError, Term};

use crate::collector::DocAddrs;
use crate::defs::definition_symbols;
use crate::error::SearchError;
use crate::options::IndexOptions;
use crate::schema::{Fields, head_of, schema};
use crate::walk::{stat_map, walk_files};

/// Outcome of one incremental refresh pass.
#[derive(Debug, Default, Clone, Copy)]
pub struct RefreshReport {
    /// Files newly added to the index.
    pub added: u32,
    /// Files whose content changed and were re-indexed.
    pub updated: u32,
    /// Documents dropped (file gone, renamed away, or became unindexable).
    pub removed: u32,
    /// Files present and unchanged since the previous pass.
    pub unchanged: u32,
    /// Files skipped because a NUL byte appears in the sniff window.
    pub skipped_binary: u32,
    /// Files skipped for exceeding [`IndexOptions::max_file_bytes`].
    pub skipped_oversize: u32,
    /// Files skipped because reading them failed (transient lock, perms).
    pub skipped_unreadable: u32,
    /// Live documents in the index after this pass.
    pub docs_indexed: usize,
}

impl RefreshReport {
    /// Documents written or deleted by this pass — 0 means "no rebuild".
    pub fn touched(&self) -> u32 {
        self.added + self.updated + self.removed
    }
}

fn engine(err: impl std::fmt::Display) -> SearchError {
    SearchError::Engine(err.to_string())
}

/// An open lexical code index backed by tantivy BM25. Cheap to keep alive;
/// see the [module docs](self) for the locking model.
pub struct CodeIndex {
    pub(crate) index: Index,
    pub(crate) reader: IndexReader,
    index_dir: PathBuf,
    pub(crate) fields: Fields,
}

impl std::fmt::Debug for CodeIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodeIndex")
            .field("index_dir", &self.index_dir.display().to_string())
            .finish_non_exhaustive()
    }
}

enum FileDoc {
    Indexed(TantivyDocument),
    Binary,
    Oversize,
    Unreadable,
}

impl CodeIndex {
    /// Opens (or creates) the index stored under `index_dir` — the caller
    /// decides where indexes live (e.g. `<session>/.harness/index`).
    pub fn open(index_dir: &Path) -> Result<Self, SearchError> {
        fs::create_dir_all(index_dir)?;
        let expected = schema();
        let index = Self::open_or_create(index_dir, &expected)?;
        if index.schema() != expected {
            return Err(SearchError::IncompatibleIndex(
                index_dir.display().to_string(),
            ));
        }
        let fields = Fields::from_schema(&expected)?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(engine)?;
        Ok(Self {
            index,
            reader,
            index_dir: index_dir
                .canonicalize()
                .unwrap_or_else(|_| index_dir.to_path_buf()),
            fields,
        })
    }

    fn open_or_create(index_dir: &Path, expected: &Schema) -> Result<Index, SearchError> {
        if index_dir.join("meta.json").is_file() {
            return Index::open_in_dir(index_dir).map_err(engine);
        }
        match Index::create_in_dir(index_dir, expected.clone()) {
            Ok(index) => Ok(index),
            // A sibling created it first: open theirs.
            Err(TantivyError::IndexAlreadyExists) => Index::open_in_dir(index_dir).map_err(engine),
            Err(err) => Err(engine(err)),
        }
    }

    /// Incremental refresh with default options.
    pub fn refresh(&self, root: &Path) -> Result<RefreshReport, SearchError> {
        self.refresh_with(root, &IndexOptions::default())
    }

    /// Walks `root` (gitignore rules apply) and applies the minimal set of
    /// index edits: changed files re-added, deleted files removed, renames
    /// handled as remove+add of the path key.
    pub fn refresh_with(
        &self,
        root: &Path,
        options: &IndexOptions,
    ) -> Result<RefreshReport, SearchError> {
        let root = root.canonicalize()?;
        let (files, _) = walk_files(&root, Some(&self.index_dir));
        let disk = stat_map(files);
        let indexed = self.indexed_entries()?;
        let mut report = RefreshReport::default();

        // Classify first, write after: deletions are only queued once the
        // replacement document exists, so a transient read failure keeps the
        // previous document instead of dropping it.
        let mut deletes: Vec<String> = Vec::new();
        let mut adds: Vec<TantivyDocument> = Vec::new();
        for (rel, stat) in &disk {
            let Some((mtime, size)) = indexed.get(rel) else {
                match self.build_document(stat, options) {
                    FileDoc::Indexed(doc) => {
                        report.added += 1;
                        adds.push(doc);
                    }
                    FileDoc::Binary => report.skipped_binary += 1,
                    FileDoc::Oversize => report.skipped_oversize += 1,
                    FileDoc::Unreadable => report.skipped_unreadable += 1,
                }
                continue;
            };
            if *mtime == stat.mtime_nanos && *size == stat.size {
                report.unchanged += 1;
                continue;
            }
            match self.build_document(stat, options) {
                FileDoc::Indexed(doc) => {
                    deletes.push(rel.clone());
                    report.updated += 1;
                    adds.push(doc);
                }
                FileDoc::Binary => {
                    deletes.push(rel.clone());
                    report.skipped_binary += 1;
                    report.removed += 1;
                }
                FileDoc::Oversize => {
                    deletes.push(rel.clone());
                    report.skipped_oversize += 1;
                    report.removed += 1;
                }
                FileDoc::Unreadable => report.skipped_unreadable += 1,
            }
        }
        for rel in indexed.keys() {
            if !disk.contains_key(rel) {
                deletes.push(rel.clone());
                report.removed += 1;
            }
        }

        if deletes.is_empty() && adds.is_empty() {
            report.docs_indexed = self.live_docs()?;
            return Ok(report);
        }

        let mut writer = self.acquire_writer(options)?;
        for rel in &deletes {
            writer.delete_term(Term::from_field_text(self.fields.path, rel));
        }
        for doc in &adds {
            writer.add_document(doc.clone()).map_err(engine)?;
        }
        writer.commit().map_err(engine)?;
        self.reader.reload().map_err(engine)?;
        report.docs_indexed = self.live_docs()?;
        Ok(report)
    }

    fn acquire_writer(&self, options: &IndexOptions) -> Result<tantivy::IndexWriter, SearchError> {
        match self.index.writer_with_num_threads(1, options.writer_heap) {
            Ok(writer) => Ok(writer),
            Err(TantivyError::LockFailure(_, hint)) => Err(SearchError::IndexLocked(
                hint.unwrap_or_else(|| "meta.lock unavailable".into()),
            )),
            Err(err) => Err(engine(err)),
        }
    }

    fn build_document(&self, stat: &crate::walk::FileStat, options: &IndexOptions) -> FileDoc {
        if stat.size > options.max_file_bytes {
            return FileDoc::Oversize;
        }
        let bytes = match fs::read(&stat.abs) {
            Ok(bytes) => bytes,
            Err(_) => return FileDoc::Unreadable,
        };
        let sniff = &bytes[..bytes.len().min(options.sniff_bytes)];
        if sniff.contains(&0) {
            return FileDoc::Binary;
        }
        let content = String::from_utf8_lossy(&bytes);
        let head = head_of(&content, options.head_bytes);
        let mut doc = TantivyDocument::default();
        doc.add_text(self.fields.path, &stat.rel);
        doc.add_i64(self.fields.mtime, stat.mtime_nanos);
        doc.add_u64(self.fields.size, stat.size);
        doc.add_text(self.fields.content, content.as_ref());
        doc.add_text(self.fields.head, head);
        doc.add_text(self.fields.defs, definition_symbols(&content));
        FileDoc::Indexed(doc)
    }

    fn indexed_entries(&self) -> Result<BTreeMap<String, (i64, u64)>, SearchError> {
        let searcher = self.reader.searcher();
        let addresses: Vec<DocAddress> = searcher
            .search(&AllQuery, &DocAddrs::default())
            .map_err(engine)?
            .into_flat();
        let mut entries = BTreeMap::new();
        for address in addresses {
            let doc: TantivyDocument = searcher.doc(address).map_err(engine)?;
            let Some(path) = doc
                .get_first(self.fields.path)
                .and_then(|value| value.as_str())
            else {
                continue;
            };
            let mtime = doc
                .get_first(self.fields.mtime)
                .and_then(|value| value.as_i64())
                .unwrap_or(0);
            let size = doc
                .get_first(self.fields.size)
                .and_then(|value| value.as_u64())
                .unwrap_or(0);
            entries.insert(path.to_string(), (mtime, size));
        }
        Ok(entries)
    }

    fn live_docs(&self) -> Result<usize, SearchError> {
        let searcher = self.reader.searcher();
        Ok(searcher
            .segment_readers()
            .iter()
            .map(|reader| reader.num_docs() as usize)
            .sum())
    }
}
