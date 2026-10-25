//! Open-document bookkeeping (full-text sync): versions per file plus the
//! last-synced text cache used for UTF-16 column conversion.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::lock;

struct DocState {
    version: u32,
    text: String,
}

/// Tracks which documents have been opened (didOpen) and their last-synced
/// version and full text.
pub(crate) struct DocTracker {
    docs: Mutex<HashMap<PathBuf, DocState>>,
}

impl DocTracker {
    pub(crate) fn new() -> Self {
        Self {
            docs: Mutex::new(HashMap::new()),
        }
    }

    /// Registers a didOpen at version 1; returns the version. A repeated
    /// open of an already-tracked file is a no-op returning its version.
    pub(crate) fn open(&self, file: &Path, text: String) -> u32 {
        let mut docs = lock(&self.docs);
        if let Some(state) = docs.get_mut(file) {
            return state.version;
        }
        docs.insert(file.to_path_buf(), DocState { version: 1, text });
        1
    }

    /// Records a didChange: version increments, text replaces. Opening
    /// implicitly first if the file was never tracked.
    pub(crate) fn change(&self, file: &Path, text: String) -> u32 {
        let mut docs = lock(&self.docs);
        match docs.get_mut(file) {
            Some(state) => {
                state.version = state.version.saturating_add(1);
                state.text = text;
                state.version
            }
            None => {
                docs.insert(file.to_path_buf(), DocState { version: 1, text });
                1
            }
        }
    }

    /// The last-synced full text, if the file is tracked.
    pub(crate) fn text(&self, file: &Path) -> Option<String> {
        lock(&self.docs).get(file).map(|state| state.text.clone())
    }

    /// Forgets all tracked documents (a freshly spawned server starts
    /// with an empty document set; didOpen re-fires per file).
    pub(crate) fn clear(&self) {
        lock(&self.docs).clear();
    }
}
