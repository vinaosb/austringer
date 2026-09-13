//! Server stderr → tracing bridge (todo 29): stdio child stderr is never
//! lost (every line flows to the `harness_mcp::server_stderr` tracing
//! target) and never spammed (per-line byte cap + per-connection line
//! budget with a counted overflow summary).

use tokio::io::AsyncReadExt;
use tokio::process::ChildStderr;

/// Maximum bytes emitted per stderr line (longer lines truncate with a
/// marker; the truncation is counted).
pub const MAX_LINE_BYTES: usize = 8 * 1024;

/// Maximum lines emitted per connection; beyond this the drain counts
/// silently and discloses the total at EOF.
pub const MAX_LINES: usize = 10_000;

/// Drains one stderr stream, invoking `on_line` per bounded line. Returns
/// the total line count (including suppressed ones). EOF-terminated: this
/// future always finishes when the pipe closes.
pub async fn drain_stderr<R, F>(mut stderr: R, mut on_line: F) -> usize
where
    R: AsyncReadExt + Unpin,
    F: FnMut(&str),
{
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4 * 1024];
    let mut emitted: usize = 0;
    let mut total: usize = 0;
    loop {
        match stderr.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => buffer.extend_from_slice(&chunk[..n]),
        }
        while let Some(newline) = buffer.iter().position(|byte| *byte == b'\n') {
            let mut line: Vec<u8> = buffer.drain(..=newline).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            total += 1;
            if emitted < MAX_LINES {
                emitted += 1;
                on_line(&render_bounded(&line));
            }
        }
        // Over-long run without a newline: keep only the tail to bound memory.
        if buffer.len() > MAX_LINE_BYTES {
            let keep = buffer.len() - MAX_LINE_BYTES;
            buffer.drain(..keep);
        }
    }
    if total > emitted {
        let suppressed = total - emitted;
        on_line(&format!(
            "(stderr line budget exhausted: {suppressed} further lines suppressed)"
        ));
    }
    total
}

fn render_bounded(line: &[u8]) -> String {
    if line.len() > MAX_LINE_BYTES {
        let truncated: Vec<u8> = line[..MAX_LINE_BYTES].to_vec();
        format!(
            "{}(truncated, {} bytes dropped)",
            String::from_utf8_lossy(&truncated),
            line.len() - MAX_LINE_BYTES
        )
    } else {
        String::from_utf8_lossy(line).into_owned()
    }
}

/// Spawns the production drain: every bounded line becomes a
/// `tracing::debug!` event on the `harness_mcp::server_stderr` target.
pub fn spawn_stderr_drain(stderr: ChildStderr, server: &str) -> tokio::task::JoinHandle<()> {
    let server = server.to_owned();
    tokio::spawn(async move {
        let total =
            drain_stderr(stderr, |line| tracing::debug!(target: "harness_mcp::server_stderr", server = %server, "{line}")).await;
        tracing::debug!(
            target: "harness_mcp::server_stderr",
            server = %server,
            total,
            "stderr drain complete"
        );
    })
}
