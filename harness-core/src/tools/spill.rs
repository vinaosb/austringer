//! Tool-I/O byte caps with transparent, content-addressed disk spill.
//!
//! Oversized tool results never enter context wholesale: the bytes spill to
//! `<session>/spill/<blake3-hex>` (temp file + fsync + rename, mirroring the
//! media-store crash-safety contract) and the context receives a pointer
//! plus the first N bytes, where N is the configurable inline cap
//! ([`IoCap::DEFAULT`] = 4 KiB). Spilled content is recoverable losslessly
//! by pointer via [`SpillStore::read`]; the in-context preview is rendered
//! lossily so binary or invalid-UTF-8 payloads cannot corrupt the model's
//! view.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::event::ContentHash;

/// Inline byte cap for tool results: payloads at or under the cap pass
/// through untouched; larger payloads spill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IoCap {
    /// Maximum number of result bytes allowed inline in context.
    pub max_inline_bytes: usize,
}

impl IoCap {
    /// The default cap: 4 KiB.
    pub const DEFAULT: Self = Self {
        max_inline_bytes: 4096,
    };
}

impl Default for IoCap {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl IoCap {
    /// Caps one tool result: inline pass-through under the cap, spill over
    /// it. The observation is the model-facing rendering either way.
    pub fn apply(&self, bytes: &[u8], store: &SpillStore) -> io::Result<SpillOutcome> {
        if bytes.len() <= self.max_inline_bytes {
            return Ok(SpillOutcome {
                observation: String::from_utf8_lossy(bytes).into_owned(),
                spill: None,
            });
        }
        let hash = store.store(bytes)?;
        let prefix = &bytes[..self.max_inline_bytes];
        let observation = format!(
            "[tool output spilled: {} bytes exceeded the {}-byte inline cap; first {} bytes shown; full content recoverable by spill pointer {}]\n{}\n[… {} more bytes]",
            bytes.len(),
            self.max_inline_bytes,
            prefix.len(),
            hash,
            String::from_utf8_lossy(prefix),
            bytes.len() - prefix.len(),
        );
        Ok(SpillOutcome {
            observation,
            spill: Some(SpillPointer {
                hash,
                total_bytes: bytes.len() as u64,
                prefix: prefix.to_vec(),
            }),
        })
    }
}

/// Pointer to a spilled tool result: content hash, total size, and the
/// first-N-byte preview that entered context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpillPointer {
    /// Blake3 content hash of the spilled bytes (the recovery key).
    pub hash: ContentHash,
    /// Total byte length of the spilled payload.
    pub total_bytes: u64,
    /// The first `min(cap, total)` bytes shown in context.
    pub prefix: Vec<u8>,
}

/// The capped rendering of one tool result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpillOutcome {
    /// The model-facing observation (inline content, or pointer + preview).
    pub observation: String,
    /// Set when the payload spilled.
    pub spill: Option<SpillPointer>,
}

/// Content-addressed store for oversized tool results under
/// `<session>/spill/`.
pub struct SpillStore {
    dir: PathBuf,
}

impl SpillStore {
    /// Directory name of the spill store inside a session directory.
    pub const DIR_NAME: &'static str = "spill";

    /// Open (creating) the spill directory for a session.
    pub fn open(session_dir: &Path) -> io::Result<Self> {
        let dir = session_dir.join(Self::DIR_NAME);
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    /// Store bytes, returning their content hash. Identical content stored
    /// twice writes nothing the second time (content addressing dedupes).
    pub fn store(&self, bytes: &[u8]) -> io::Result<ContentHash> {
        let hash = ContentHash::from_bytes(bytes);
        let target = self.path_for(&hash);
        if !target.exists() {
            write_atomic(&self.dir, &target, bytes)?;
        }
        Ok(hash)
    }

    /// Load spilled bytes by hash; `Ok(None)` when the hash is unknown.
    pub fn read(&self, hash: &ContentHash) -> io::Result<Option<Vec<u8>>> {
        match fs::read(self.path_for(hash)) {
            Ok(data) => Ok(Some(data)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Filesystem path a hash maps to.
    #[must_use]
    pub fn path_for(&self, hash: &ContentHash) -> PathBuf {
        // The hex form is the canonical `blake3:<64 hex>` ref minus its
        // prefix; stripping is total because `to_ref` always adds it.
        let reference = hash.to_ref();
        let hex = reference.strip_prefix("blake3:").unwrap_or_default();
        self.dir.join(hex)
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
