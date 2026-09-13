//! Vector dimension of memory retrieval behind a trait.
//!
//! Plan amendment calls for sqlite-vec (`vec0` virtual tables). The
//! published `sqlite-vec` crate cannot be vendored on this machine
//! (task-24 evidence records the probes: 0.1.10-alpha.4 ships a
//! `sqlite-vec.c` that `#include`s the unpackaged
//! `sqlite-vec-diskann.c`; 0.1.6-alpha.2 exports a stub
//! `sqlite3_vec_init()` with no extension entry). Per the task's
//! fallback clause the vector dimension is this trait with an
//! FTS5-only default: retrieval degrades to lexical-only blending, and
//! a vec0-backed implementation is one `impl VectorIndex` away when a
//! clean vendor path exists (runtime `load_extension` or a fixed crate
//! release). [`InMemoryVectorIndex`] provides exact cosine search for
//! tests and for callers that supply their own embeddings.

use std::collections::BTreeMap;
use std::sync::RwLock;

/// Dense-vector side of hybrid memory retrieval. Implementations must
/// be externally synchronized (called behind the store's lock) or
/// internally locked like the in-memory one.
pub trait VectorIndex: Send + Sync {
    /// Inserts or replaces the vector for `note_id`.
    ///
    /// # Errors
    /// Implementation-defined typed failure.
    fn upsert(&self, note_id: i64, vector: &[f32]) -> Result<(), crate::error::SessionError>;

    /// Removes `note_id`, if present.
    ///
    /// # Errors
    /// Implementation-defined typed failure.
    fn remove(&self, note_id: i64) -> Result<(), crate::error::SessionError>;

    /// Top-`k` notes by similarity to `query` (cosine, higher = more
    /// similar), best first.
    ///
    /// # Errors
    /// Implementation-defined typed failure.
    fn top_k(&self, query: &[f32], k: usize)
    -> Result<Vec<(i64, f32)>, crate::error::SessionError>;

    /// Cosine similarity between two indexed notes, when both exist.
    ///
    /// # Errors
    /// Implementation-defined typed failure.
    fn similarity(&self, a: i64, b: i64) -> Result<Option<f32>, crate::error::SessionError>;
}

/// The default: no vector dimension. All vector scores are zero and
/// retrieval degrades to lexical-only ranking (blend weights still
/// applied, so the lexical component dominates by construction).
#[derive(Debug, Default)]
pub struct FtsOnlyVectors;

impl VectorIndex for FtsOnlyVectors {
    fn upsert(&self, _: i64, _: &[f32]) -> Result<(), crate::error::SessionError> {
        Ok(())
    }

    fn remove(&self, _: i64) -> Result<(), crate::error::SessionError> {
        Ok(())
    }

    fn top_k(&self, _: &[f32], _: usize) -> Result<Vec<(i64, f32)>, crate::error::SessionError> {
        Ok(Vec::new())
    }

    fn similarity(&self, _: i64, _: i64) -> Result<Option<f32>, crate::error::SessionError> {
        Ok(None)
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let (mut dot, mut na, mut nb) = (0.0f32, 0.0f32, 0.0f32);
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

/// Exact in-memory cosine index over unit-normalized vectors.
#[derive(Default)]
pub struct InMemoryVectorIndex {
    vectors: RwLock<BTreeMap<i64, Vec<f32>>>,
}

impl InMemoryVectorIndex {
    /// An empty index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl VectorIndex for InMemoryVectorIndex {
    fn upsert(&self, note_id: i64, vector: &[f32]) -> Result<(), crate::error::SessionError> {
        self.vectors
            .write()
            .map_err(|_| poisoned())?
            .insert(note_id, vector.to_vec());
        Ok(())
    }

    fn remove(&self, note_id: i64) -> Result<(), crate::error::SessionError> {
        self.vectors
            .write()
            .map_err(|_| poisoned())?
            .remove(&note_id);
        Ok(())
    }

    fn top_k(
        &self,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(i64, f32)>, crate::error::SessionError> {
        let vectors = self.vectors.read().map_err(|_| poisoned())?;
        let mut scored: Vec<(i64, f32)> = vectors
            .iter()
            .map(|(id, vector)| (*id, cosine(query, vector)))
            .filter(|(_, score)| *score > 0.0)
            .collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        scored.truncate(k);
        Ok(scored)
    }

    fn similarity(&self, a: i64, b: i64) -> Result<Option<f32>, crate::error::SessionError> {
        let vectors = self.vectors.read().map_err(|_| poisoned())?;
        Ok(match (vectors.get(&a), vectors.get(&b)) {
            (Some(va), Some(vb)) => Some(cosine(va, vb)),
            _ => None,
        })
    }
}

fn poisoned() -> crate::error::SessionError {
    crate::error::SessionError::Sqlite {
        detail: "vector index lock poisoned".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_ranks_aligned_vectors_first() {
        let index = InMemoryVectorIndex::new();
        index.upsert(1, &[1.0, 0.0]).unwrap();
        index.upsert(2, &[0.0, 1.0]).unwrap();
        let top = index.top_k(&[0.9, 0.1], 2).unwrap();
        assert_eq!(top[0].0, 1, "aligned vector must rank first: {top:?}");
        assert!(top[0].1 > top[1].1);
        assert!(index.similarity(1, 2).unwrap().is_some_and(|s| s < 0.01));
    }

    #[test]
    fn fts_only_default_is_empty() {
        let index = FtsOnlyVectors;
        assert!(index.top_k(&[1.0], 5).unwrap().is_empty());
        assert!(index.similarity(1, 2).unwrap().is_none());
    }
}
