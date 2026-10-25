//! The externalized segment store and the persisted compaction state
//! (plan todo 13, Grok + Cline patterns).
//!
//! Every compaction renders RECOVERABLE segment files plus an index
//! under `{root}/compaction/`: a Summary segment (the doctrine
//! summary), a SealedSpan segment (the folded-away prefix at the
//! configured detail level), and a RetainedSpan segment (the surviving
//! window). [`SegmentStore::load_folded`] reconstructs the folded view
//! from disk — auditable and re-loadable by design. The persisted
//! state carries a blake3 hash of the sealed prefix;
//! [`SegmentStore::validate_prefix`] re-derives it on resume, and a
//! mismatch is a typed [`StaleState`] forcing a re-compact. The store
//! takes a root path parameter only — it never depends on the session
//! crate.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::CompactedSession;
use crate::event::{RecordedEvent, Sequence};
use crate::eviction::soft_trim;

/// How much mass sealed-span segment files carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentDetail {
    /// Summary text only.
    Minimal,
    /// Summary plus head/tail excerpts of the sealed span.
    Standard,
    /// The full sealed events.
    Full,
}

impl SegmentDetail {
    /// The index-file name of the level.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Standard => "standard",
            Self::Full => "full",
        }
    }
}

/// What one segment holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SegmentKind {
    /// The doctrine summary.
    Summary,
    /// The folded-away sealed prefix.
    SealedSpan,
    /// The retained surviving window.
    RetainedSpan,
}

/// Head/tail excerpt of a sealed span (Standard detail).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeadTail {
    /// Leading excerpt.
    pub head: String,
    /// Trailing excerpt.
    pub tail: String,
}

/// One recoverable segment file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SegmentRecord {
    /// The compaction ordinal this segment belongs to.
    pub ordinal: u64,
    /// What the segment holds.
    pub kind: SegmentKind,
    /// Inclusive sequence range of the span covered.
    pub seq_range: (Sequence, Sequence),
    /// The summary text (Summary and SealedSpan segments).
    pub summary: String,
    /// The excerpts (Standard SealedSpan segments).
    pub excerpt: Option<HeadTail>,
    /// The full events (Full detail and RetainedSpan segments).
    pub events: Option<Vec<RecordedEvent>>,
}

/// One index entry pointing at a segment file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexEntry {
    /// The compaction ordinal.
    pub ordinal: u64,
    /// File name relative to the compaction directory.
    pub file: String,
    /// What the file holds.
    pub kind: SegmentKind,
    /// The detail level it was written at.
    pub detail: String,
}

/// The per-session compaction index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SegmentIndex {
    /// Completed compactions.
    pub compactions: u64,
    /// The entries in write order.
    pub entries: Vec<IndexEntry>,
}

pub use super::state::{PersistedCompaction, StaleState, sealed_prefix_hash};

/// Why a segment-store operation failed, typed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SegmentError {
    /// Filesystem failure.
    #[error("segment io failure at {path}: {detail}")]
    Io {
        /// The path involved.
        path: String,
        /// What failed.
        detail: String,
    },
    /// A file exists but its content is incomplete (torn write).
    #[error("torn segment file {file}: {detail}")]
    Torn {
        /// The file.
        file: String,
        /// What is missing.
        detail: String,
    },
    /// A file parsed but its structure is wrong.
    #[error("corrupt segment store at {file}: {detail}")]
    Corrupt {
        /// The file.
        file: String,
        /// What is wrong.
        detail: String,
    },
}

/// Segment files written by one compaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenSegments {
    /// The segment files.
    pub files: Vec<PathBuf>,
    /// The rewritten index.
    pub index: PathBuf,
    /// The persisted state.
    pub state: PathBuf,
}

pub use super::folded::FoldedView;

/// The externalized segment store rooted at a session directory.
#[derive(Debug, Clone)]
pub struct SegmentStore {
    root: PathBuf,
}

impl SegmentStore {
    /// A store rooted at `root` (the session directory; a path
    /// parameter only — no session-crate dependency).
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `{root}/compaction/` (created on write).
    #[must_use]
    pub fn compaction_dir(&self) -> PathBuf {
        self.root.join("compaction")
    }

    fn read_index(&self) -> Result<SegmentIndex, SegmentError> {
        let path = self.compaction_dir().join("index.json");
        if !path.exists() {
            return Ok(SegmentIndex::default());
        }
        let body = fs::read_to_string(&path).map_err(|error| Self::io(&path, error))?;
        serde_json::from_str(&body).map_err(|error| SegmentError::Corrupt {
            file: file_name(&path),
            detail: error.to_string(),
        })
    }

    /// The next compaction ordinal.
    pub fn next_ordinal(&self) -> Result<u64, SegmentError> {
        Ok(self.read_index()?.compactions + 1)
    }

