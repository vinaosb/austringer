//! Persistent symbol-table cache: one JSON entry per analyzed file, keyed
//! by Blake3 of the canonical path, validated by content hash + length
//! (mtime is stored for fast-path freshness and refreshed on hash hits).
//!
//! Corruption contract: any unreadable/unparseable/outdated/torn entry is
//! treated as a miss — the file re-parses and the entry is overwritten.
//! Never an error surfaced to callers, never a panic. Writes are
//! temp-file + rename, so concurrent analyzers cannot interleave a torn
//! entry (last writer wins).

use crate::analyze::Analysis;
use crate::language::LanguageId;
use crate::parse::Diagnostic;
use crate::tags::{Definition, Reference};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const CACHE_VERSION: u32 = 1;

/// File identity used for cache validation.
pub(crate) struct FileStamp {
    /// Last-modified time.
    pub mtime: SystemTime,
    /// Size in bytes.
    pub len: u64,
}

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    v: u32,
    path: String,
    lang: String,
    mtime_secs: u64,
    mtime_nanos: u32,
    len: u64,
    blake3: String,
    has_error: bool,
    has_missing: bool,
    tags_truncated: bool,
    definitions: Vec<Definition>,
    references: Vec<Reference>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Clone)]
pub(crate) struct TableCache {
    dir: PathBuf,
}

impl TableCache {
    pub(crate) fn new(dir: PathBuf) -> Self {
        TableCache { dir }
    }

    fn entry_path(&self, canonical: &Path) -> PathBuf {
        let key = blake3::hash(canonical.as_os_str().as_encoded_bytes());
        self.dir.join(format!("{}.json", key.to_hex()))
    }

    /// Load the entry for `canonical` if it is intact, current-version,
    /// and matches `content_hash` + `stamp.len`. The stored mtime is not
    /// authoritative (a touched-but-unchanged file stays a hit); the
    /// boolean reports whether it is stale so the caller can refresh.
    pub(crate) fn load(
        &self,
        canonical: &Path,
        content_hash: &blake3::Hash,
        stamp: &FileStamp,
    ) -> Option<(Analysis, bool)> {
        let text = std::fs::read_to_string(self.entry_path(canonical)).ok()?;
        let entry: CacheEntry = serde_json::from_str(&text).ok()?;
        if entry.v != CACHE_VERSION
            || entry.blake3 != content_hash.to_hex().to_string()
            || entry.len != stamp.len
            || entry.path != canonical.to_string_lossy()
        {
            return None;
        }
        let language = LanguageId::from_name(&entry.lang)?;
        let analysis = Analysis {
            language,
            definitions: entry.definitions,
            references: entry.references,
            diagnostics: entry.diagnostics,
            has_error: entry.has_error,
            has_missing: entry.has_missing,
            tags_truncated: entry.tags_truncated,
            cache: crate::analyze::CacheStatus::Hit,
        };
        let stale_mtime = secs_nanos(&stamp.mtime) != (entry.mtime_secs, entry.mtime_nanos);
        Some((analysis, stale_mtime))
    }

    /// Persist an entry; failures are reported (caller degrades to
    /// `CacheStatus::StoreFailed`) but never propagate as errors.
    pub(crate) fn store(
        &self,
        canonical: &Path,
        stamp: &FileStamp,
        content_hash: &blake3::Hash,
        language: LanguageId,
        analysis: &Analysis,
    ) -> std::io::Result<()> {
        let (mtime_secs, mtime_nanos) = secs_nanos(&stamp.mtime);
        let entry = CacheEntry {
            v: CACHE_VERSION,
            path: canonical.to_string_lossy().into_owned(),
            lang: language.name().to_owned(),
            mtime_secs,
            mtime_nanos,
            len: stamp.len,
            blake3: content_hash.to_hex().to_string(),
            has_error: analysis.has_error,
            has_missing: analysis.has_missing,
            tags_truncated: analysis.tags_truncated,
            definitions: analysis.definitions.clone(),
            references: analysis.references.clone(),
            diagnostics: analysis.diagnostics.clone(),
        };
        let json = serde_json::to_vec(&entry).map_err(std::io::Error::other)?;
        std::fs::create_dir_all(&self.dir)?;
        let final_path = self.entry_path(canonical);
        let tmp_path = self.dir.join(format!(
            "{}.tmp",
            final_path.file_name().unwrap_or_default().to_string_lossy()
        ));
        std::fs::write(&tmp_path, json)?;
        std::fs::rename(&tmp_path, final_path)
    }
}

fn secs_nanos(t: &SystemTime) -> (u64, u32) {
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => (d.as_secs(), d.subsec_nanos()),
        Err(e) => {
            let d = e.duration();
            (0, d.subsec_nanos())
        }
    }
}
