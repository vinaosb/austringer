//! Server stderr capture: lines are bounded, forwarded to the configured
//! hook (if any) or retained in a small ring buffer — server stderr never
//! reaches the host's stderr either way.

use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;

use crate::config::StderrHook;
use crate::lock;
use crate::native::{Inner, STDERR_LINE_CHARS, STDERR_LINE_READ_CAP, STDERR_RING_LINES};

/// Drains `stream` line by line into the hook or the bounded ring.
pub(crate) async fn capture_stderr(
    mut stream: Box<dyn AsyncRead + Unpin + Send>,
    weak: std::sync::Weak<Inner>,
    hook: Option<StderrHook>,
) {
    loop {
        let line = match read_line_bounded(&mut stream).await {
            Ok(Some(line)) => line,
            Ok(None) | Err(_) => break,
        };
        let text: String = String::from_utf8_lossy(&line)
            .chars()
            .take(STDERR_LINE_CHARS)
            .collect();
        if let Some(hook) = &hook {
            hook(&text);
        } else if let Some(inner) = weak.upgrade() {
            let mut tail = lock(&inner.stderr_tail);
            if tail.len() == STDERR_RING_LINES {
                tail.remove(0);
            }
            tail.push(text);
        }
    }
}

/// Reads one line (through `\n`), bounding memory at
/// [`STDERR_LINE_READ_CAP`] bytes; overlong lines are truncated and the
/// remainder discarded.
async fn read_line_bounded(
    stream: &mut (dyn AsyncRead + Unpin + Send),
) -> std::io::Result<Option<Vec<u8>>> {
    let mut line = Vec::with_capacity(128);
    let mut byte = [0u8; 1];
    loop {
        let n = stream.read(&mut byte).await?;
        if n == 0 {
            return if line.is_empty() {
                Ok(None)
            } else {
                Ok(Some(line))
            };
        }
        if byte[0] == b'\n' {
            return Ok(Some(line));
        }
        if line.len() < STDERR_LINE_READ_CAP {
            line.push(byte[0]);
        }
    }
}
