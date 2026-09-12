//! Byte-level sink abstraction for the append-only log.
//!
//! The [`EventSink`] trait exists so IO failures are injectable: production
//! writes through [`FileSink`]; tests provide doubles that simulate
//! disk-full, torn mid-writes, and fsync failures against a real backing
//! file.

use std::fs::{File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Write side of the session log: appends complete JSONL lines.
pub trait EventSink {
    /// Append one complete line (newline included) as a single write, then
    /// flush.
    ///
    /// On `Err` the sink may have leaked a partial (torn) line; the caller
    /// will invoke [`EventSink::discard_partial`] to attempt recovery, and
    /// the reader tolerates any torn tail that survives.
    fn append_line(&mut self, line: &[u8]) -> io::Result<()>;

    /// Force durability of everything appended so far.
    fn sync(&mut self) -> io::Result<()>;

    /// Best-effort rollback to the end of the last fully-appended line.
    fn discard_partial(&mut self) -> io::Result<()>;
}

/// File-backed production sink.
///
/// `committed` advances only after a fully successful append, so
/// [`FileSink::discard_partial`] always restores the valid prefix.
pub struct FileSink {
    file: File,
    path: PathBuf,
    committed: u64,
}

impl FileSink {
    /// Open (creating) a log file whose valid prefix is `committed_len`
    /// bytes: any uncommitted torn tail beyond that length is cut away and
    /// the write position starts exactly at the commit boundary.
    pub fn open(path: &Path, committed_len: u64) -> io::Result<Self> {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        file.set_len(committed_len)?;
        file.seek(SeekFrom::Start(committed_len))?;
        Ok(Self {
            file,
            path: path.to_path_buf(),
            committed: committed_len,
        })
    }

    /// Path of the backing file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl EventSink for FileSink {
    fn append_line(&mut self, line: &[u8]) -> io::Result<()> {
        self.file.write_all(line)?;
        self.file.flush()?;
        self.committed += line.len() as u64;
        Ok(())
    }

    fn sync(&mut self) -> io::Result<()> {
        self.file.sync_data()
    }

    fn discard_partial(&mut self) -> io::Result<()> {
        self.file.set_len(self.committed)?;
        self.file.seek(SeekFrom::Start(self.committed))?;
        Ok(())
    }
}
