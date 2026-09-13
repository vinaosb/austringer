//! Typed folder-trust set for project rules.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// A directory's rules load only if the directory itself or an ancestor
/// is trusted AND no ancestor-or-self is carved out as untrusted
/// (explicit deny wins over allow — the same direction as the team floor).
#[derive(Debug, Clone, Default)]
pub struct TrustConfig {
    trusted: BTreeSet<PathBuf>,
    untrusted: BTreeSet<PathBuf>,
}

impl TrustConfig {
    /// Trusts the given directories and their subtrees.
    pub fn trusted_from(dirs: impl IntoIterator<Item = PathBuf>) -> Self {
        let mut cfg = Self::default();
        for dir in dirs {
            if let Ok(canon) = dir.canonicalize() {
                cfg.trusted.insert(canon);
            }
        }
        cfg
    }

    /// Carves a directory out of the trusted set (deny wins over allow).
    pub fn untrust(&mut self, dir: &Path) {
        if let Ok(canon) = dir.canonicalize() {
            self.untrusted.insert(canon);
        }
    }

    pub(crate) fn is_trusted(&self, dir: &Path) -> bool {
        let canon = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        let mut denied = false;
        let mut allowed = false;
        let mut cur = Some(canon.clone());
        while let Some(d) = cur {
            if self.untrusted.contains(&d) {
                denied = true;
            }
            if self.trusted.contains(&d) {
                allowed = true;
            }
            cur = d.parent().map(Path::to_path_buf);
        }
        allowed && !denied
    }
}
