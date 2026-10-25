//! File-URI encoding/decoding for `textDocument` URIs.
//!
//! Windows paths round-trip through the `file:///C:/...` form. Percent
//! encoding keeps the unreserved set plus `/` and `:` (drive-letter colon)
//! raw — the same shape rust-analyzer, gopls, and clangd emit. Decoding is
//! tolerant: invalid escapes pass through literally, invalid UTF-8 goes
//! lossy. Server-provided URIs are untrusted input and can never panic this
//! code.

use std::path::PathBuf;

/// Encodes a local path as a `file://` URI. Returns `None` for non-UTF-8
/// paths (documented limitation: lossy URIs would not round-trip).
#[must_use]
pub fn file_to_uri(path: &std::path::Path) -> Option<String> {
    let text = path.to_str()?;
    let slashed = text.replace('\\', "/");
    let body = if slashed.starts_with('/') {
        format!("file://{slashed}")
    } else {
        format!("file:///{slashed}")
    };
    Some(percent_encode(&body))
}

/// Decodes a `file://` URI back to a path. Tolerates missing schemes and
/// both 2- and 3-slash authorities.
#[must_use]
pub fn uri_to_file(uri: &str) -> PathBuf {
    let after_scheme = uri.strip_prefix("file://").unwrap_or(uri);
    // "/C:/x" (3-slash, Windows drive) -> "C:/x"; "/home/x" (unix root)
    // stays rooted.
    let path_part = match after_scheme.strip_prefix('/') {
        Some(tail)
            if tail.len() >= 2
                && tail.as_bytes()[0].is_ascii_alphabetic()
                && tail.as_bytes()[1] == b':' =>
        {
            tail
        }
        _ => after_scheme,
    };
    PathBuf::from(percent_decode(path_part))
}

/// Percent-encodes everything outside the URI unreserved set, keeping `/`
/// and `:` raw.
fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for &byte in text.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                out.push(byte as char);
            }
            _ => {
                const HEX: &[u8; 16] = b"0123456789ABCDEF";
                out.push('%');
                out.push(HEX[(byte >> 4) as usize] as char);
                out.push(HEX[(byte & 0x0F) as usize] as char);
            }
        }
    }
    out
}

/// Percent-decodes `%XX` sequences. Invalid escapes stay literal.
fn percent_decode(text: &str) -> String {
    fn hex_val(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }

    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (
                bytes.get(i + 1).copied().and_then(hex_val),
                bytes.get(i + 2).copied().and_then(hex_val),
            )
        {
            out.push((hi << 4) | lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
