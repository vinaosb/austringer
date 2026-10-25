//! Omnibar completion source over the working tree: `@file:` via the
//! gitignore-respecting bounded walker; `@symbol:` and `@git:` stay
//! honestly empty until the LSP bridge (todo 38) and git plumbing
//! supply real candidates.

use std::path::Path;

use crate::omnibar::CompletionSource;

/// Walk cap per completion query: the omnibar is interactive, not a
/// search engine.
const MAX_WALKED: usize = 2_000;

/// Bounded file-walking completion source rooted at one directory.
pub struct FileCompletions {
    root: std::path::PathBuf,
}

impl FileCompletions {
    /// Completions relative to `root` (paths come back
    /// forward-slashed for the `@file:` grammar).
    #[must_use]
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_owned(),
        }
    }
}

impl CompletionSource for FileCompletions {
    fn files(&self, query: &str) -> Vec<String> {
        let mut hits: Vec<String> = Vec::new();
        let walker = ignore::WalkBuilder::new(&self.root)
            .hidden(true)
            .git_ignore(true)
            .require_git(false)
            .build();
        for entry in walker.flatten().take(MAX_WALKED) {
            let Some(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_file() {
                continue;
            }
            let Ok(relative) = entry.path().strip_prefix(&self.root) else {
                continue;
            };
            let path = relative.to_string_lossy().replace('\\', "/");
            if path.contains(query) {
                hits.push(path);
            }
        }
        hits.sort();
        hits.truncate(crate::omnibar::COMPLETE_LIMIT);
        hits
    }

    fn symbols(&self, _query: &str) -> Vec<String> {
        Vec::new()
    }

    fn git_refs(&self, _query: &str) -> Vec<String> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_match_by_substring_and_are_bounded() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("alpha.rs"), b"").expect("write");
        std::fs::create_dir(dir.path().join("nested")).expect("dir");
        std::fs::write(dir.path().join("nested").join("beta.rs"), b"").expect("write");
        let source = FileCompletions::new(dir.path());
        assert_eq!(
            source.files("alpha"),
            vec!["alpha.rs".to_owned()],
            "substring match, forward slashes"
        );
        assert_eq!(source.files("beta"), vec!["nested/beta.rs".to_owned()]);
        assert!(source.files("nothing-matches").is_empty());
        assert!(source.symbols("x").is_empty());
        assert!(source.git_refs("x").is_empty());
    }

    #[test]
    fn gitignored_files_are_not_offered() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join(".gitignore"), b"secret/\n").expect("gitignore");
        std::fs::write(dir.path().join("visible.rs"), b"").expect("write");
        std::fs::create_dir_all(dir.path().join("secret")).expect("dir");
        std::fs::write(dir.path().join("secret").join("key.rs"), b"").expect("write");
        let source = FileCompletions::new(dir.path());
        assert!(source.files("key").is_empty());
        assert_eq!(source.files("visible").len(), 1);
    }
}
