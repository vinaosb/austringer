//! The headless terminal's side of the terminal query contract (todo 19,
//! hardened todo 20).
//!
//! ConPTY (and terminal-aware children like vim) send queries — cursor
//! position (`ESC [ 6 n`), primary device attributes (`ESC [ c`) — and
//! BLOCK until a response arrives; on Windows conhost's handshake can
//! stall the whole child for ~60 s per unanswered query. A real terminal
//! answers; this runner is the terminal, so queries are answered
//! immediately (cursor reports come straight from the screen model).
//!
//! todo-20 design note (measured, see `tests/pathologies.rs`): answering
//! reactively — the moment the query bytes arrive on the master — closes
//! the 60 s window to single-digit milliseconds, because the pump is
//! already reading when the child sends its first query. PROACTIVE
//! pre-injection (writing a DA1 reply into the pty before any query) was
//! rejected: a child that never queries would consume the stray reply as
//! keyboard input. The pathology fixture pins the reactive latency bound.

use std::io::Write;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::screen::ScreenModel;

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // Poisoning only happens after a reader panic; the shared state
    // stays consistent for reading regardless.
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Answers every query found in `stream` (leaving the unconsumed tail in
/// place for split-read queries).
pub(crate) fn answer_queries(
    stream: &mut Vec<u8>,
    writer: &mut dyn Write,
    screen: &Arc<Mutex<Option<ScreenModel>>>,
) {
    let mut consumed = 0;
    while let Some((offset, kind)) = find_query(&stream[consumed..]) {
        consumed += offset + query_len(&kind);
        let response = match kind {
            // DSR cursor position: report the model's tracked cursor.
            Query::CursorPosition => {
                let (row, col) = lock(screen).as_ref().map_or((0, 0), ScreenModel::cursor);
                format!("\x1b[{};{}R", row + 1, col + 1).into_bytes()
            }
            // DA1: "VT100 with AVO" — the minimal honest identity.
            Query::DeviceAttributes => b"\x1b[?1;2c".to_vec(),
        };
        let _ = writer.write_all(&response);
        let _ = writer.flush();
    }
    stream.drain(..consumed);
}

enum Query {
    CursorPosition,
    DeviceAttributes,
}

fn query_len(kind: &Query) -> usize {
    match kind {
        Query::CursorPosition => 4,
        Query::DeviceAttributes => 3,
    }
}

fn find_query(bytes: &[u8]) -> Option<(usize, Query)> {
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != 0x1b {
            continue;
        }
        let window = &bytes[index..];
        if window.starts_with(b"\x1b[6n") {
            return Some((index, Query::CursorPosition));
        }
        if window.starts_with(b"\x1b[c") || window.starts_with(b"\x1b[0c") {
            return Some((index, Query::DeviceAttributes));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct CollectingWriter(Vec<u8>);
    impl Write for CollectingWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn answer_in_chunks(chunks: &[&[u8]]) -> Vec<u8> {
        let screen = Arc::new(Mutex::new(Some(ScreenModel::new(10, 5))));
        let mut writer = CollectingWriter(Vec::new());
        let mut carry = Vec::new();
        for chunk in chunks {
            carry.extend_from_slice(chunk);
            answer_queries(&mut carry, &mut writer, &screen);
            let keep = carry.len().saturating_sub(5);
            carry.drain(..keep);
        }
        writer.0
    }

    #[test]
    fn da1_and_dsr_queries_each_get_exactly_one_answer() {
        let replies = answer_in_chunks(&[b"noise\x1b[c more\x1b[6n tail"]);
        assert_eq!(replies, b"\x1b[?1;2c\x1b[1;1R");
    }

    #[test]
    fn a_query_split_across_reads_is_still_answered_once() {
        let replies = answer_in_chunks(&[b"pre\x1b[", b"0", b"c after"]);
        assert_eq!(replies, b"\x1b[?1;2c");
    }

    #[test]
    fn non_query_escapes_produce_no_traffic() {
        let replies = answer_in_chunks(&[b"\x1b[2J\x1b[1;1Hplain"]);
        assert!(replies.is_empty());
    }
}
