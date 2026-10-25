//! Worktree/shadow GC with reclaimed-refs grace (plan todo 21, the Grok
//! 412 GB worktree-sprawl lesson): retired shadows live one GC cycle in
//! the reclaimed area (gix-backed ones pin their base commit under
//! `refs/austringer/reclaimed/<id>/<commit>`), and orphaned active shadows —
//! owners that died without retiring — are reaped by age.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::VfsError;
use crate::manifest::dir_size;
use crate::paths;
use crate::store::ShadowStore;
use crate::worktree;

/// Conservative defaults: a full day of grace before anything is deleted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GcPolicy {
    pub min_age: Duration,
}

impl Default for GcPolicy {
    fn default() -> Self {
        Self {
            min_age: Duration::from_secs(24 * 60 * 60),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GcReport {
    pub deleted: usize,
    pub retained: usize,
    pub bytes_reclaimed: u64,
}

impl ShadowStore {
    /// Sweep once. Entries younger than `policy.min_age` are retained;
    /// expired entries are deleted (reclaimed refs included) and their
    /// bytes accounted. Per-entry failures are logged and skipped so one
    /// locked file cannot wedge the whole sweep.
    pub fn sweep(&self, now: SystemTime, policy: &GcPolicy) -> Result<GcReport, VfsError> {
        let mut report = GcReport::default();
        self.sweep_reclaimed(now, policy, &mut report);
        self.sweep_orphans(now, policy, &mut report);
        Ok(report)
    }

    fn sweep_reclaimed(&self, now: SystemTime, policy: &GcPolicy, report: &mut GcReport) {
        let entries = match paths::read_dir_sorted(&self.root.join("reclaimed")) {
            Ok(entries) => entries,
            Err(_) => return,
        };
        for entry in entries {
            let retired_at = read_marker(&entry, ".austringer-retired-at")
                .and_then(|text| text.trim().parse::<u64>().ok());
            let Some(retired_at) = retired_at else {
                report.retained += 1;
                continue;
            };
            if !expired(now, retired_at, policy.min_age) {
                report.retained += 1;
                continue;
            }
            report.bytes_reclaimed += dir_size(&entry);
            delete_reclaimed_ref_marker(&entry);
            match paths::remove_dir_all(&entry) {
                Ok(()) => report.deleted += 1,
                Err(error) => {
                    tracing::warn!(entry = %entry.display(), %error, "reclaimed sweep: delete failed");
                    report.retained += 1;
                }
            }
        }
    }

    fn sweep_orphans(&self, now: SystemTime, policy: &GcPolicy, report: &mut GcReport) {
        let entries = match paths::read_dir_sorted(&self.root.join("shadows")) {
            Ok(entries) => entries,
            Err(_) => return,
        };
        for entry in entries {
            let Some(name) = entry.file_name().and_then(|n| n.to_str()) else {
                report.retained += 1;
                continue;
            };
            let created_secs = name.split('-').next().unwrap_or("").parse::<u64>().ok();
            let Some(created_secs) = created_secs else {
                report.retained += 1;
                continue;
            };
            if !expired(now, created_secs, policy.min_age) {
                report.retained += 1;
                continue;
            }
            report.bytes_reclaimed += dir_size(&entry);
            match paths::remove_dir_all(&entry) {
                Ok(()) => report.deleted += 1,
                Err(error) => {
                    tracing::warn!(entry = %entry.display(), %error, "orphan sweep: delete failed");
                    report.retained += 1;
                }
            }
        }
    }
}

fn expired(now: SystemTime, then_secs: u64, min_age: Duration) -> bool {
    let Some(then) = UNIX_EPOCH.checked_add(Duration::from_secs(then_secs)) else {
        return false;
    };
    now.duration_since(then)
        .map(|age| age >= min_age)
        .unwrap_or(false)
}

fn read_marker(dir: &Path, marker: &str) -> Option<String> {
    paths::read_file(&dir.join(marker))
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

/// Delete the git reclaimed-ref recorded in a retired gix shadow's
/// markers; directories without markers (other backings) are a no-op.
fn delete_reclaimed_ref_marker(dir: &Path) {
    let commit = read_marker(dir, ".austringer-commit");
    let repo = read_marker(dir, ".austringer-repo");
    if let (Some(commit), Some(repo)) = (commit, repo) {
        let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if let Err(error) = worktree::delete_reclaimed_ref(Path::new(&repo), name, &commit) {
            tracing::warn!(%commit, %error, "reclaimed ref delete failed");
        }
    }
}
