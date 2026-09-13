//! Long-path-aware filesystem shims (same discipline as the T21 layer:
//! all traffic goes through the `\\?\` form so >260-char trees work
//! regardless of OS policy), plus small serialization helpers.

use std::path::{Path, PathBuf};

use crate::SnapshotError;

/// Extended-length form of an absolute path; relative, already-extended,
/// and UNC paths pass through. Verbatim paths reject `/`, so forward
/// slashes are normalized in the prefixed form.
#[must_use]
pub(crate) fn extended(path: &Path) -> PathBuf {
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

pub(crate) fn read_file(path: &Path) -> Result<Vec<u8>, SnapshotError> {
    std::fs::read(extended(path)).map_err(|source| SnapshotError::io("read", source))
}

pub(crate) fn write_file(path: &Path, bytes: &[u8]) -> Result<(), SnapshotError> {
    std::fs::write(extended(path), bytes).map_err(|source| SnapshotError::io("write", source))
}

pub(crate) fn create_dir_all(path: &Path) -> Result<(), SnapshotError> {
    std::fs::create_dir_all(extended(path)).map_err(|source| SnapshotError::io("mkdir", source))
}

pub(crate) fn remove_file(path: &Path) -> Result<(), SnapshotError> {
    std::fs::remove_file(extended(path)).map_err(|source| SnapshotError::io("remove", source))
}

pub(crate) fn remove_dir_all(path: &Path) -> Result<(), SnapshotError> {
    std::fs::remove_dir_all(extended(path)).map_err(|source| SnapshotError::io("rmdir", source))
}

/// Deterministic sorted directory listing (stability across platforms).
pub(crate) fn read_dir_sorted(path: &Path) -> Result<Vec<PathBuf>, SnapshotError> {
    let mut children: Vec<PathBuf> = std::fs::read_dir(extended(path))
        .map_err(|source| SnapshotError::io("read_dir", source))?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<_, _>>()
        .map_err(|source| SnapshotError::io("dir entry", source))?;
    children.sort();
    Ok(children)
}

/// (mtime seconds, mtime nanoseconds, size) of the file at `path`.
pub(crate) fn file_stat(path: &Path) -> Result<(u64, u32, u64), SnapshotError> {
    let meta = std::fs::symlink_metadata(extended(path))
        .map_err(|source| SnapshotError::io("stat", source))?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .unwrap_or_default();
    Ok((mtime.as_secs(), mtime.subsec_nanos(), meta.len()))
}

/// Portable relative path (forward slashes) for manifest keys.
#[must_use]
pub(crate) fn slash(rel: &Path) -> String {
    rel.to_string_lossy().replace('\\', "/")
}

pub(crate) fn to_json<T: serde::Serialize>(value: &T) -> Result<String, SnapshotError> {
    serde_json::to_string(value).map_err(|source| SnapshotError::Serialize(source.to_string()))
}

pub(crate) fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}
