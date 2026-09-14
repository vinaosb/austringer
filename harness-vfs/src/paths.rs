//! Path containment validation and long-path-aware filesystem shims.
//!
//! Shadow-relative paths are validated lexically: only `Normal` components
//! are legal, which structurally rejects absolute, UNC/verbatim (`\\?\`),
//! and parent-traversal forms before any filesystem access happens. All
//! filesystem traffic in this crate flows through the `extended` form so
//! trees deeper than the classic 260-char Windows limit keep working
//! regardless of the OS long-path policy.

use std::io;
use std::path::{Component, Path, PathBuf};

use crate::error::PathRejection;

pub(crate) fn validate_relative(path: &Path) -> Result<(), PathRejection> {
    let mut any = false;
    for component in path.components() {
        match component {
            Component::Normal(_) => any = true,
            Component::Prefix(_) | Component::RootDir => return Err(PathRejection::Absolute),
            Component::ParentDir => return Err(PathRejection::ParentTraversal),
            Component::CurDir => return Err(PathRejection::NonCanonical),
        }
    }
    if any {
        Ok(())
    } else {
        Err(PathRejection::NonCanonical)
    }
}

/// Extended-length (`\\?\`) form of an absolute path; relative,
/// already-extended, and UNC paths pass through unchanged (mirrors the
/// harness-exec long-path convention). Verbatim paths reject `/`, so any
/// forward slashes are normalized to `\` in the prefixed form.
pub(crate) fn extended(path: &Path) -> PathBuf {
    // The \\?\ verbatim form is a Windows-only long-path mechanism; on
    // unix it is not a valid path and corrupts POSIX prefixes.
    if !cfg!(windows) {
        return path.to_path_buf();
    }
    let text = path.to_string_lossy();
    if !path.is_absolute()
        || text.starts_with(r"\\?\")
        || text.starts_with(r"\\.\")
        || text.starts_with(r"\\")
    {
        return path.to_path_buf();
    }
    PathBuf::from(format!(r"\\?\{}", text.replace('/', "\\")))
}

pub(crate) fn read_file(path: &Path) -> io::Result<Vec<u8>> {
    std::fs::read(extended(path))
}

pub(crate) fn write_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    std::fs::write(extended(path), bytes)
}

pub(crate) fn create_dir_all(path: &Path) -> io::Result<()> {
    std::fs::create_dir_all(extended(path))
}

pub(crate) fn remove_dir_all(path: &Path) -> io::Result<()> {
    std::fs::remove_dir_all(extended(path))
}

pub(crate) fn rename(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::rename(extended(from), extended(to))
}

/// Directory entries sorted by file name so walks are deterministic.
pub(crate) fn read_dir_sorted(path: &Path) -> io::Result<Vec<PathBuf>> {
    let mut children: Vec<PathBuf> = std::fs::read_dir(extended(path))?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<io::Result<_>>()?;
    children.sort();
    Ok(children)
}
