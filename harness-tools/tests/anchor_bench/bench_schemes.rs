//! Anchor-scheme implementations for the benchmark harness (plan todo 16,
//! clause 9): content-hash (the shipped scheme), chunk-fingerprint, and
//! checkpoint-chain, each with an honest recovery procedure and a
//! lines-read counter.

use harness_tools::LineTable;
use harness_tools::anchors::{AnchorStatus, HASH_NEIGHBORS, Hashline, REANCHOR_RADIUS};

pub trait Scheme {
    fn name(&self) -> &'static str;
    /// Anchor the target line; returns the opaque anchor + lines read.
    fn anchor(&self, table: &LineTable, target: usize) -> (Vec<u8>, usize);
    /// Try to recover the anchor in `table` (possibly drifted).
    /// Returns (new_line, lines_read); `None` = typed rejection.
    fn recover(&self, anchor: &[u8], table: &LineTable) -> Option<(usize, usize)>;
}

fn serialize(anchor: &Hashline) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&anchor.line.to_le_bytes());
    bytes.extend_from_slice(&anchor.target);
    for hash in anchor.before.iter().chain(&anchor.after) {
        bytes.extend_from_slice(hash);
    }
    bytes
}

fn deserialize(bytes: &[u8]) -> Hashline {
    let line = usize::from_le_bytes(bytes[0..8].try_into().unwrap_or([0; 8]));
    let mut target = [0u8; 8];
    target.copy_from_slice(&bytes[8..16]);
    let mut before = Vec::new();
    let mut after = Vec::new();
    for hash in bytes[16..].as_chunks::<8>().0 {
        let mut chunk = [0u8; 8];
        chunk.copy_from_slice(hash);
        if before.len() < 2 {
            before.push(chunk);
        } else {
            after.push(chunk);
        }
    }
    Hashline {
        line,
        before,
        target,
        after,
    }
}

pub struct ContentHash;

impl Scheme for ContentHash {
    fn name(&self) -> &'static str {
        "content-hash"
    }
    fn anchor(&self, table: &LineTable, target: usize) -> (Vec<u8>, usize) {
        let anchor = Hashline::compute(table, target).unwrap_or_else(|e| panic!("{e}"));
        (serialize(&anchor), 2 * HASH_NEIGHBORS + 1)
    }
    fn recover(&self, anchor_bytes: &[u8], table: &LineTable) -> Option<(usize, usize)> {
        // Read-amplification model: exact verify reads the signature
        // (2N+1 lines); on miss, the re-anchor scan is charged its
        // full ±radius sweep (conservative upper bound — the scan
        // stops at the match).
        let anchor = deserialize(anchor_bytes);
        match anchor.verify(table) {
            AnchorStatus::Matched(line) => Some((line, 2 * HASH_NEIGHBORS + 1)),
            AnchorStatus::ReAnchored { to, .. } => {
                Some((to, 2 * HASH_NEIGHBORS + 1 + 2 * REANCHOR_RADIUS + 1))
            }
            AnchorStatus::Drifted { .. } => None,
        }
    }
}

pub struct ChunkFingerprint;

impl Scheme for ChunkFingerprint {
    fn name(&self) -> &'static str {
        "chunk-fingerprint"
    }
    fn anchor(&self, table: &LineTable, target: usize) -> (Vec<u8>, usize) {
        let chunk_first = (target - 1) / 20 * 20 + 1;
        let contents: Vec<&str> = (chunk_first..chunk_first + 20)
            .filter_map(|line| table.content_opt(line))
            .collect();
        let mut bytes = harness_tools::anchors::line_hash(&contents.join("\n")).to_vec();
        bytes.extend_from_slice(&((target - chunk_first + 1) as u64).to_le_bytes());
        (bytes, 20)
    }
    fn recover(&self, anchor: &[u8], table: &LineTable) -> Option<(usize, usize)> {
        let mut fingerprint = [0u8; 8];
        fingerprint.copy_from_slice(&anchor[..8]);
        let offset = u64::from_le_bytes(anchor[8..16].try_into().unwrap_or([0; 8])) as usize;
        let total = table.line_count();
        let mut read = 0usize;
        for start in 1..=total.saturating_sub(19) {
            let contents: Vec<&str> = (start..start + 20)
                .filter_map(|line| table.content_opt(line))
                .collect();
            read += 20;
            if harness_tools::anchors::line_hash(&contents.join("\n")) == fingerprint {
                return Some((start + offset - 1, read));
            }
        }
        None
    }
}

pub struct CheckpointChain;

impl Scheme for CheckpointChain {
    fn name(&self) -> &'static str {
        "checkpoint-chain"
    }
    fn anchor(&self, table: &LineTable, target: usize) -> (Vec<u8>, usize) {
        let checkpoint = (target - 1) / 40 * 40;
        let mut bytes = checkpoint.to_le_bytes().to_vec();
        bytes.extend_from_slice(&target.to_le_bytes());
        let mut acc = [0u8; 8];
        for line in checkpoint + 1..=target {
            let mut hash =
                harness_tools::anchors::line_hash(table.content_opt(line).unwrap_or_default());
            for (index, byte) in hash.iter_mut().enumerate() {
                *byte ^= acc[index];
            }
            acc = hash;
        }
        bytes.extend_from_slice(&acc);
        (bytes, target - checkpoint)
    }
    fn recover(&self, anchor: &[u8], table: &LineTable) -> Option<(usize, usize)> {
        let checkpoint = usize::from_le_bytes(anchor[0..8].try_into().unwrap_or([0; 8]));
        let target = usize::from_le_bytes(anchor[8..16].try_into().unwrap_or([0; 8]));
        let mut expected = [0u8; 8];
        expected.copy_from_slice(&anchor[16..24]);
        let mut read = 0usize;
        // An insertion inside the segment breaks the chain; every
        // candidate shift costs a full forward re-hash of the segment.
        for shift in 0..40 {
            let mut acc = [0u8; 8];
            for line in checkpoint + 1 + shift..=target + shift {
                let Some(content) = table.content_opt(line) else {
                    break;
                };
                let mut hash = harness_tools::anchors::line_hash(content);
                for (index, byte) in hash.iter_mut().enumerate() {
                    *byte ^= acc[index];
                }
                acc = hash;
                read += 1;
            }
            if acc == expected {
                return Some((target + shift, read));
            }
        }
        None
    }
}
