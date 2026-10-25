//! Hot-reload seam: a [`SkillWatcher`] trait with a zero-dependency
//! polling implementation (content-hash snapshots — mtime granularity on
//! Windows is unreliable for fast edits). The daemon's real FS watcher
//! (the `notify` crate + debouncer) implements the SAME trait; the
//! [`DebounceConfig`] below is the typed seam it consumes, so test code
//! exercises the polling impl and production code swaps only the impl.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::SkillError;

/// A detected change to a SKILL.md file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillChange {
    /// A new SKILL.md appeared.
    Added(PathBuf),
    /// A SKILL.md was removed.
    Removed(PathBuf),
    /// A SKILL.md's content changed.
    Changed(PathBuf),
}

/// Watcher seam implemented by the polling fallback (here) and by the
/// daemon's debounced `notify` watcher (later).
pub trait SkillWatcher {
    /// Returns changes since the previous poll (first poll establishes
    /// the baseline and returns no events).
    fn poll(&mut self) -> Result<Vec<SkillChange>, SkillError>;
}

/// Tuning for the daemon's debounced watcher. Defaults are deliberately
/// conservative: 300 ms coalesces editor save-storms without feeling laggy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebounceConfig {
    /// Coalescing window in milliseconds.
    pub interval_ms: u64,
    /// Maximum events buffered per window before flushing.
    pub max_burst: u32,
}

impl Default for DebounceConfig {
    fn default() -> Self {
        Self {
            interval_ms: 300,
            max_burst: 16,
        }
    }
}

/// Zero-dependency polling watcher: snapshots the blake3 content hash of
/// every SKILL.md under the watched base and diffs on each poll.
#[derive(Debug)]
pub struct PollingWatcher {
    base: PathBuf,
    baseline: BTreeMap<PathBuf, String>,
}

impl PollingWatcher {
    /// Creates the watcher and takes the baseline snapshot.
    pub fn new(base: impl AsRef<Path>) -> Result<Self, SkillError> {
        let base = base.as_ref().to_path_buf();
        let baseline = snapshot(&base)?;
        Ok(Self { base, baseline })
    }
}

impl SkillWatcher for PollingWatcher {
    fn poll(&mut self) -> Result<Vec<SkillChange>, SkillError> {
        let current = snapshot(&self.base)?;
        let mut events = Vec::new();
        for (path, hash) in &current {
            match self.baseline.get(path) {
                Some(old) if old != hash => events.push(SkillChange::Changed(path.clone())),
                Some(_) => {}
                None => events.push(SkillChange::Added(path.clone())),
            }
        }
        for path in self.baseline.keys() {
            if !current.contains_key(path) {
                events.push(SkillChange::Removed(path.clone()));
            }
        }
        self.baseline = current;
        Ok(events)
    }
}

fn snapshot(base: &Path) -> Result<BTreeMap<PathBuf, String>, SkillError> {
    let mut out = BTreeMap::new();
    let mut stack = vec![base.to_path_buf()];
    let mut depth_stack = vec![0u32];
    while let (Some(dir), Some(depth)) = (stack.pop(), depth_stack.pop()) {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(SkillError::io("watch scan", &e)),
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if name.starts_with('.') && !is_vendor_dir(&name) {
                    continue;
                }
                if matches!(name.as_str(), "node_modules" | "target" | "__pycache__") || depth > 8 {
                    continue;
                }
                stack.push(path);
                depth_stack.push(depth + 1);
            } else if path.file_name().is_some_and(|n| n == "SKILL.md") {
                let bytes = std::fs::read(&path).map_err(|e| SkillError::io("watch read", &e))?;
                out.insert(path, blake3::hash(&bytes).to_hex().to_string());
            }
        }
    }
    Ok(out)
}

fn is_vendor_dir(name: &str) -> bool {
    matches!(
        name,
        ".austringer" | ".opencode" | ".claude" | ".agents" | ".cursor"
    )
}
