//! Streaming spill of oversized raw PTY output (plan todo 19, dsh/Kilo
//! pattern: spill-not-truncate).
//!
//! The raw byte stream is buffered in memory only up to the [`IoCap`]
//! inline cap. Beyond it, every byte streams to a temp file inside the
//! session's [`SpillStore`] directory while a `blake3` hash accumulates
//! incrementally; `finish` fsyncs and renames the temp file to the store's
//! content-addressed path, so the pointer is recoverable via
//! [`SpillStore::read`] with the exact T6 layout. (T6's
//! `SpillStore::store(&[u8])` requires the whole payload in memory, which
//! would turn a 100 MB output into a 100 MB allocation — the streaming
//! writer reuses the store's directory contract and recovery path but not
//! its in-memory `store` call. `IoCap` is reused as the threshold so both
//! spill mechanisms agree on what "oversized" means.)
//!
//! Spilled bytes are the raw PTY stream (control sequences included): a
//! lossless recovery record. They never enter context — see
//! [`crate::spec::ExecOutcome`].

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::PathBuf;

use austringer_core::event::ContentHash;
use austringer_core::tools::{IoCap, SpillStore};

use crate::fsutil::{RetryBudget, long_path, retry_sharing_violation};

/// Pointer to a spilled raw stream: recovery key and size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpilledOutput {
    /// Blake3 content hash of the raw byte stream (the recovery key).
    pub hash: ContentHash,
    /// Total raw bytes captured.
    pub raw_bytes: u64,
}

enum Sink {
    Buffering(Vec<u8>),
    Streaming { file: File, temp: PathBuf },
}

/// Buffers raw bytes in memory up to the cap, then streams to disk.
pub struct RawSpool {
    store: Option<SpillStore>,
    cap: IoCap,
    sink: Option<Sink>,
    hasher: blake3::Hasher,
    total: u64,
}

impl RawSpool {
    /// A spool over the given session spill store.
    #[must_use]
    pub fn new(store: SpillStore, cap: IoCap) -> Self {
        Self {
            store: Some(store),
            cap,
            sink: Some(Sink::Buffering(Vec::new())),
            hasher: blake3::Hasher::new(),
            total: 0,
        }
    }

    /// Appends raw bytes.
    pub fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.hasher.update(bytes);
        self.total += bytes.len() as u64;
        match self.sink.take() {
            Some(Sink::Buffering(mut buffer)) => {
                buffer.extend_from_slice(bytes);
                if buffer.len() > self.cap.max_inline_bytes {
                    self.start_stream(&buffer)?;
                } else {
                    self.sink = Some(Sink::Buffering(buffer));
                }
                Ok(())
            }
            Some(mut stream @ Sink::Streaming { .. }) => {
                let result = match &mut stream {
                    Sink::Streaming { file, .. } => file.write_all(bytes),
                    // Structural only: `stream` was just matched Streaming.
                    Sink::Buffering(_) => Ok(()),
                };
                self.sink = Some(stream);
                result
            }
            None => Ok(()),
        }
    }

    /// Total raw bytes seen so far.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.total
    }

    /// Content hash of everything written so far.
    #[must_use]
    pub fn pointer_hash(&self) -> ContentHash {
        let digest = self.hasher.finalize();
        let reference = format!("blake3:{}", digest.to_hex());
        match ContentHash::parse(&reference) {
            Ok(hash) => hash,
            // Unreachable for blake3's own lowercase-hex output; the
            // fallback keeps the function total because lib code never
            // panics.
            Err(_) => ContentHash::from_bytes(digest.as_bytes()),
        }
    }

    /// Finalizes the spool; `Some` when the stream exceeded the cap and was
    /// persisted (recoverable via [`SpillStore::read`]).
    pub fn finish(mut self) -> io::Result<Option<SpilledOutput>> {
        let hash = self.pointer_hash();
        let total = self.total;
        match self.sink.take() {
            Some(Sink::Buffering(_)) => Ok(None),
            Some(Sink::Streaming { file, temp }) => {
                file.sync_all()?;
                drop(file);
                let Some(store) = self.store.as_ref() else {
                    let _ = fs::remove_file(&temp);
                    return Ok(None);
                };
                let target = long_path(&store.path_for(&hash));
                let temp = long_path(&temp);
                if let Some(parent) = target.parent() {
                    let parent = parent.to_path_buf();
                    retry_sharing_violation(&RetryBudget::default(), || {
                        fs::create_dir_all(&parent)
                    })?;
                }
                // Content addressing: an identical stream spilled
                // concurrently makes the losing rename a no-op success.
                let rename =
                    retry_sharing_violation(&RetryBudget::default(), || fs::rename(&temp, &target));
                if let Err(error) = rename {
                    if !target.exists() {
                        return Err(error);
                    }
                    let _ = fs::remove_file(&temp);
                }
                Ok(Some(SpilledOutput {
                    hash,
                    raw_bytes: total,
                }))
            }
            None => Ok(None),
        }
    }

    fn start_stream(&mut self, buffered: &[u8]) -> io::Result<()> {
        let Some(store) = self.store.as_ref() else {
            self.sink = None;
            return Ok(());
        };
        let temp = long_path(
            &store
                .path_for(&self.pointer_hash())
                .with_extension(format!("tmp-{}", std::process::id())),
        );
        let temp_for_create = temp.clone();
        let mut file =
            retry_sharing_violation(&RetryBudget::default(), || File::create(&temp_for_create))?;
        file.write_all(buffered)?;
        self.sink = Some(Sink::Streaming { file, temp });
        Ok(())
    }
}

impl Drop for RawSpool {
    fn drop(&mut self) {
        // Best-effort temp cleanup for unfinalized spools (killed runs).
        if let Some(Sink::Streaming { temp, .. }) = &self.sink {
            let _ = fs::remove_file(temp);
        }
    }
}
