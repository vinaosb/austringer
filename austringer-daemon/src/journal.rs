//! Turn journal storage: typed steps, content-addressable activity
//! ids, and the append-only on-disk journal (one JSONL line per step;
//! torn tails are skipped on load, never fatal).

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Content-addressed activity id: blake3 of name + NUL + canonical
/// input bytes. Identical activity inputs collide to one id — that is
/// the retry/replay key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActivityId(pub String);

/// Compute the activity id for `name` + `input`.
#[must_use]
pub fn activity_id(name: &str, input: &serde_json::Value) -> ActivityId {
    let canonical = serde_json::to_vec(input).unwrap_or_default();
    let mut hasher = blake3::Hasher::new();
    hasher.update(name.as_bytes());
    hasher.update(&[0]);
    hasher.update(&canonical);
    ActivityId(hasher.finalize().to_hex().to_string())
}

/// One journaled workflow step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JournalStep {
    /// A turn workflow began.
    TurnStarted {
        /// Turn number.
        turn: u64,
        /// The starting prompt (or signal).
        prompt: String,
    },
    /// An activity (tool call, inference, sub-step) began.
    ActivityStarted {
        /// Content-addressed id.
        id: String,
        /// Activity name.
        name: String,
        /// Canonical input.
        input: serde_json::Value,
    },
    /// An activity produced its output.
    ActivityCompleted {
        /// Matching activity id.
        id: String,
        /// Canonical output.
        output: serde_json::Value,
        /// True when produced by an explicit retry.
        retried: bool,
    },
    /// A signal arrived mid-turn (signal-with-start delivered path).
    Signal {
        /// Signal payload.
        text: String,
    },
    /// A child workflow spawned.
    ChildStarted {
        /// Child journal id.
        child: String,
        /// Child spec (task description).
        spec: String,
    },
    /// A child workflow finished.
    ChildCompleted {
        /// Child journal id.
        child: String,
        /// Child result summary.
        result: String,
    },
    /// The turn workflow finished.
    TurnFinished {
        /// Run outcome summary.
        outcome: String,
    },
}

/// Durable-execution failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DurableError {
    /// Journal IO failure.
    #[error("journal io: {0}")]
    Io(String),
}

/// Default rotation cap for file-backed journals (32 MiB); matches
/// [`crate::config::DaemonConfig::journal_max_bytes`]. 0 would mean
/// unlimited — this constant never is.
pub const DEFAULT_JOURNAL_MAX_BYTES: u64 = 33_554_432;

/// Append-only turn journal on disk.
pub struct Journal {
    path: PathBuf,
    steps: Vec<JournalStep>,
    file: Option<std::fs::File>,
    max_bytes: u64,
}

impl Journal {
    /// Open (creating directories as needed) with the default
    /// rotation cap ([`DEFAULT_JOURNAL_MAX_BYTES`]). Replays prior
    /// steps from the CURRENT file only; torn/malformed lines are
    /// skipped, never abort the load.
    pub fn open(path: &Path) -> Result<Self, DurableError> {
        Self::open_capped(path, DEFAULT_JOURNAL_MAX_BYTES)
    }

    /// Open with an explicit rotation cap; `0` disables rotation
    /// (unlimited growth, the pre-rotation behavior).
    pub fn open_capped(path: &Path, max_bytes: u64) -> Result<Self, DurableError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| DurableError::Io(e.to_string()))?;
        }
        let mut steps = Vec::new();
        if path.exists() {
            let raw = std::fs::read_to_string(path).map_err(|e| DurableError::Io(e.to_string()))?;
            for line in raw.lines() {
                if let Ok(step) = serde_json::from_str::<JournalStep>(line) {
                    steps.push(step);
                }
            }
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| DurableError::Io(e.to_string()))?;
        Ok(Self {
            path: path.to_owned(),
            steps,
            file: Some(file),
            max_bytes,
        })
    }

    /// In-memory journal (tests, embedders).
    #[must_use]
    pub fn in_memory() -> Self {
        Self {
            path: PathBuf::new(),
            steps: Vec::new(),
            file: None,
            max_bytes: 0,
        }
    }

    /// Append one step durably (line per step). Size-capped rotation:
    /// when the current file already exceeds `max_bytes`, it is
    /// rotated to a single forensic `.1` generation (replacing any
    /// prior one) BEFORE this record is written, so the newest record
    /// always lands in the fresh current file. Replay (`open`) reads
    /// the current file only, which is why `.1` is forensic-only and
    /// correct: the LATEST state (open-turn prompt, last activity
    /// outputs) always lives in current, and a rotated-away activity
    /// output simply falls back to real execution on replay — the
    /// designed behavior for absent history.
    pub fn record(&mut self, step: JournalStep) -> Result<(), DurableError> {
        if self.file.is_some() && crate::rotation::exceeds_cap(&self.path, self.max_bytes) {
            self.rotate()?;
        }
        if let Some(file) = self.file.as_mut() {
            let mut line =
                serde_json::to_vec(&step).map_err(|e| DurableError::Io(e.to_string()))?;
            line.push(b'\n');
            file.write_all(&line)
                .and_then(|_| file.flush())
                .map_err(|e| DurableError::Io(e.to_string()))?;
        }
        self.steps.push(step);
        Ok(())
    }

    /// Rotate at a record boundary: close the append handle, rename
    /// current → current `.1` (atomic; replaces the prior generation,
    /// dropping the OLDEST history), reopen a fresh current file.
    fn rotate(&mut self) -> Result<(), DurableError> {
        self.file = None;
        crate::rotation::rotate(&self.path).map_err(|e| DurableError::Io(e.to_string()))?;
        self.file = Some(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
                .map_err(|e| DurableError::Io(e.to_string()))?,
        );
        Ok(())
    }

    /// Fsync the journal (turn boundary).
    pub fn sync(&mut self) -> Result<(), DurableError> {
        if let Some(file) = self.file.as_mut() {
            file.sync_all()
                .map_err(|e| DurableError::Io(e.to_string()))?;
        }
        Ok(())
    }

    /// Journal path (empty for in-memory journals).
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Replayed steps.
    #[must_use]
    pub fn steps(&self) -> &[JournalStep] {
        &self.steps
    }

    /// Activity outputs by id (latest wins), the replay table.
    #[must_use]
    pub fn completed_outputs(&self) -> HashMap<String, serde_json::Value> {
        let mut map = HashMap::new();
        for step in &self.steps {
            if let JournalStep::ActivityCompleted { id, output, .. } = step {
                map.insert(id.clone(), output.clone());
            }
        }
        map
    }

    /// The prompt of the last (possibly unfinished) turn, if any.
    #[must_use]
    pub fn open_turn_prompt(&self) -> Option<String> {
        let mut open: Option<String> = None;
        for step in &self.steps {
            match step {
                JournalStep::TurnStarted { prompt, .. } => open = Some(prompt.clone()),
                JournalStep::TurnFinished { .. } => open = None,
                _ => {}
            }
        }
        open
    }
}

/// Journal path for a session turn.
#[must_use]
pub fn turn_journal_path(root: &Path, session: &str, turn: u64) -> PathBuf {
    root.join("journal")
        .join(session)
        .join(format!("{turn}.jsonl"))
}

/// Journal path for a child workflow.
#[must_use]
pub fn child_journal_path(root: &Path, session: &str, child: &str) -> PathBuf {
    root.join("journal")
        .join(session)
        .join(format!("child-{child}.jsonl"))
}
