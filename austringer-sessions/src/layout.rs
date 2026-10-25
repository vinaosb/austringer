//! Session-state directory contract (Copilot §20): every session owns
//! `{session-root}/{id}/` containing `checkpoints/`, `plan.md`, `files/`,
//! and `events.jsonl`. The store's SQLite source of truth lives at
//! `{session-root}/store.db`; archived cold sessions under
//! `{session-root}/archive/`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::SessionError;

/// Names that live beside session directories inside the root.
const RESERVED: [&str; 2] = ["store.db", "archive"];

/// Windows reserved device names (a session id becomes a directory).
const WINDOWS_RESERVED: [&str; 21] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT9",
];

/// Validates a session id as a store key and directory name. Unicode
/// letters and digits are welcome; path breakers, separators, control
/// characters, reserved device names, and the store's own file names are
/// not.
pub fn validate_id(id: &str) -> Result<(), SessionError> {
    let reject = |reason: &'static str| {
        Err(SessionError::InvalidId {
            id: id.to_owned(),
            reason,
        })
    };
    if id.is_empty() {
        return reject("empty");
    }
    if id.len() > 64 {
        return reject("longer than 64 bytes");
    }
    if id.starts_with('.') || id.ends_with('.') || id.starts_with(' ') || id.ends_with(' ') {
        return reject("leading/trailing dot or space");
    }
    if id.bytes().any(|b| {
        b.is_ascii_control()
            || matches!(
                b,
                b'/' | b'\\' | b':' | b'*' | b'?' | b'"' | b'<' | b'>' | b'|'
            )
    }) {
        return reject("contains a path/control character");
    }
    let upper = id.to_ascii_uppercase();
    if WINDOWS_RESERVED.contains(&upper.as_str()) || RESERVED.contains(&id) {
        return reject("reserved name");
    }
    Ok(())
}

/// Paths of the session store layout.
#[derive(Debug, Clone)]
pub(crate) struct Layout {
    /// The session root directory.
    pub root: PathBuf,
}

impl Layout {
    pub(crate) fn new(root: &Path) -> Self {
        Self {
            root: root.to_owned(),
        }
    }

    /// `{root}/store.db`
    pub(crate) fn db_path(&self) -> PathBuf {
        self.root.join("store.db")
    }

    /// `{root}/{id}`
    pub(crate) fn session_dir(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }

    /// `{root}/archive`
    pub(crate) fn archive_dir(&self) -> PathBuf {
        self.root.join("archive")
    }

    /// Materializes the full per-session contract, idempotently:
    /// `{root}/{id}/checkpoints/`, `files/`, `plan.md`, `events.jsonl`.
    pub(crate) fn ensure_session(&self, id: &str) -> Result<PathBuf, SessionError> {
        let dir = self.session_dir(id);
        fs::create_dir_all(dir.join("checkpoints")).map_err(SessionError::io)?;
        fs::create_dir_all(dir.join("files")).map_err(SessionError::io)?;
        touch(&dir.join("plan.md")).map_err(SessionError::io)?;
        touch(&dir.join("events.jsonl")).map_err(SessionError::io)?;
        Ok(dir)
    }

    /// Creates the archive directory on first use.
    pub(crate) fn ensure_archive(&self) -> Result<PathBuf, SessionError> {
        let dir = self.archive_dir();
        fs::create_dir_all(&dir).map_err(SessionError::io)?;
        Ok(dir)
    }

    /// Removes a session's directory tree (prune/GC path only).
    pub(crate) fn remove_session(&self, id: &str) -> Result<(), SessionError> {
        let dir = self.session_dir(id);
        match fs::remove_dir_all(&dir) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(SessionError::io(error)),
        }
    }
}

fn touch(path: &Path) -> io::Result<()> {
    match fs::OpenOptions::new().create(true).append(true).open(path) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_breakers_accepts_unicode() {
        for bad in [
            "", "a/b", "a\\b", "..", ".", "a:b", "store.db", "archive", "CON", "com1", " lead",
            "trail ", "dot.", "nul\x00",
        ] {
            assert!(validate_id(bad).is_err(), "{bad:?} must be rejected");
        }
        assert!(validate_id("会话-42-λ").is_ok());
        assert!(validate_id("s-1_2.3").is_ok());
    }

    #[test]
    fn layout_materializes_the_contract() {
        let root = tempfile::tempdir().unwrap();
        let layout = Layout::new(root.path());
        let dir = layout.ensure_session("abc").unwrap();
        assert!(dir.join("checkpoints").is_dir());
        assert!(dir.join("files").is_dir());
        assert!(dir.join("plan.md").is_file());
        assert!(dir.join("events.jsonl").is_file());
        layout.ensure_session("abc").unwrap();
    }
}
