//! Hybrid memory retrieval: FTS5 BM25 + vector similarity, ~0.7/0.3
//! blend, temporal decay, MMR re-rank, min-score gate, first-turn /
//! post-compaction triggers.
//!
//! With no [`VectorIndex`][crate::vector::VectorIndex] attached (the
//! FTS5-only default) every vector component is 0.0 and MMR degenerates
//! to blended order — the blend arithmetic is unchanged, so attaching a
//! vector index later upgrades ranking without touching callers.

use std::collections::BTreeMap;

use crate::error::SessionError;
use crate::memory::note_by_id;
use crate::store::SessionStore;

/// What triggered a recall — selects injection depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecallTrigger {
    /// First turn of a session: small, high-precision injection.
    FirstTurn,
    /// Post-compaction re-search: deeper re-priming after history loss.
    PostCompaction,
}

/// One Tier-1 injection hit. Every score component is exposed so the
/// blend is auditable end to end.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryHit {
    /// The note.
    pub note: crate::memory::MemoryNote,
    /// Normalized lexical score in `[0, 1]` (from `-bm25`).
    pub lexical: f64,
    /// Cosine similarity in `[0, 1]` (0 when no vector half).
    pub vector: f64,
    /// `w_vec * vector + w_lex * lexical`.
    pub blended: f64,
    /// `blended * exp(-decay_per_day * age_days)`.
    pub decayed: f64,
    /// Post-MMR final score (equals `decayed` when vectors are absent).
    pub final_score: f64,
}

/// Retrieval tuning.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RecallConfig {
    /// Weight of the vector component (plan default 0.7).
    pub vector_weight: f64,
    /// Weight of the lexical component (plan default 0.3).
    pub lexical_weight: f64,
    /// Exponential decay per day of note age.
    pub decay_per_day: f64,
    /// MMR relevance-vs-diversity tradeoff (1 = pure relevance).
    pub mmr_lambda: f64,
    /// Hits below this final score are gated out.
    pub min_score: f64,
    /// Injection depth for first turns.
    pub first_turn_limit: usize,
    /// Injection depth for post-compaction re-searches.
    pub post_compaction_limit: usize,
}

impl Default for RecallConfig {
    fn default() -> Self {
        Self {
            vector_weight: 0.7,
            lexical_weight: 0.3,
            decay_per_day: 0.05,
            mmr_lambda: 0.7,
            min_score: 0.05,
            first_turn_limit: 8,
            post_compaction_limit: 16,
        }
    }
}

/// A `/recall` query.
#[derive(Debug, Clone, Copy)]
pub struct RecallQuery<'a> {
    /// Query text (FTS5 MATCH syntax is allowed).
    pub text: &'a str,
    /// Query embedding for the vector half, when available.
    pub vector: Option<&'a [f32]>,
    /// Why the recall is running.
    pub trigger: RecallTrigger,
}

struct Candidate {
    note_id: i64,
    lexical_raw: Option<f64>,
    vector_raw: Option<f64>,
}

