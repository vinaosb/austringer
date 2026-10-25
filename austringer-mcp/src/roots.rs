//! Client-side roots confinement (todo 29): before any tool call is sent
//! to a mounted server, its arguments are scanned for path-like strings;
//! every absolute path (or `..` traversal) that would land outside ALL
//! declared roots is denied with a typed [`McpClientError::RootsEscape`].
//!
//! This is an ARGUMENT gate, not a sandbox: it refuses to *send* escaping
//! paths; process-level confinement is the T26 sandbox's job. Comparison
//! is lexical (no filesystem access, no TOCTOU against the fs), with
//! Windows case-folding.

use serde_json::Value;
use std::path::{Component, Path, PathBuf, Prefix};

use crate::config::ServerEntry;
use crate::error::McpClientError;

/// The confinement guard for one mounted server.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Roots {
    roots: Vec<PathBuf>,
}

impl Roots {
    /// Builds the guard from declared roots (empty = deny every
    /// path-like argument; fail closed).
    #[must_use]
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self { roots }
    }

    /// Builds the guard from a server entry's declared roots.
    #[must_use]
    pub fn from_entry(entry: &ServerEntry) -> Self {
        Self::new(entry.roots.clone())
    }

    /// Whether an absolute path lexically sits under any root.
    #[must_use]
    pub fn contains(&self, path: &Path) -> bool {
        self.roots.iter().any(|root| is_under(path, root))
    }

    /// Scans one tool call's arguments; the first escaping path yields a
    /// typed denial. Strings are treated as paths when absolute or when
    /// they contain `..` or separator components.
    pub fn check_arguments(&self, tool: &str, arguments: &Value) -> Result<(), McpClientError> {
        match arguments {
            Value::String(text) => self.check_string(tool, text),
            Value::Array(items) => {
                for item in items {
                    self.check_arguments(tool, item)?;
                }
                Ok(())
            }
            Value::Object(map) => {
                for value in map.values() {
                    self.check_arguments(tool, value)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn check_string(&self, tool: &str, text: &str) -> Result<(), McpClientError> {
        if !looks_like_path(text) {
            return Ok(());
        }
        let candidate = Path::new(text);
        if candidate.is_absolute() {
            if self.contains(candidate) {
                Ok(())
            } else {
                Err(McpClientError::RootsEscape {
                    tool: tool.to_owned(),
                    path: text.to_owned(),
                })
            }
        } else if self
            .roots
            .iter()
            .any(|root| is_under(&root.join(candidate), root))
        {
            Ok(())
        } else {
            Err(McpClientError::RootsEscape {
                tool: tool.to_owned(),
                path: text.to_owned(),
            })
        }
    }
}

fn looks_like_path(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    if is_absolute_text(text) {
        return true;
    }
    text.contains("..") || text.contains('/') || text.contains('\\')
}

fn is_absolute_text(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    // UNC or POSIX-absolute
    if text.starts_with("\\\\") || text.starts_with('/') {
        return true;
    }
    // Drive-absolute: `C:\` or `C:/`
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

/// Lexical containment after normalizing `..` and `.` components; case
/// folded on Windows.
fn is_under(path: &Path, root: &Path) -> bool {
    let path_components = normalize(path);
    let root_components = normalize(root);
    if root_components.is_empty() || path_components.len() < root_components.len() {
        return false;
    }
    path_components
        .iter()
        .zip(root_components.iter())
        .all(|(a, b)| components_equal(a, b))
}

fn normalize(path: &Path) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(part) => {
                out.push(part.to_string_lossy().into_owned());
            }
            Component::Prefix(prefix) => {
                if let Prefix::Disk(letter) = prefix.kind() {
                    out.push(format!("{}:", (letter as char).to_ascii_uppercase()));
                }
            }
            Component::RootDir => {}
        }
    }
    out
}

#[cfg(windows)]
fn components_equal(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

#[cfg(not(windows))]
fn components_equal(a: &str, b: &str) -> bool {
    a == b
}
