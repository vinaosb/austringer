//! Corrective revisions (plan todo 22): re-editing paths ALREADY applied
//! in this transaction — the fixer seam's write path. `apply` rejects
//! re-edits because the caller's expected hash anchors on pre-transaction
//! content; `revise` anchors on the CURRENT shadow bytes instead, and
//! keeps the transaction's ORIGINAL user-tree expectation so commit-time
//! drift verification is unchanged.

use crate::edit::ShadowEdit;
use crate::error::VfsError;
use crate::manifest::hash_of;
use crate::paths;
use crate::shadow::{ApplyStats, ShadowHandle};

impl ShadowHandle<'_> {
    /// Apply `revisions` inside the shadow, updating the recorded
    /// transaction instead of extending it:
    ///
    /// * A path already applied in this transaction is rewritten in the
    ///   shadow and its recorded `new_content` replaced. The revision's
    ///   `expected_hash` (when present) must match the CURRENT shadow
    ///   bytes — i.e. what the fixer actually read.
    /// * A new path is introduced with `apply`-style create/replace
    ///   semantics (hash anchored on current shadow bytes) and joins the
    ///   transaction with that expectation.
    ///
    /// The user tree remains untouched (only [`Self::commit`] writes it).
    pub fn revise(&mut self, revisions: &[ShadowEdit]) -> Result<ApplyStats, VfsError> {
        if self.retired {
            return Err(VfsError::AlreadyRetired);
        }
        if revisions.is_empty() {
            return Err(VfsError::EmptyEdits);
        }
        let mut seen = std::collections::HashSet::new();
        for edit in revisions {
            if let Err(reason) = paths::validate_relative(&edit.path) {
                return Err(VfsError::InvalidShadowPath {
                    path: edit.path.clone(),
                    reason,
                });
            }
            if !seen.insert(edit.path.as_path()) {
                return Err(VfsError::DuplicateEditPath {
                    path: edit.path.clone(),
                });
            }
        }
        let mut revised = 0;
        let mut introduced = 0;
        for edit in revisions {
            let shadow_file = self.shadow_dir.join(&edit.path);
            let existing = self.applied.iter().position(|a| a.path == edit.path);
            match existing {
                Some(index) => {
                    self.verify_anchor(&shadow_file, edit)?;
                    self.write_revision(&shadow_file, edit)?;
                    self.applied[index].new_content = edit.new_content.clone();
                    revised += 1;
                }
                None => {
                    self.verify_anchor(&shadow_file, edit)?;
                    self.write_revision(&shadow_file, edit)?;
                    self.applied.push(edit.clone());
                    introduced += 1;
                }
            }
        }
        Ok(ApplyStats {
            applied: revised,
            created: introduced,
        })
    }

    fn verify_anchor(
        &self,
        shadow_file: &std::path::Path,
        edit: &ShadowEdit,
    ) -> Result<(), VfsError> {
        let Some(expected) = edit.expected_hash else {
            if shadow_file.symlink_metadata().is_ok() {
                return Err(VfsError::HashMismatch {
                    path: edit.path.clone(),
                    expected: "<absent>".to_owned(),
                    actual: "<file exists>".to_owned(),
                });
            }
            return Ok(());
        };
        let bytes = paths::read_file(shadow_file).map_err(|source| VfsError::Io {
            op: "revise read",
            source,
        })?;
        let actual = hash_of(&bytes);
        if actual != expected {
            return Err(VfsError::HashMismatch {
                path: edit.path.clone(),
                expected: crate::manifest::hex_of(&expected),
                actual: crate::manifest::hex_of(&actual),
            });
        }
        Ok(())
    }

    fn write_revision(
        &self,
        shadow_file: &std::path::Path,
        edit: &ShadowEdit,
    ) -> Result<(), VfsError> {
        if let Some(parent) = shadow_file.parent() {
            paths::create_dir_all(parent).map_err(|source| VfsError::Io {
                op: "revise mkdir",
                source,
            })?;
        }
        paths::write_file(shadow_file, &edit.new_content).map_err(|source| VfsError::Io {
            op: "revise write",
            source,
        })
    }
}
