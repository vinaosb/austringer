//! Hashline anchors (plan todo 16, clause 3): a content-hash anchor over
//! the target line plus [`HASH_NEIGHBORS`] neighbors on each side.
//!
//! Verification contract — never corrupt:
//! - exact line + target + neighbor hashes match → [`AnchorStatus::Matched`];
//! - hash at the recorded line fails → bounded re-anchor scan within
//!   [`REANCHOR_RADIUS`] lines; a unique full-signature match becomes
//!   [`AnchorStatus::ReAnchored`] (drift tolerated, position corrected);
//! - zero candidates, or several equally-plausible candidates, →
//!   [`AnchorStatus::Drifted`] carrying the candidate list — a typed
//!   rejection, never a first-match guess.
//!
//! Hashes cover **logical** line content (terminator-free), so wholesale
//! CRLF↔LF re-encoding of a file invalidates nothing.

use crate::error::ToolsError;
use crate::lines::LineTable;

/// Neighbor lines hashed on each side of the target.
pub const HASH_NEIGHBORS: usize = 2;

/// How far from the recorded line the re-anchor scan searches.
pub const REANCHOR_RADIUS: usize = 50;

/// Truncated blake3 of one logical line.
pub type LineHash = [u8; 8];

/// Hash one logical line's content.
pub fn line_hash(content: &str) -> LineHash {
    let digest = blake3::hash(content.as_bytes());
    let mut out = [0u8; 8];
    out.copy_from_slice(&digest.as_bytes()[..8]);
    out
}

fn hash_at(table: &LineTable, line: isize) -> Option<LineHash> {
    if line < 1 {
        return None;
    }
    table.content_opt(line as usize).map(line_hash)
}

/// A content-hash anchor for one target line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hashline {
    /// Recorded 1-based target line.
    pub line: usize,
    /// Hashes of the [`HASH_NEIGHBORS`] lines above the target (nearest
    /// first, i.e. `before[0]` is the line directly above).
    pub before: Vec<LineHash>,
    /// Hash of the target line itself.
    pub target: LineHash,
    /// Hashes of the [`HASH_NEIGHBORS`] lines below the target (nearest
    /// first).
    pub after: Vec<LineHash>,
}

/// Outcome of verifying an anchor against a (possibly changed) table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnchorStatus {
    /// The anchor matches exactly at its recorded line.
    Matched(usize),
    /// Drift detected; a unique full-signature match exists `to` lines
    /// into the scanned radius.
    ReAnchored {
        /// Recorded line.
        from: usize,
        /// Corrected 1-based line.
        to: usize,
    },
    /// Rejected: no candidate, or several equally-plausible candidates
    /// (ambiguous — never pick the first).
    Drifted {
        /// Candidate lines that matched the full signature (may be empty).
        candidates: Vec<usize>,
    },
}

impl Hashline {
    /// Compute the anchor for `line` of `table`.
    /// Errors: [`ToolsError::LineOutOfRange`].
    pub fn compute(table: &LineTable, line: usize) -> Result<Self, ToolsError> {
        let total = table.line_count();
        if line == 0 || line > total {
            return Err(ToolsError::LineOutOfRange { line, total });
        }
        let mut before = Vec::with_capacity(HASH_NEIGHBORS);
        for offset in 1..=HASH_NEIGHBORS {
            if let Some(hash) = hash_at(table, line as isize - offset as isize) {
                before.push(hash);
            }
        }
        let mut after = Vec::with_capacity(HASH_NEIGHBORS);
        for offset in 1..=HASH_NEIGHBORS {
            if let Some(hash) = hash_at(table, line as isize + offset as isize) {
                after.push(hash);
            }
        }
        Ok(Hashline {
            line,
            before,
            target: hash_at(table, line as isize).unwrap_or([0; 8]),
            after,
        })
    }

    /// Verify the anchor against `table`, applying the contract above.
    pub fn verify(&self, table: &LineTable) -> AnchorStatus {
        if self.signature_matches(table, self.line) {
            return AnchorStatus::Matched(self.line);
        }
        let total = table.line_count();
        let low = self.line.saturating_sub(REANCHOR_RADIUS).max(1);
        let high = (self.line + REANCHOR_RADIUS).min(total);
        let candidates: Vec<usize> = (low..=high)
            .filter(|&line| line != self.line && self.signature_matches(table, line))
            .collect();
        if candidates.len() == 1 {
            AnchorStatus::ReAnchored {
                from: self.line,
                to: candidates[0],
            }
        } else {
            AnchorStatus::Drifted { candidates }
        }
    }

    /// Full-signature check at `line`: target hash plus every recorded
    /// neighbor hash (recorded neighbors only — anchors at file edges
    /// carry fewer neighbors by construction).
    fn signature_matches(&self, table: &LineTable, line: usize) -> bool {
        if hash_at(table, line as isize) != Some(self.target) {
            return false;
        }
        for (offset, expected) in self.before.iter().enumerate() {
            let probe = line as isize - (offset as isize + 1);
            if hash_at(table, probe) != Some(*expected) {
                return false;
            }
        }
        for (offset, expected) in self.after.iter().enumerate() {
            let probe = line as isize + (offset as isize + 1);
            if hash_at(table, probe) != Some(*expected) {
                return false;
            }
        }
        true
    }
}
