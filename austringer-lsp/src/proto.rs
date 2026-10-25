//! LSP base-protocol framing over arbitrary byte streams: `Content-Length`
//! headers plus body, with hard caps so a lying or garbage peer can never
//! make the reader buffer unbounded data ("never OOM" is a typed error).

use std::io;

use thiserror::Error;
use tokio::io::AsyncBufRead;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;

/// Hard bound on the header block of one frame.
pub const DEFAULT_MAX_HEADER_BYTES: usize = 4 * 1024;

/// Hard bound on one frame body. Real LSP frames are KBs; 8 MiB is a generous
/// ceiling that still bounds a lying `Content-Length` instantly.
pub const DEFAULT_MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

/// Bounded-read caps for framing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameCaps {
    /// Maximum bytes for the header block (headers + blank line).
    pub max_header_bytes: usize,
    /// Maximum declared/actual body size.
    pub max_frame_bytes: usize,
}

impl Default for FrameCaps {
    fn default() -> Self {
        Self {
            max_header_bytes: DEFAULT_MAX_HEADER_BYTES,
            max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
        }
    }
}

/// Frame-level failures. `Io` covers transport death (EOF, broken pipe);
/// everything else is protocol corruption, distinguished so callers can
/// decide between restart and hard failure.
#[derive(Debug, Error)]
pub enum FrameError {
    /// Stream ended while the header block was still incomplete.
    #[error("stream ended mid-header")]
    TruncatedHeader,

    /// Stream ended before the declared body length was read.
    #[error("stream ended mid-body")]
    TruncatedBody,

    /// No blank line within the header cap (runaway header / garbage).
    #[error("header block exceeds the {0} byte cap")]
    HeaderOverrun(usize),

    /// Declared body size exceeds the frame cap. Nothing is read.
    #[error("declared Content-Length {declared} exceeds the {cap} byte frame cap")]
    BodyOverrun {
        /// The (untrusted) declared length.
        declared: usize,
        /// The configured cap.
        cap: usize,
    },

    /// The header block ended without a Content-Length line.
    #[error("missing Content-Length header")]
    MissingContentLength,

    /// Content-Length was present but not a number.
    #[error("invalid Content-Length value: {0}")]
    BadContentLength(String),

    /// Transport-level I/O failure.
    #[error("io: {0}")]
    Io(#[from] io::Error),
}

/// Reads exactly one LSP frame and returns its body bytes.
///
/// Both `\r\n` and bare `\n` header terminations are accepted (tolerant
/// readers interoperate with every server in practice). Unknown headers are
/// ignored; a header block without `Content-Length` is
/// [`FrameError::MissingContentLength`]. The header scan is byte-accounted
/// against `max_header_bytes` and the body is only allocated after the
/// declared length passes `max_frame_bytes` — all reads bounded, always.
pub async fn read_frame(
    reader: &mut (dyn AsyncBufRead + Unpin + Send),
    caps: &FrameCaps,
) -> Result<Vec<u8>, FrameError> {
    let mut content_length: Option<usize> = None;
    let mut used = 0usize;

    loop {
        let mut line = Vec::with_capacity(64);
        // Read one header line byte-exactly (fill_buf/consume keeps any
        // buffered body bytes intact for the read_exact below).
        loop {
            if used + line.len() > caps.max_header_bytes {
                return Err(FrameError::HeaderOverrun(caps.max_header_bytes));
            }
            let (chunk_len, newline_at) = {
                let buf = reader.fill_buf().await?;
                if buf.is_empty() {
                    return Err(FrameError::TruncatedHeader);
                }
                match buf.iter().position(|&b| b == b'\n') {
                    Some(pos) => {
                        line.extend_from_slice(&buf[..=pos]);
                        (pos + 1, true)
                    }
                    None => {
                        let n = buf.len();
                        line.extend_from_slice(buf);
                        (n, false)
                    }
                }
            };
            reader.consume(chunk_len);
            used += chunk_len;
            if newline_at {
                break;
            }
        }

        let trimmed = trim_eol(&line);
        if trimmed.is_empty() {
            return match content_length {
                None => Err(FrameError::MissingContentLength),
                Some(len) if len > caps.max_frame_bytes => Err(FrameError::BodyOverrun {
                    declared: len,
                    cap: caps.max_frame_bytes,
                }),
                Some(len) => {
                    let mut body = vec![0u8; len];
                    reader
                        .read_exact(&mut body)
                        .await
                        .map_err(|_| FrameError::TruncatedBody)?;
                    Ok(body)
                }
            };
        }
        if let Some(value) = parse_content_length(trimmed) {
            content_length = Some(value?);
        }
    }
}

/// Writes one LSP frame (`Content-Length` header + body) and flushes.
pub async fn write_frame(
    writer: &mut (dyn AsyncWrite + Unpin + Send),
    body: &[u8],
) -> io::Result<()> {
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    writer.write_all(header.as_bytes()).await?;
    writer.write_all(body).await?;
    writer.flush().await
}

/// Strips one trailing `\n` and optional `\r`.
fn trim_eol(line: &[u8]) -> &[u8] {
    let mut end = line.len();
    if end > 0 && line[end - 1] == b'\n' {
        end -= 1;
    }
    if end > 0 && line[end - 1] == b'\r' {
        end -= 1;
    }
    &line[..end]
}

/// Parses a `Content-Length: N` header line (ASCII case-insensitive).
/// Returns `None` for other headers; `Some(Err)` for a malformed value.
fn parse_content_length(line: &[u8]) -> Option<Result<usize, FrameError>> {
    let text = std::str::from_utf8(line).ok()?;
    let (name, value) = text.split_once(':')?;
    if !name.trim().eq_ignore_ascii_case("content-length") {
        return None;
    }
    match value.trim().parse::<usize>() {
        Ok(n) => Some(Ok(n)),
        Err(_) => Some(Err(FrameError::BadContentLength(value.trim().to_owned()))),
    }
}
