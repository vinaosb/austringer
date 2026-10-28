//! Size-capped, single-generation rotation for the append-only JSONL
//! journals (turn/child journals, `runs.jsonl`, the scheduler
//! journal). Every replay path reads only the CURRENT file, so the
//! rotated `.1` generation is FORENSIC-ONLY history: the latest state
//! always lands in the current file because rotation happens before a
//! record is written, and only the OLDEST history is ever dropped
//! (a new rotation replaces the prior `.1`).
//!
//! Caller contract: rotate only at a record boundary — after one
//! record is fully on disk and before the next is written, so no
//! mid-record corruption is possible.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The retained `.1` generation path for `path` (e.g. `1.jsonl` →
/// `1.jsonl.1`).
#[must_use]
pub fn rotated_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map_or_else(|| OsString::from("journal"), ToOwned::to_owned);
    name.push(".1");
    path.with_file_name(name)
}

/// Whether `path` currently exceeds `max_bytes`. `max_bytes == 0`
/// means unlimited (never over); a missing file is size zero.
#[must_use]
pub fn exceeds_cap(path: &Path, max_bytes: u64) -> bool {
    max_bytes > 0
        && std::fs::metadata(path)
            .map(|meta| meta.len() > max_bytes)
            .unwrap_or(false)
}

/// Atomically rotate `path` to its `.1` generation, REPLACING any
/// prior one (single retained generation — oldest history dropped).
/// The caller must close open handles on `path` first; the rename
/// itself is `std::fs::rename` (atomic, replace-on both platforms).
///
/// # Errors
/// I/O failure from the rename.
pub fn rotate(path: &Path) -> std::io::Result<()> {
    std::fs::rename(path, rotated_path(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotated_path_appends_generation_suffix() {
        assert_eq!(
            rotated_path(Path::new("data/journal/s1/2.jsonl")),
            PathBuf::from("data/journal/s1/2.jsonl.1")
        );
        assert_eq!(
            rotated_path(Path::new("data/runs.jsonl")),
            PathBuf::from("data/runs.jsonl.1")
        );
    }

    #[test]
    fn exceeds_cap_is_false_for_zero_cap_and_missing_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("nope.jsonl");
        assert!(!exceeds_cap(&missing, 0));
        assert!(!exceeds_cap(&missing, 1024));
        let present = dir.path().join("here.jsonl");
        std::fs::write(&present, b"0123456789").expect("write");
        assert!(!exceeds_cap(&present, 10), "exceeds means strictly greater");
        assert!(exceeds_cap(&present, 9));
        assert!(!exceeds_cap(&present, 0), "0 = unlimited");
    }

    #[test]
    fn rotate_replaces_the_prior_generation() -> Result<(), std::io::Error> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("j.jsonl");
        std::fs::write(&path, b"oldest\n")?;
        rotate(&path)?;
        assert_eq!(std::fs::read_to_string(rotated_path(&path))?, "oldest\n");
        // Second rotation replaces the first .1: single generation.
        std::fs::write(&path, b"newest\n")?;
        rotate(&path)?;
        assert_eq!(std::fs::read_to_string(rotated_path(&path))?, "newest\n");
        assert!(!path.exists(), "current must be gone until reopened");
        Ok(())
    }
}
