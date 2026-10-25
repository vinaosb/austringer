//! Append-only session event log: writer, fsync policy, failure contract.

pub mod reader;
pub mod sink;

pub use reader::{Anomaly, LoadReport, replay};
pub use sink::{EventSink, FileSink};

use std::fs;
use std::io;
use std::path::Path;

use crate::event::{Event, EventKind, RecordedEvent, Sequence};

/// File name of the append-only log inside a session directory.
pub const EVENTS_FILE_NAME: &str = "events.jsonl";

/// When the writer fsyncs after appending an event.
///
/// The log always flushes (userspace) per event; the policy controls the
/// durability barrier only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsyncPolicy {
    /// fsync after every appended event.
    Always,
    /// fsync only after snapshot-marker events — the natural restore points.
    OnSnapshot,
    /// Never fsync. Tests only.
    NeverForTests,
}

/// Why an append failed.
#[derive(Debug, thiserror::Error)]
pub enum AppendError {
    /// The write (or its fsync) failed and the session degraded to
    /// read-only safe mode. `notice` is a loud system event to surface
    /// through the API — it is never written to the broken file.
    #[error("append failed; session degraded to read-only: {cause}")]
    DegradedToReadOnly {
        /// The underlying IO failure.
        cause: io::Error,
        /// Loud system notice describing the degradation.
        notice: Event,
    },
    /// The session is already in read-only safe mode.
    #[error("session log is read-only after an earlier failure")]
    ReadOnly,
    /// The event failed to serialize.
    #[error("event serialization failed: {0}")]
    Serialize(String),
    /// The 64-bit sequence space is exhausted.
    #[error("session sequence numbers exhausted")]
    SequenceExhausted,
}

/// Read-only safe-mode state after an IO failure.
#[derive(Debug)]
pub struct Degradation {
    /// What went wrong (display form of the original error).
    pub cause: String,
    /// Loud system notice describing the degradation; surface it in the UI
    /// and the in-memory live view.
    pub notice: Event,
}

/// Append-only writer for `<session>/events.jsonl`.
///
/// The API offers exactly one mutation — [`EventLogWriter::append`]; written
/// events are never updated in place. Sequence numbers are assigned by the
/// writer, start at 1, and increase monotonically per session.
///
/// # Failure contract
///
/// Any IO failure while appending degrades the session to read-only safe
/// mode: further appends fail fast with [`AppendError::ReadOnly`], the file
/// is left holding a valid prefix (a torn tail that survives recovery is
/// ignored by [`replay`]), and the failure surfaces loudly as a system
/// notice event in the API — never as a write into the broken file.
pub struct EventLogWriter {
    sink: Box<dyn EventSink>,
    policy: FsyncPolicy,
    last: Sequence,
    degraded: Option<Degradation>,
}

impl EventLogWriter {
    /// Open a session's log for append. Any torn tail left by a crash is
    /// truncated first: an unterminated final line was never committed, so
    /// the writer resumes at [`LoadReport::valid_len`], the commit boundary.
    pub fn open(session_dir: &Path, policy: FsyncPolicy) -> io::Result<Self> {
        fs::create_dir_all(session_dir)?;
        let path = session_dir.join(EVENTS_FILE_NAME);
        let report = reader::scan(&path)?;
        let sink = FileSink::open(&path, report.valid_len)?;
        let last = report
            .events
            .last()
            .map_or(Sequence(0), |recorded| recorded.seq);
        Ok(Self {
            sink: Box::new(sink),
            policy,
            last,
            degraded: None,
        })
    }

    /// Build a writer over an injected sink (test doubles, exotic storage).
    pub fn with_sink(sink: Box<dyn EventSink>, policy: FsyncPolicy, last: Sequence) -> Self {
        Self {
            sink,
            policy,
            last,
            degraded: None,
        }
    }

    /// Append an event, assigning it the next sequence number.
    pub fn append(&mut self, event: Event) -> Result<Sequence, AppendError> {
        if self.degraded.is_some() {
            return Err(AppendError::ReadOnly);
        }
        let seq = self.last.next().ok_or(AppendError::SequenceExhausted)?;
        let recorded = RecordedEvent { seq, event };
        let mut line = serde_json::to_string(&recorded)
            .map_err(|error| AppendError::Serialize(error.to_string()))?;
        line.push('\n');
        match self.sink.append_line(line.as_bytes()) {
            Ok(()) => {
                self.last = seq;
                let needs_sync = match self.policy {
                    FsyncPolicy::Always => true,
                    FsyncPolicy::OnSnapshot => recorded.event.kind() == EventKind::Snapshot,
                    FsyncPolicy::NeverForTests => false,
                };
                if needs_sync && let Err(cause) = self.sink.sync() {
                    return Err(self.degrade(cause));
                }
                Ok(seq)
            }
            Err(cause) => {
                // Best-effort rollback; a surviving torn tail is tolerated
                // by the reader on the next load.
                let _ = self.sink.discard_partial();
                Err(self.degrade(cause))
            }
        }
    }

    /// Sequence number of the last appended event; `Sequence(0)` when none.
    #[must_use]
    pub fn last_sequence(&self) -> Sequence {
        self.last
    }

    /// True once an IO failure has degraded the session to read-only.
    #[must_use]
    pub fn is_read_only(&self) -> bool {
        self.degraded.is_some()
    }

    /// The degradation state, when the session is read-only.
    #[must_use]
    pub fn degraded(&self) -> Option<&Degradation> {
        self.degraded.as_ref()
    }

    fn degrade(&mut self, cause: io::Error) -> AppendError {
        let notice = Event::System {
            text: format!(
                "EVENT LOG DEGRADED — session is now READ-ONLY. Log write failed: {cause}. \
                 The log retains its last valid prefix; no further events will be written."
            ),
        };
        self.degraded = Some(Degradation {
            cause: cause.to_string(),
            notice: notice.clone(),
        });
        AppendError::DegradedToReadOnly { cause, notice }
    }
}
