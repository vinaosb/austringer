//! File-copy backings: CoW reflink with per-file fallback detection, and
//! the plain full copy that backs everything else.

use std::path::Path;

use crate::error::VfsError;
use crate::manifest::Manifest;
use crate::paths;

pub(crate) enum CopyOutcome {
    /// Every file was block-cloned (FICLONE / APFS clonefile / ReFS
    /// duplicate-extents): writes into the shadow are CoW by construction.
    Reflinked,
    /// At least one file could not be reflinked; the destination may hold
    /// a partial plain copy the caller must discard before re-trying with
    /// another backing.
    FellBack,
}

/// reflink every manifest entry. The first file that reports a plain-copy
/// fallback aborts the whole backing: `reflink_or_copy` silently degrades
/// per call, and an undisclosed mix would break the backing contract.
pub(crate) fn reflink_tree(
    src: &Path,
    dst: &Path,
    manifest: &Manifest,
) -> Result<CopyOutcome, VfsError> {
    let mut cow = true;
    for rel in manifest.files.keys() {
        let from = src.join(rel);
        let to = dst.join(rel);
        if let Some(parent) = to.parent() {
            paths::create_dir_all(parent).map_err(|source| VfsError::Io {
                op: "reflink mkdir",
                source,
            })?;
        }
        match reflink_copy::reflink_or_copy(paths::extended(&from), paths::extended(&to)) {
            Ok(None) => {}
            Ok(Some(_)) => {
                cow = false;
                break;
            }
            Err(source) => {
                return Err(VfsError::Io {
                    op: "reflink",
                    source,
                });
            }
        }
    }
    Ok(if cow {
        CopyOutcome::Reflinked
    } else {
        CopyOutcome::FellBack
    })
}

pub(crate) fn copy_tree(src: &Path, dst: &Path, manifest: &Manifest) -> Result<(), VfsError> {
    for rel in manifest.files.keys() {
        let from = src.join(rel);
        let to = dst.join(rel);
        if let Some(parent) = to.parent() {
            paths::create_dir_all(parent).map_err(|source| VfsError::Io {
                op: "copy mkdir",
                source,
            })?;
        }
        std::fs::copy(paths::extended(&from), paths::extended(&to)).map_err(|source| {
            VfsError::Io {
                op: "full copy",
                source,
            }
        })?;
    }
    Ok(())
}
