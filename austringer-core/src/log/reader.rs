//! Crash-tolerant replay of the append-only JSONL log.

use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use super::EVENTS_FILE_NAME;
use crate::event::{RecordedEvent, Sequence};

/// A recoverable anomaly observed while replaying a session.
///
/// Anomalies never abort a load: the reader reports them and returns every
/// event it could recover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Anomaly {
    /// The final line lacks its newline (crash mid-write). It was never
    /// committed; `bytes` is its length.
    TornFinalLine {
        /// Length of the ignored partial line in bytes.
        bytes: usize,
    },
    /// A newline-terminated line failed to parse as a recorded event.
    InvalidLine {
        /// 1-based line number.
        line: u64,
        /// Why the line failed to parse.
        reason: String,
    },
    /// A line's sequence number skipped ahead.
    UnexpectedSequence {
        /// 1-based line number.
        line: u64,
        /// The sequence number the log should have carried.
        expected: Sequence,
        /// The sequence number actually found.
        found: Sequence,
    },
}

/// The result of replaying a session log.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LoadReport {
    /// Every recovered event, in sequence order.
    pub events: Vec<RecordedEvent>,
    /// Recoverable anomalies (torn tails, invalid lines, sequence jumps).
    pub anomalies: Vec<Anomaly>,
    /// Byte length of the valid prefix: everything up to and including the
    /// last newline-terminated line. A torn tail beyond this length was
    /// never committed; writers must truncate to here before appending.
    pub valid_len: u64,
}

/// Replay `<session>/events.jsonl`.
///
/// A missing session directory or log file is an empty session, not an
/// error. Structural problems come back as [`Anomaly`] values — never as
/// errors, never panics.
pub fn replay(session_dir: &Path) -> io::Result<LoadReport> {
    scan(&session_dir.join(EVENTS_FILE_NAME))
}

/// Scan one log file. Shared by [`replay`] and the writer's open path.
pub(super) fn scan(path: &Path) -> io::Result<LoadReport> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(LoadReport::default());
        }
        Err(error) => return Err(error),
    };
    let mut reader = BufReader::new(file);
    let mut report = LoadReport::default();
    let mut expected = Sequence::FIRST;
    let mut line_no: u64 = 0;
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        if reader.read_until(b'\n', &mut buffer)? == 0 {
            break;
        }
        line_no += 1;
        if buffer.last() != Some(&b'\n') {
            // Unterminated final line: uncommitted, ignored, reported.
            report.anomalies.push(Anomaly::TornFinalLine {
                bytes: buffer.len(),
            });
            break;
        }
        buffer.pop();
        match serde_json::from_slice::<RecordedEvent>(&buffer) {
            Ok(recorded) => {
                if recorded.seq != expected {
                    report.anomalies.push(Anomaly::UnexpectedSequence {
                        line: line_no,
                        expected,
                        found: recorded.seq,
                    });
                }
                expected = recorded.seq.next().unwrap_or(Sequence(u64::MAX));
                report.events.push(recorded);
            }
            Err(error) => report.anomalies.push(Anomaly::InvalidLine {
                line: line_no,
                reason: error.to_string(),
            }),
        }
        // Newline-terminated lines — valid or anomalous — are committed.
        report.valid_len += buffer.len() as u64 + 1;
    }
    Ok(report)
}