    /// Writes the segment files, the index, and the persisted state
    /// for one completed compaction. The sealed span and its hash are
    /// derived from the FULL stream (`events`) minus the compacted
    /// session's retained window, so validation later re-derives
    /// exactly what was pinned. All writes happen at the end and the
    /// index is rewritten last, so a crash mid-write leaves at most
    /// unreferenced files — never a lying index.
    pub fn write_compaction(
        &self,
        compacted: &CompactedSession,
        events: &[RecordedEvent],
        detail: SegmentDetail,
    ) -> Result<WrittenSegments, SegmentError> {
        let ordinal = self.next_ordinal()?;
        let dir = self.compaction_dir();
        fs::create_dir_all(&dir).map_err(|error| Self::io(&dir, error))?;
        let retained_seqs: std::collections::BTreeSet<u64> =
            compacted.retained.iter().map(|event| event.seq.0).collect();
        let sealed: Vec<RecordedEvent> = events
            .iter()
            .filter(|recorded| !retained_seqs.contains(&recorded.seq.0))
            .cloned()
            .collect();
        let sealed_end = sealed.last().map_or(Sequence(0), |event| event.seq);
        let seq_range = |list: &[RecordedEvent]| {
            (
                list.first().map_or(Sequence(0), |event| event.seq),
                list.last().map_or(Sequence(0), |event| event.seq),
            )
        };
        let sealed_range = seq_range(&sealed);
        let records = [
            SegmentRecord {
                ordinal,
                kind: SegmentKind::Summary,
                seq_range: (Sequence(1), compacted.through_seq),
                summary: compacted.summary.clone(),
                excerpt: None,
                events: None,
            },
            SegmentRecord {
                ordinal,
                kind: SegmentKind::SealedSpan,
                seq_range: sealed_range,
                summary: compacted.summary.clone(),
                excerpt: (detail == SegmentDetail::Standard).then(|| {
                    let rendered = crate::context::render_history(&sealed);
                    HeadTail {
                        head: soft_trim(&rendered, 512, 0),
                        tail: soft_trim(&rendered, 0, 512),
                    }
                }),
                events: (detail == SegmentDetail::Full).then(|| sealed.clone()),
            },
            SegmentRecord {
                ordinal,
                kind: SegmentKind::RetainedSpan,
                seq_range: seq_range(&compacted.retained),
                summary: String::new(),
                excerpt: None,
                events: Some(compacted.retained.clone()),
            },
        ];
        let mut files = Vec::new();
        let mut entries = Vec::new();
        for (position, record) in records.iter().enumerate() {
            let name = format!("segment-{ordinal}-{}.json", position + 1);
            let path = dir.join(&name);
            let body = serde_json::to_string(record).map_err(|error| SegmentError::Io {
                path: name.clone(),
                detail: error.to_string(),
            })?;
            fs::write(&path, body).map_err(|error| Self::io(&path, error))?;
            files.push(path);
            entries.push(IndexEntry {
                ordinal,
                file: name,
                kind: record.kind,
                detail: detail.name().to_owned(),
            });
        }
        let state = PersistedCompaction {
            summary: compacted.summary.clone(),
            through_seq: sealed_end,
            prefix_hash: sealed_prefix_hash(events, sealed_end),
            before_tokens: compacted.before_tokens,
            after_tokens: compacted.after_tokens,
        };
        let state_path = dir.join("state.json");
        let state_body = serde_json::to_string(&state).map_err(|error| SegmentError::Io {
            path: file_name(&state_path),
            detail: error.to_string(),
        })?;
        fs::write(&state_path, state_body).map_err(|error| Self::io(&state_path, error))?;
        let mut index = self.read_index()?;
        index.compactions = ordinal;
        index.entries.extend(entries);
        let index_path = dir.join("index.json");
        let index_body = serde_json::to_string(&index).map_err(|error| SegmentError::Io {
            path: file_name(&index_path),
            detail: error.to_string(),
        })?;
        fs::write(&index_path, index_body).map_err(|error| Self::io(&index_path, error))?;
        Ok(WrittenSegments {
            files,
            index: index_path,
            state: state_path,
        })
    }

    /// Loads and reconstructs the folded view from the index + segment
    /// files. Missing store loads empty; torn or corrupt files are
    /// typed errors.
    pub fn load_folded(&self) -> Result<FoldedView, SegmentError> {
        let index = self.read_index()?;
        let mut segments = Vec::new();
        for entry in &index.entries {
            let path = self.compaction_dir().join(&entry.file);
            let body = fs::read_to_string(&path).map_err(|error| Self::io(&path, error))?;
            let record: SegmentRecord =
                serde_json::from_str(&body).map_err(|error| SegmentError::Torn {
                    file: entry.file.clone(),
                    detail: error.to_string(),
                })?;
            segments.push(record);
        }
        Ok(FoldedView { segments })
    }

    pub(crate) fn io(path: &Path, error: std::io::Error) -> SegmentError {
        SegmentError::Io {
            path: path.display().to_string(),
            detail: error.to_string(),
        }
    }

    pub(crate) fn file_name(path: &Path) -> String {
        file_name(path)
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}
