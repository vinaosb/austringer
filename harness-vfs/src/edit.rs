//! Typed shadow edits and batch resolution (the parse-don't-validate
//! phase of the shadow transaction).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::error::{FileHash, VfsError};
use crate::paths;

/// One whole-file replacement inside the shadow.
///
/// Mapping from the editing tools (plan todo 16): a
/// `harness_tools::edit::LineEdit` batch is applied in memory via
/// `harness_tools::edit::apply_edits`, which yields the post-edit
/// `harness_tools::lines::LineTable`; the caller sends this layer
/// `ShadowEdit::replace(path, blake3(pre_edit_bytes), table.as_bytes())`.
/// The expected hash is the drift contract: if anything moved between the
/// tool's read and this layer's write, the edit is rejected typed instead
/// of landing on shifted content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowEdit {
    /// Shadow-relative path; only `Normal` path components are legal.
    pub path: PathBuf,
    /// blake3 of the bytes the caller last saw at `path`; `None` asserts
    /// the file does not exist yet (creation).
    pub expected_hash: Option<FileHash>,
    /// Exact replacement bytes (CRLF and encodings pass through unmunged).
    pub new_content: Vec<u8>,
}

impl ShadowEdit {
    #[must_use]
    pub fn replace(
        path: impl Into<PathBuf>,
        expected_hash: FileHash,
        new_content: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            path: path.into(),
            expected_hash: Some(expected_hash),
            new_content: new_content.into(),
        }
    }

    #[must_use]
    pub fn create(path: impl Into<PathBuf>, new_content: impl Into<Vec<u8>>) -> Self {
        Self {
            path: path.into(),
            expected_hash: None,
            new_content: new_content.into(),
        }
    }
}

/// Validate a batch: paths legal, no duplicates inside the batch and none
/// re-editing a path already applied in this transaction (re-editing a
/// modified file would race the caller's stale expectation — compute
/// against current shadow content instead).
pub(crate) fn resolve_batch(
    already_applied: &[ShadowEdit],
    edits: &[ShadowEdit],
) -> Result<Vec<ShadowEdit>, VfsError> {
    if edits.is_empty() {
        return Err(VfsError::EmptyEdits);
    }
    let taken: HashSet<&Path> = already_applied.iter().map(|e| e.path.as_path()).collect();
    let mut seen = HashSet::with_capacity(edits.len());
    for edit in edits {
        if let Err(reason) = paths::validate_relative(&edit.path) {
            return Err(VfsError::InvalidShadowPath {
                path: edit.path.clone(),
                reason,
            });
        }
        if !seen.insert(edit.path.as_path()) || taken.contains(edit.path.as_path()) {
            return Err(VfsError::DuplicateEditPath {
                path: edit.path.clone(),
            });
        }
    }
    Ok(edits.to_vec())
}
