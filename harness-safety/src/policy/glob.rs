//! Deny-glob parsing, matching, and fail-closed resolution (plan todo 26).
//!
//! A deny-glob is only "kernel-enforced" when it can be resolved to the
//! concrete on-disk path set the backend will guard (deny ACEs, Landlock
//! rules, Seatbelt subpaths). A glob that matches nothing that exists has
//! nothing to enforce against and is therefore [`GlobError::Unresolvable`]
//! — the backend must refuse to start rather than allow-by-failure.

use std::fmt;
use std::path::{Path, PathBuf};

/// Typed failure while turning a glob string into an enforceable rule.
#[derive(Debug, PartialEq, Eq)]
pub enum GlobError {
    /// Syntactically invalid pattern (unclosed class, stray token).
    Malformed {
        /// The rejected pattern.
        pattern: String,
    },
    /// Syntactically valid, but nothing it matches exists under the root,
    /// so no kernel-enforceable target set can be produced.
    Unresolvable {
        /// The pattern that failed to resolve.
        pattern: String,
    },
}

impl GlobError {
    /// True when the failure is the fail-closed "nothing to enforce" case.
    #[must_use]
    pub fn is_unresolvable(&self) -> bool {
        matches!(self, GlobError::Unresolvable { .. })
    }
}

impl fmt::Display for GlobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GlobError::Malformed { pattern } => {
                write!(f, "malformed deny-glob {pattern:?}")
            }
            GlobError::Unresolvable { pattern } => {
                write!(
                    f,
                    "unresolvable deny-glob {pattern:?}: no existing match under root"
                )
            }
        }
    }
}

impl std::error::Error for GlobError {}

/// A compiled deny-glob: `*` (one segment), `?` (one char), `[a-z]`
/// (class), `**` (any depth). Matching is pure text — no regex engine,
/// no interpretation of untrusted input beyond these operators.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DenyGlob {
    pattern: String,
}

impl DenyGlob {
    /// Parse and validate a glob pattern.
    ///
    /// # Errors
    /// [`GlobError::Malformed`] for unclosed classes or empty patterns.
    pub fn parse(pattern: &str) -> Result<Self, GlobError> {
        if pattern.is_empty() {
            return Err(GlobError::Malformed {
                pattern: pattern.into(),
            });
        }
        let mut depth = 0usize;
        for ch in pattern.chars() {
            match ch {
                '[' => depth += 1,
                ']' if depth == 0 => {
                    return Err(GlobError::Malformed {
                        pattern: pattern.into(),
                    });
                }
                ']' => depth -= 1,
                // glob syntax is forward-slash only; a backslash is a
                // Windows path separator smuggled in, never a literal
                '\\' => {
                    return Err(GlobError::Malformed {
                        pattern: pattern.into(),
                    });
                }
                _ => {}
            }
        }
        if depth != 0 {
            return Err(GlobError::Malformed {
                pattern: pattern.into(),
            });
        }
        Ok(Self {
            pattern: pattern.into(),
        })
    }

    /// The original pattern text.
    #[must_use]
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Match this glob against a relative path or file-name text.
    #[must_use]
    pub fn matches_text(&self, text: &str) -> bool {
        matches_glob(&self.pattern, text)
    }

    /// Match against a path relative to the policy root (forward slashes).
    #[must_use]
    pub fn matches_relative(&self, rel: &Path) -> bool {
        let text = rel.to_string_lossy().replace('\\', "/");
        self.matches_text(&text)
    }

    /// Resolve to the concrete existing paths this glob guards. The set
    /// includes directories so deny rules inherit onto future children.
    ///
    /// # Errors
    /// [`GlobError::Unresolvable`] when zero existing paths match — the
    /// fail-closed refusal case.
    pub fn resolve(&self, root: &Path) -> Result<Vec<PathBuf>, GlobError> {
        let mut hits: Vec<PathBuf> = Vec::new();
        collect_matches(root, root, self, &mut hits);
        if hits.is_empty() {
            return Err(GlobError::Unresolvable {
                pattern: self.pattern.clone(),
            });
        }
        hits.sort();
        Ok(hits)
    }
}

fn collect_matches(root: &Path, dir: &Path, glob: &DenyGlob, hits: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        if glob.matches_relative(rel) {
            hits.push(path.clone());
        }
        if path.is_dir() {
            // `dir/**` semantics: a matched directory guards the subtree,
            // but we still walk it so nested explicit matches are listed.
            if glob.matches_relative(&rel.join("__tail__")) {
                hits.push(path.clone());
            }
            collect_matches(root, &path, glob, hits);
        }
    }
}

fn matches_glob(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    match_here(&p, 0, &t, 0)
}

fn match_here(p: &[char], mut pi: usize, t: &[char], mut ti: usize) -> bool {
    while pi < p.len() {
        match p[pi] {
            '*' => {
                let glob_all = p.get(pi + 1) == Some(&'*');
                if glob_all {
                    // `**` swallows any remainder including separators
                    let rest = &p[pi + 2..];
                    let rest = if rest.first() == Some(&'/') {
                        &rest[1..]
                    } else {
                        rest
                    };
                    for skip in ti..=t.len() {
                        if match_here(rest, 0, t, skip) {
                            return true;
                        }
                    }
                    return false;
                }
                let rest = &p[pi + 1..];
                for skip in ti..=t.len() {
                    if match_here(rest, 0, t, skip) {
                        return true;
                    }
                }
                return false;
            }
            '?' => {
                if ti >= t.len() {
                    return false;
                }
                pi += 1;
                ti += 1;
            }
            '[' => {
                if ti >= t.len() {
                    return false;
                }
                let (matched, next) = match_class(p, pi, t[ti]);
                if !matched {
                    return false;
                }
                pi = next;
                ti += 1;
            }
            ch => {
                if ti >= t.len() || t[ti] != ch {
                    return false;
                }
                pi += 1;
                ti += 1;
            }
        }
    }
    ti == t.len()
}

/// Returns (matched, index after `]`).
fn match_class(p: &[char], open: usize, candidate: char) -> (bool, usize) {
    let mut i = open + 1;
    let negate = p.get(i) == Some(&'!');
    if negate {
        i += 1;
    }
    let mut matched = false;
    while i < p.len() && p[i] != ']' {
        if let (Some(&'-'), Some(&end)) = (p.get(i + 1), p.get(i + 2)) {
            if end != ']' && candidate >= p[i] && candidate <= end {
                matched = true;
            }
            i += 3;
            continue;
        }
        if p[i] == candidate {
            matched = true;
        }
        i += 1;
    }
    (matched != negate, (i + 1).min(p.len()))
}
