//! The spawned-effects journal: append-only JSONL rows for every launch
//! effect. The step vocabulary (`child_started` / `child_completed`) is
//! aligned with the daemon's durable-workflow `JournalStep` tags so a
//! daemon-side bridge (T45 integration) can consume these rows without a
//! vocabulary fork; this crate's records are a documented superset (the
//! `resumed` flag on `child_started`).

use std::io::Write;
use std::path::{Path, PathBuf};

/// blake3 hex fingerprint of spec bytes (stale-reload detection).
pub fn fingerprint_bytes(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// A child-session id is any id in the `{launch}-child-{task}` namespace
/// (hidden-from-main-list marker).
pub fn is_child_session(id: &str) -> bool {
    id.contains("-child-")
}

/// One journal row.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JournalRecord {
    /// The spec was loaded and resolved.
    SpecLoaded {
        /// Spec fingerprint at resolve time.
        fingerprint: String,
        /// Task count.
        tasks: u32,
    },
    /// A wave started.
    WaveStarted {
        /// Wave index.
        index: u32,
        /// Child ids.
        children: Vec<String>,
    },
    /// A child spawned (daemon-vocabulary tag).
    ChildStarted {
        /// Child id.
        child: String,
        /// Serialized spawn spec (model/effort/role/writes).
        spec: String,
        /// Resume of an existing child id (Kilo task_id resumability).
        #[serde(default)]
        resumed: bool,
    },
    /// A child completed (daemon-vocabulary tag).
    ChildCompleted {
        /// Child id.
        child: String,
        /// The clamped summary (1-2k band).
        result: String,
    },
    /// A child failed.
    ChildFailed {
        /// Child id.
        child: String,
        /// Failure class wire name.
        class: String,
        /// Failure detail.
        detail: String,
    },
    /// Spawning stopped early.
    BudgetStopped {
        /// Why.
        reason: String,
        /// Children completed so far.
        completed: Vec<String>,
    },
    /// An escalation was decided.
    Escalation {
        /// Child id.
        child: String,
        /// Granted?
        granted: bool,
    },
    /// Siblings collided on a path.
    Collision {
        /// Path.
        path: String,
        /// First toucher.
        a: String,
        /// Second toucher.
        b: String,
    },
    /// The model family changed between waves.
    FamilySwitch {
        /// Family left.
        from: String,
        /// Family entered.
        to: String,
    },
    /// The spec mutated mid-run.
    SpecStale {
        /// Expected fingerprint.
        expected: String,
        /// Observed fingerprint.
        actual: String,
    },
    /// The launch finished.
    LaunchFinished {
        /// Rolled-up summary.
        summary: String,
        /// Completed without early stop?
        complete: bool,
    },
}

/// Journal failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JournalError {
    /// I/O failure.
    #[error("journal io: {0}")]
    Io(String),
    /// A mid-file row failed to parse.
    #[error("journal corrupt at line {line}")]
    Corrupt {
        /// 1-based line number.
        line: usize,
    },
}

/// The journal: JSONL file (or in memory).
#[derive(Debug)]
pub struct TeamJournal {
    path: Option<PathBuf>,
    file: Option<std::fs::File>,
    records: Vec<JournalRecord>,
}

impl Default for TeamJournal {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl TeamJournal {
    /// Opens (creates) an on-disk journal.
    pub fn open(path: &Path) -> Result<Self, JournalError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| JournalError::Io(format!("{}: {e}", parent.display())))?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| JournalError::Io(format!("{}: {e}", path.display())))?;
        Ok(Self {
            path: Some(path.to_path_buf()),
            file: Some(file),
            records: Vec::new(),
        })
    }

    /// An in-memory journal.
    pub fn in_memory() -> Self {
        Self {
            path: None,
            file: None,
            records: Vec::new(),
        }
    }

    /// The journal path, if on disk.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Appends a row (immediate flush — the journal must survive a crash
    /// mid-wave).
    pub fn record(&mut self, record: JournalRecord) -> Result<(), JournalError> {
        let line =
            serde_json::to_string(&record).map_err(|e| JournalError::Io(format!("encode: {e}")))?;
        if let Some(file) = self.file.as_mut() {
            writeln!(file, "{line}").map_err(|e| JournalError::Io(e.to_string()))?;
            file.flush().map_err(|e| JournalError::Io(e.to_string()))?;
        }
        self.records.push(record);
        Ok(())
    }

    /// All rows this process appended.
    pub fn records(&self) -> &[JournalRecord] {
        &self.records
    }
}

/// Loads journal rows from disk; a torn LAST line (crash mid-write) is
/// skipped, mid-file corruption is a typed error.
pub fn load_records(path: &Path) -> Result<Vec<JournalRecord>, JournalError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| JournalError::Io(format!("{}: {e}", path.display())))?;
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let mut out = Vec::with_capacity(lines.len());
    for (index, line) in lines.iter().enumerate() {
        match serde_json::from_str(line) {
            Ok(record) => out.push(record),
            Err(_) if index == lines.len() - 1 => {}
            Err(_) => return Err(JournalError::Corrupt { line: index + 1 }),
        }
    }
    Ok(out)
}
