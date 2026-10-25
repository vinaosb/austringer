//! Incremental checkpoint writes (Cursor) and host-side checkpoint
//! recording (Zed).
//!
//! Every checkpoint persists ONLY the new transcript entries of its turn
//! (`{session}/checkpoints/<turn>.json`, the T24 directory contract) —
//! the conversation is never rewritten. Subagents resume from their last
//! checkpoint via [`CheckpointWriter::load`]. The host records
//! checkpoints in the session thread through [`CheckpointRecorder`] so
//! native and external (ACP) agents get a uniform rewind surface.

use std::path::{Path, PathBuf};

use crate::ids::{SessionId, Turn};
use crate::manifest::{SubmoduleRecord, TaskState};
use crate::provenance::DetachedSignature;
use crate::{SnapshotError, util};
use serde::{Deserialize, Serialize};

pub const CHECKPOINTS_DIR: &str = "checkpoints";
const THREAD_FILE: &str = "checkpoint-thread.jsonl";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranscriptEntry {
    pub seq: u64,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CheckpointMeta {
    pub task: Option<TaskState>,
    /// Snapshot pointer (ref name or CAS manifest path) recorded with the
    /// checkpoint, when one was taken.
    pub snapshot: Option<String>,
    pub submodules: Vec<SubmoduleRecord>,
    pub signature: Option<DetachedSignature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CheckpointFile {
    session: String,
    turn: u32,
    entries: Vec<TranscriptEntry>,
    through_seq: u64,
    meta: CheckpointMeta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumePoint {
    pub turn: Turn,
    pub entries: Vec<TranscriptEntry>,
    pub through_seq: u64,
    pub task: Option<TaskState>,
}

pub struct CheckpointWriter {
    root: PathBuf,
}

impl CheckpointWriter {
    pub fn new(session_root: &Path) -> Result<Self, SnapshotError> {
        util::create_dir_all(&session_root.join(CHECKPOINTS_DIR))?;
        Ok(Self {
            root: session_root.to_path_buf(),
        })
    }

    fn file_of(&self, turn: Turn) -> PathBuf {
        self.root
            .join(CHECKPOINTS_DIR)
            .join(format!("{}.json", turn.slot()))
    }

    /// Persist only the NEW entries of this turn; earlier files are never
    /// reopened or rewritten.
    pub fn write(
        &self,
        session: &SessionId,
        turn: Turn,
        new_entries: &[TranscriptEntry],
        meta: &CheckpointMeta,
    ) -> Result<PathBuf, SnapshotError> {
        let through_seq = new_entries.last().map_or(0, |entry| entry.seq);
        let file = CheckpointFile {
            session: session.as_str().to_owned(),
            turn: turn.get(),
            entries: new_entries.to_vec(),
            through_seq,
            meta: meta.clone(),
        };
        let path = self.file_of(turn);
        util::write_file(&path, util::to_json(&file)?.as_bytes())?;
        Ok(path)
    }

    /// The last checkpoint of this session root, when its session field
    /// matches — the subagent resume point after a failure.
    pub fn load(&self, session: &SessionId) -> Result<Option<ResumePoint>, SnapshotError> {
        let Some((turn, path)) = self.latest_file()? else {
            return Ok(None);
        };
        let bytes = util::read_file(&path)?;
        let file: CheckpointFile = classify(&path, &bytes)?;
        if file.session != session.as_str() {
            return Ok(None);
        }
        if file.turn != turn.get() {
            return Err(SnapshotError::CorruptCheckpoint { path });
        }
        Ok(Some(ResumePoint {
            turn,
            entries: file.entries,
            through_seq: file.through_seq,
            task: file.meta.task,
        }))
    }

    /// through-seq recorded at a specific turn, for the rewind cursor.
    pub(crate) fn through_at(&self, turn: Turn) -> Option<u64> {
        let path = self.file_of(turn);
        let bytes = util::read_file(&path).ok()?;
        let file: CheckpointFile = serde_json::from_slice(&bytes).ok()?;
        (file.through_seq > 0).then_some(file.through_seq)
    }

    fn latest_file(&self) -> Result<Option<(Turn, PathBuf)>, SnapshotError> {
        let dir = self.root.join(CHECKPOINTS_DIR);
        let Ok(children) = util::read_dir_sorted(&dir) else {
            return Ok(None);
        };
        Ok(children
            .iter()
            .filter_map(|path| {
                let stem = path.file_stem()?.to_str()?;
                let n = stem.parse::<u32>().ok()?;
                Some((Turn::new(n).ok()?, path.clone()))
            })
            .max_by_key(|(turn, _)| turn.get()))
    }
}

/// Torn (truncated write) vs corrupt (wrong shape): a serde EOF error
/// means the document ended mid-value — a torn write; any other parse
/// failure is a well-formed document with the wrong shape.
fn classify<T: serde::de::DeserializeOwned>(path: &Path, bytes: &[u8]) -> Result<T, SnapshotError> {
    match serde_json::from_slice(bytes) {
        Ok(value) => Ok(value),
        Err(source) if source.classify() == serde_json::error::Category::Eof => {
            Err(SnapshotError::TornCheckpoint {
                path: path.to_path_buf(),
            })
        }
        Err(_) => Err(SnapshotError::CorruptCheckpoint {
            path: path.to_path_buf(),
        }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "origin", rename_all = "snake_case")]
pub enum CheckpointOrigin {
    Native,
    External { agent: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointRecord {
    pub session: SessionId,
    pub turn: Turn,
    pub origin: CheckpointOrigin,
    /// Ref name or CAS manifest path of the recorded checkpoint.
    pub target: String,
}

/// Host-side recording (Zed): the host, not the agent, appends checkpoint
/// records to the session thread — uniform for native and external
/// (ACP-driven) agents.
pub trait CheckpointRecorder {
    fn record(&mut self, record: &CheckpointRecord) -> Result<(), SnapshotError>;
}

pub struct SessionThreadRecorder {
    thread: PathBuf,
}

impl SessionThreadRecorder {
    pub fn new(session_root: &Path) -> Self {
        Self {
            thread: session_root.join(THREAD_FILE),
        }
    }
}

impl CheckpointRecorder for SessionThreadRecorder {
    fn record(&mut self, record: &CheckpointRecord) -> Result<(), SnapshotError> {
        let line = format!("{}\n", util::to_json(record)?);
        let bytes = line.as_bytes();
        let existing = util::read_file(&self.thread).unwrap_or_default();
        util::create_dir_all(self.thread.parent().unwrap_or(Path::new(".")))?;
        util::write_file(&self.thread, &[existing, bytes.to_vec()].concat())
    }
}
