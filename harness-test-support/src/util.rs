//! Small hermetic test utilities.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A unique temporary directory removed (best effort) on drop.
///
/// Every test that touches files owns its own [`TestDir`]: paths key on
/// `tag + pid + counter`, so concurrent test processes (nextest runs one
/// process per test) and retries never share state.
#[derive(Debug)]
pub struct TestDir {
    path: PathBuf,
}

impl TestDir {
    /// Creates a fresh empty directory under the system temp dir.
    pub fn new(tag: &str) -> std::io::Result<Self> {
        let seq = DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("harness-ts-{tag}-{}-{seq}", std::process::id()));
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    /// The directory's path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        // Best effort; a leaked temp dir never fails a test.
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
