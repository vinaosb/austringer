//! Content-addressed, out-of-line media storage for image payloads.
//!
//! Image bytes never ride inside the JSONL: they are written once to
//! `<session>/media/<blake3-hex>` (temp file + fsync + rename, so a crash
//! never leaves a half-written media file under its final name) and are
//! referenced from events by content hash.

use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use super::content::{ImageBlock, MimeType};

/// Reasons a content-hash string fails to parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvalidContentHash {
    /// Missing the `blake3:` prefix.
    #[error("content hash must start with 'blake3:'")]
    MissingPrefix,
    /// Wrong length after the prefix (must be 64 hex characters).
    #[error("content hash must be 64 hex characters after the prefix")]
    BadLength,
    /// Non-hexadecimal characters.
    #[error("content hash contains non-hexadecimal characters")]
    BadHex,
}

/// Blake3 content hash (256-bit), serialized as `blake3:<64 hex chars>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    /// Hash arbitrary bytes with Blake3.
    #[must_use]
    pub fn from_bytes(data: &[u8]) -> Self {
        Self(*blake3::hash(data).as_bytes())
    }

    /// Canonical string form `blake3:<hex>`.
    #[must_use]
    pub fn to_ref(&self) -> String {
        format!("blake3:{}", self.hex())
    }

    /// Shortened form for placeholder markers: `blake3:<first 16 hex>…`.
    #[must_use]
    pub fn short_ref(&self) -> String {
        let hex = self.hex();
        format!("blake3:{}…", &hex[..16])
    }

    /// Parse the canonical `blake3:<hex>` form.
    pub fn parse(form: &str) -> Result<Self, InvalidContentHash> {
        let Some(hex) = form.strip_prefix("blake3:") else {
            return Err(InvalidContentHash::MissingPrefix);
        };
        if hex.len() != 64 {
            return Err(InvalidContentHash::BadLength);
        }
        let mut digest = [0u8; 32];
        for (index, byte) in digest.iter_mut().enumerate() {
            let high = hex_digit(hex.as_bytes()[index * 2]).ok_or(InvalidContentHash::BadHex)?;
            let low = hex_digit(hex.as_bytes()[index * 2 + 1]).ok_or(InvalidContentHash::BadHex)?;
            *byte = (high << 4) | low;
        }
        Ok(Self(digest))
    }

    fn hex(&self) -> String {
        fn digit(value: u8) -> char {
            match value {
                0..=9 => (b'0' + value) as char,
                _ => (b'a' + value - 10) as char,
            }
        }
        let mut out = String::with_capacity(64);
        for byte in self.0 {
            out.push(digit(byte >> 4));
            out.push(digit(byte & 0x0F));
        }
        out
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_ref())
    }
}

impl From<ContentHash> for String {
    fn from(hash: ContentHash) -> Self {
        hash.to_ref()
    }
}

impl TryFrom<String> for ContentHash {
    type Error = InvalidContentHash;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl FromStr for ContentHash {
    type Err = InvalidContentHash;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Reference to out-of-line media: content hash plus byte length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaRef {
    /// Blake3 content hash of the stored bytes.
    pub hash: ContentHash,
    /// Length of the stored bytes.
    pub bytes: u64,
}

/// Content-addressed store for image bytes under `<session>/media/`.
pub struct MediaStore {
    dir: PathBuf,
}

impl MediaStore {
    /// Directory name of the media store inside a session directory.
    pub const DIR_NAME: &'static str = "media";

    /// Open (creating) the media directory for a session.
    pub fn open(session_dir: &Path) -> io::Result<Self> {
        let dir = session_dir.join(Self::DIR_NAME);
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    /// Store bytes, returning the referencing image block.
    ///
    /// Storing identical content twice writes nothing the second time
    /// (content addressing gives dedup for free).
    pub fn store(
        &self,
        mime: &MimeType,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> io::Result<ImageBlock> {
        let hash = ContentHash::from_bytes(data);
        let target = self.path_for(&hash);
        if !target.exists() {
            write_atomic(&self.dir, &target, data)?;
        }
        Ok(ImageBlock {
            mime: mime.clone(),
            width,
            height,
            media: MediaRef {
                hash,
                bytes: data.len() as u64,
            },
        })
    }

    /// Load stored bytes by hash; `Ok(None)` when the hash is unknown.
    pub fn load(&self, hash: &ContentHash) -> io::Result<Option<Vec<u8>>> {
        match fs::read(self.path_for(hash)) {
            Ok(data) => Ok(Some(data)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Filesystem path a hash maps to.
    #[must_use]
    pub fn path_for(&self, hash: &ContentHash) -> PathBuf {
        self.dir.join(hash.hex())
    }
}

/// Durable atomic file write: temp file in the same directory, fsync, then
/// rename over the target. A rename target that already exists (identical
/// content stored concurrently) counts as success.
fn write_atomic(dir: &Path, target: &Path, data: &[u8]) -> io::Result<()> {
    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
    let temp = dir.join(format!(
        ".tmp-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = File::create(&temp)?;
        file.write_all(data)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, target)
    })();
    match result {
        Ok(()) => Ok(()),
        Err(_) if target.exists() => {
            let _ = fs::remove_file(&temp);
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_file(&temp);
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_parse_roundtrips_when_canonical_form_used() {
        let hash = ContentHash::from_bytes(b"harness");
        let parsed = ContentHash::parse(&hash.to_ref());
        assert_eq!(parsed, Ok(hash));
        assert_eq!(
            ContentHash::parse("sha256:abcd"),
            Err(InvalidContentHash::MissingPrefix)
        );
        assert_eq!(
            ContentHash::parse("blake3:abcd"),
            Err(InvalidContentHash::BadLength)
        );
        assert_eq!(
            ContentHash::parse(&format!("blake3:{}", "z".repeat(64))),
            Err(InvalidContentHash::BadHex)
        );
    }
}
