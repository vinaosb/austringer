//! Hunk-review view types (todo 19): one surfaced diff file with
//! per-hunk decision state and diffstat parsing. Pure view state —
//! the wire carries no hunk decisions yet, so accept/reject is
//! client-side until a protocol surface exists (todo-19 issues
//! entry); unparseable hunks render a placeholder, never parse.

/// One hunk's review outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HunkDecision {
    /// Not yet reviewed.
    Pending,
    /// Reviewed and accepted.
    Accepted,
    /// Reviewed and rejected.
    Rejected,
}

/// One rendered hunk plus its review decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    /// Hunk text as the daemon rendered it (unified-diff lines).
    pub text: String,
    /// Review decision.
    pub decision: HunkDecision,
}

/// A surfaced semantic diff for one path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffEntry {
    /// Changed path.
    pub path: String,
    /// Rendered hunks, arrival order.
    pub hunks: Vec<DiffHunk>,
}

/// Placeholder for hunks without a unified-diff header.
pub const UNPARSEABLE_HUNK: &str = "(unparseable hunk)";

/// True when a hunk carries a `@@` header line (parseable shape).
#[must_use]
pub fn hunk_is_parseable(hunk: &str) -> bool {
    hunk.lines().any(|line| line.starts_with("@@"))
}

impl DiffEntry {
    /// Collect a daemon diff; every hunk starts pending.
    #[must_use]
    pub fn new(path: &str, hunks: Vec<String>) -> Self {
        Self {
            path: path.to_owned(),
            hunks: hunks
                .into_iter()
                .map(|text| DiffHunk {
                    text,
                    decision: HunkDecision::Pending,
                })
                .collect(),
        }
    }

    /// Diffstats `(added, removed)` over parseable hunks. File
    /// headers (`+++`/`---`) and `@@` markers never count.
    #[must_use]
    pub fn stats(&self) -> (u32, u32) {
        let mut added = 0;
        let mut removed = 0;
        for hunk in &self.hunks {
            if !hunk_is_parseable(&hunk.text) {
                continue;
            }
            for line in hunk.text.lines() {
                if line.starts_with('+') && !line.starts_with("+++") {
                    added += 1;
                } else if line.starts_with('-') && !line.starts_with("---") {
                    removed += 1;
                }
            }
        }
        (added, removed)
    }

    /// Decide the first pending hunk; returns its index when one
    /// existed.
    pub fn decide_next_pending(&mut self, decision: HunkDecision) -> Option<usize> {
        let index = self
            .hunks
            .iter()
            .position(|hunk| hunk.decision == HunkDecision::Pending)?;
        self.hunks[index].decision = decision;
        Some(index)
    }

    /// Hunks still awaiting review.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.hunks
            .iter()
            .filter(|hunk| hunk.decision == HunkDecision::Pending)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HUNK: &str = "@@ -1,2 +1,3 @@\n context\n-removed line\n+added line\n+second add";

    #[test]
    fn stats_count_adds_and_removes_skipping_headers() {
        let entry = DiffEntry::new(
            "src/lib.rs",
            vec![
                HUNK.to_owned(),
                "@@ -10 +10 @@\n-sole removal".to_owned(),
                "--- a/file\n+++ b/file\n@@ -3 +3 @@\n-zombie".to_owned(),
            ],
        );
        assert_eq!(entry.stats(), (2, 3));
    }

    #[test]
    fn unparseable_hunks_never_count_and_are_detected() {
        assert!(!hunk_is_parseable("not a hunk at all"));
        assert!(hunk_is_parseable(HUNK));
        let entry = DiffEntry::new("bad.rs", vec!["garbage".to_owned()]);
        assert_eq!(entry.stats(), (0, 0));
        assert_eq!(entry.pending_count(), 1);
    }

    #[test]
    fn decide_next_pending_walks_hunks_in_order() {
        let mut entry = DiffEntry::new("p", vec!["@@ -1 +1 @".into(), "@@ -2 +2 @".into()]);
        assert_eq!(entry.decide_next_pending(HunkDecision::Accepted), Some(0));
        assert_eq!(entry.decide_next_pending(HunkDecision::Rejected), Some(1));
        assert_eq!(entry.decide_next_pending(HunkDecision::Accepted), None);
        assert_eq!(entry.pending_count(), 0);
    }
}
