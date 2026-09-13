//! Import: bulk event ingestion and tolerant JSONL intake.

use std::path::Path;

use harness_core::event::{Event, RecordedEvent};

use crate::error::SessionError;
use crate::layout::validate_id;
use crate::meta::SessionMeta;
use crate::store::{SessionStore, constraint, state_bump};

/// Where an imported session came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportOrigin {
    /// Source label (`claude-code`, `codex`, `jsonl`, …).
    pub source: String,
    /// Count of foreign entries that could not be mapped losslessly.
    pub unknown: u64,
}

impl ImportOrigin {
    /// Origin tag for the native JSONL round-trip.
    #[must_use]
    pub fn jsonl() -> Self {
        Self {
            source: "jsonl".to_owned(),
            unknown: 0,
        }
    }

    /// Origin tag for a foreign reader result.
    #[must_use]
    pub fn foreign(source: &str, unknown: u64) -> Self {
        Self {
            source: source.to_owned(),
            unknown,
        }
    }
}

/// Report of a JSONL import: anomalies never abort the import.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportReport {
    /// The imported session's metadata.
    pub meta: SessionMeta,
    /// Newline-terminated lines that failed to parse.
    pub invalid_lines: u64,
    /// Whether a torn (unterminated) tail line was skipped.
    pub torn_tail: bool,
}

impl SessionStore {
    /// Bulk-imports events as a new session (foreign readers, JSONL) in
    /// one atomic transaction.
    ///
    /// # Errors
    /// Typed: invalid id, already exists, storage.
    pub fn import_events(
        &self,
        id: &str,
        title: &str,
        events: &[Event],
        origin: &ImportOrigin,
    ) -> Result<SessionMeta, SessionError> {
        validate_id(id)?;
        self.layout().ensure_session(id)?;
        let now = self.now();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction().map_err(SessionError::sqlite)?;
        tx.execute(
            "INSERT INTO sessions (id, title, created_at, updated_at, imported_from, imported_unknown)
             VALUES (?1, ?2, ?3, ?3, ?4, ?5)",
            rusqlite::params![id, title, now as i64, origin.source, origin.unknown as i64],
        )
        .map_err(|error| constraint(error, id))?;
        for event in events {
            crate::session::append_in_tx(&tx, id, None, event, now)?;
        }
        state_bump(&tx, "sessions_since_dream", 1)?;
        tx.commit().map_err(SessionError::sqlite)?;
        drop(conn);
        self.load_meta(id)
    }

    /// Imports a T4-format JSONL file as a new session. A torn
    /// (unterminated) tail line is skipped and reported; invalid
    /// newline-terminated lines are counted, never fatal.
    ///
    /// # Errors
    /// Typed: invalid id, already exists, unreadable file.
    pub fn import_jsonl(
        &self,
        id: &str,
        title: &str,
        path: &Path,
        origin: &ImportOrigin,
    ) -> Result<ImportReport, SessionError> {
        let raw = std::fs::read(path).map_err(SessionError::io)?;
        let ends_with_newline = raw.last() == Some(&b'\n');
        let segments: Vec<&[u8]> = raw.split(|&b| b == b'\n').collect();
        let mut events = Vec::new();
        let mut invalid_lines = 0u64;
        let mut torn_tail = false;
        for (i, line) in segments.iter().enumerate() {
            if line.is_empty() {
                continue;
            }
            let trimmed = line.strip_suffix(b"\r").unwrap_or(line);
            let is_last = i == segments.len() - 1;
            match serde_json::from_slice::<RecordedEvent>(trimmed) {
                Ok(recorded) => events.push(recorded.event),
                Err(_) if is_last && !ends_with_newline => torn_tail = true,
                Err(_) => invalid_lines += 1,
            }
        }
        let meta = self.import_events(id, title, &events, origin)?;
        Ok(ImportReport {
            meta,
            invalid_lines,
            torn_tail,
        })
    }
}