impl SessionStore {
    /// Runs hybrid retrieval and returns gated, re-ranked hits — the
    /// Tier-1 injection surface (`Vec<MemoryHit>`).
    ///
    /// # Errors
    /// Typed [`SessionError::BadQuery`] for unparseable FTS5 syntax;
    /// storage failures.
    pub fn recall(&self, query: &RecallQuery<'_>) -> Result<Vec<MemoryHit>, SessionError> {
        let config = self.recall_config();
        let limit = match query.trigger {
            RecallTrigger::FirstTurn => config.first_turn_limit,
            RecallTrigger::PostCompaction => config.post_compaction_limit,
        };
        if limit == 0 {
            return Ok(Vec::new());
        }
        let conn = self.lock_conn()?;
        let mut candidates: BTreeMap<i64, Candidate> = BTreeMap::new();
        for (note_id, raw) in lexical_candidates(&conn, query.text, config.pool(limit))? {
            candidates.insert(
                note_id,
                Candidate {
                    note_id,
                    lexical_raw: Some(raw),
                    vector_raw: None,
                },
            );
        }
        drop(conn);
        let now = self.now();
        let vector_side = query
            .vector
            .and_then(|vector| self.vector_index().map(|index| (vector, index)));
        if let Some((vector, index)) = &vector_side {
            for (note_id, raw) in index.top_k(vector, config.pool(limit))? {
                candidates
                    .entry(note_id)
                    .and_modify(|entry| entry.vector_raw = Some(raw as f64))
                    .or_insert(Candidate {
                        note_id,
                        lexical_raw: None,
                        vector_raw: Some(raw as f64),
                    });
            }
        }
        let lex_bounds = candidates
            .values()
            .filter_map(|c| c.lexical_raw)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), value| {
                (min.min(value), max.max(value))
            });
        let conn = self.lock_conn()?;
        let scored: Vec<MemoryHit> = candidates
            .values()
            .filter_map(|candidate| {
                let note = note_by_id(&conn, candidate.note_id).ok()??;
                let lexical = normalize(candidate.lexical_raw, lex_bounds.0, lex_bounds.1);
                let vector = candidate.vector_raw.unwrap_or(0.0).clamp(0.0, 1.0);
                let blended = config.vector_weight * vector + config.lexical_weight * lexical;
                let age_days = age_days(now, note.updated_at);
                let decayed = blended * (-config.decay_per_day * age_days).exp();
                Some(MemoryHit {
                    note,
                    lexical,
                    vector,
                    blended,
                    decayed,
                    final_score: decayed,
                })
            })
            .collect();
        drop(conn);
        let mut ranked = mmr_re_rank(scored, &vector_side, config.mmr_lambda);
        ranked.retain(|hit| hit.final_score >= config.min_score);
        ranked.truncate(limit);
        Ok(ranked)
    }
}

impl RecallConfig {
    fn pool(self, limit: usize) -> usize {
        limit.saturating_mul(4).max(8)
    }
}

fn age_days(now: u64, updated_at: u64) -> f64 {
    now.saturating_sub(updated_at) as f64 / 86_400.0
}

fn normalize(raw: Option<f64>, min: f64, max: f64) -> f64 {
    match raw {
        Some(value) if max > min => ((value - min) / (max - min)).clamp(0.0, 1.0),
        Some(_) => 1.0,
        None => 0.0,
    }
}

fn lexical_candidates(
    conn: &rusqlite::Connection,
    text: &str,
    pool: usize,
) -> Result<Vec<(i64, f64)>, SessionError> {
    let map_error = |error: rusqlite::Error| {
        let detail = error.to_string();
        if detail.contains("fts5") {
            SessionError::BadQuery {
                query: text.to_owned(),
                detail,
            }
        } else {
            SessionError::sqlite(error)
        }
    };
    let mut stmt = conn
        .prepare(
            "SELECT note_id, -bm25(memory_fts) FROM memory_fts
             WHERE memory_fts MATCH ?1 ORDER BY rank LIMIT ?2",
        )
        .map_err(map_error)?;
    let rows = stmt
        .query_map(rusqlite::params![text, pool as i64], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?))
        })
        .map_err(map_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_error)?;
    Ok(rows)
}

fn mmr_re_rank(
    mut hits: Vec<MemoryHit>,
    vector_side: &Option<(&[f32], std::sync::Arc<dyn crate::vector::VectorIndex>)>,
    lambda: f64,
) -> Vec<MemoryHit> {
    let Some((_, index)) = vector_side else {
        hits.sort_by(|a, b| b.final_score.total_cmp(&a.final_score));
        return hits;
    };
    let mut selected: Vec<MemoryHit> = Vec::with_capacity(hits.len());
    while !hits.is_empty() {
        let mut best_pos = 0;
        let mut best_score = f64::NEG_INFINITY;
        for (pos, hit) in hits.iter().enumerate() {
            let redundancy = selected
                .iter()
                .filter_map(|chosen| index.similarity(chosen.note.id, hit.note.id).ok().flatten())
                .fold(0.0f64, |acc, item: f32| acc.max(f64::from(item)));
            let score = lambda * hit.decayed - (1.0 - lambda) * redundancy;
            if score > best_score {
                best_score = score;
                best_pos = pos;
            }
        }
        let mut hit = hits.swap_remove(best_pos);
        hit.final_score = best_score.max(0.0);
        selected.push(hit);
    }
    selected
}
