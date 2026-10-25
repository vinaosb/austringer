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

/// Append-only turn journal on disk.
pub struct Journal {
    path: PathBuf,
    steps: Vec<JournalStep>,
    file: Option<std::fs::File>,
}

impl Journal {
    /// Open (creating directories as needed). Replays prior steps;
    /// torn/malformed lines are skipped, never abort the load.
    pub fn open(path: &Path) -> Result<Self, DurableError> {
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
        })
    }

    /// In-memory journal (tests, embedders).
    #[must_use]
    pub fn in_memory() -> Self {
        Self {
            path: PathBuf::new(),
            steps: Vec::new(),
            file: None,
        }
    }

    /// Append one step durably (line per step).
    pub fn record(&mut self, step: JournalStep) -> Result<(), DurableError> {
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
