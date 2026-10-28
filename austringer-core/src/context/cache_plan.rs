//! Provider cache-control plans (plan todo 11, hermes SS27).
//!
//! [`CachePlan`] carries explicit breakpoint placement (Anthropic's
//! four-marker scheme) with 5m/1h TTL spans as typed metadata, plus the
//! provider-specific relocation hook (OpenRouter part-level caching).
//! OpenAI needs no markers — the layout is stable, so its automatic
//! prefix cache rides the same bytes. Post-turn background tasks
//! (review/curator forks) deliberately reuse the exact prompt prefix so
//! they share the cache.

use serde::{Deserialize, Serialize};

use super::layout::SegmentMap;

/// The maximum number of cache breakpoints Anthropic accepts per request.
pub const MAX_CACHE_BREAKPOINTS: usize = 4;

/// TTL span of a cache breakpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheTtl {
    /// The short (default) span.
    FiveMinutes,
    /// The extended span for long-lived baselines.
    OneHour,
}

impl CacheTtl {
    /// The span in seconds.
    #[must_use]
    pub const fn as_secs(self) -> u64 {
        match self {
            Self::FiveMinutes => 300,
            Self::OneHour => 3600,
        }
    }
}

/// One explicit breakpoint: everything before `at` participates in the
/// provider cache under `ttl`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheBreakpoint {
    /// Byte offset the breakpoint caches up to.
    pub at: usize,
    /// The TTL span to request.
    pub ttl: CacheTtl,
}

/// Which provider's marker scheme the plan targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CachePolicy {
    /// Anthropic: up to [`MAX_CACHE_BREAKPOINTS`] explicit
    /// `cache_control` markers at segment boundaries.
    Anthropic,
    /// OpenAI: automatic prefix caching — no markers, the stable layout
    /// rides the implicit cache.
    OpenAI,
}

/// A typed cache-control plan the provider adapter consumes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachePlan {
    policy: CachePolicy,
    breakpoints: Vec<CacheBreakpoint>,
}

impl CachePlan {
    /// Plans breakpoints for `map` under `policy`.
    ///
    /// Anthropic placement: L0 (system + tools), L1 (rules + skills), the
    /// midpoint of history, and L2 (the full stable prefix) — sorted,
    /// deduplicated, and capped at [`MAX_CACHE_BREAKPOINTS`] by
    /// construction. OpenAI placement: none.
    #[must_use]
    pub fn plan(policy: CachePolicy, map: &SegmentMap) -> Self {
        let mut offsets = match policy {
            CachePolicy::Anthropic => {
                let mid = map.l1() + (map.l2() - map.l1()) / 2;
                vec![map.l0(), map.l1(), mid, map.l2()]
            }
            CachePolicy::OpenAI => vec![],
        };
        offsets.sort_unstable();
        offsets.dedup();
        offsets.truncate(MAX_CACHE_BREAKPOINTS);
        Self {
            policy,
            breakpoints: offsets
                .into_iter()
                .map(|at| CacheBreakpoint {
                    at,
                    ttl: CacheTtl::FiveMinutes,
                })
                .collect(),
        }
    }

    /// The planned breakpoints, sorted by offset.
    #[must_use]
    pub fn breakpoints(&self) -> &[CacheBreakpoint] {
        &self.breakpoints
    }

    /// The policy this plan was built for.
    #[must_use]
    pub const fn policy(&self) -> CachePolicy {
        self.policy
    }

    /// Re-TTLs every breakpoint (immutable update).
    #[must_use]
    pub fn with_ttl(mut self, ttl: CacheTtl) -> Self {
        for breakpoint in &mut self.breakpoints {
            breakpoint.ttl = ttl;
        }
        self
    }

    /// The provider-specific relocation hook: rebuilds the plan against
    /// adapter-supplied part boundaries (e.g. OpenRouter part-level
    /// caching) with one TTL. Boundaries are sorted and deduplicated;
    /// more than [`MAX_CACHE_BREAKPOINTS`] is a typed error, never a
    /// silent truncation.
    pub fn relocate(&self, boundaries: &[usize], ttl: CacheTtl) -> Result<Self, CachePlanError> {
        let mut offsets = boundaries.to_vec();
        offsets.sort_unstable();
        offsets.dedup();
        if offsets.len() > MAX_CACHE_BREAKPOINTS {
            return Err(CachePlanError::TooManyBreakpoints {
                count: offsets.len(),
            });
        }
        Ok(Self {
            policy: self.policy,
            breakpoints: offsets
                .into_iter()
                .map(|at| CacheBreakpoint { at, ttl })
                .collect(),
        })
    }

    /// A plan over adapter-supplied part boundaries — the entry point
    /// provider adapters use when no rendered SegmentMap exists (their
    /// boundaries are stable-surface offsets, e.g. tools / system /
    /// stable prefix). Same budget law as [`Self::relocate`].
    pub fn relocate_from(
        policy: CachePolicy,
        boundaries: &[usize],
        ttl: CacheTtl,
    ) -> Result<Self, CachePlanError> {
        Self {
            policy,
            breakpoints: Vec::new(),
        }
        .relocate(boundaries, ttl)
    }
}

/// Why a cache plan could not be relocated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CachePlanError {
    /// The relocation exceeded the provider's breakpoint budget.
    #[error("{count} breakpoints exceeds the maximum of {max}", max = MAX_CACHE_BREAKPOINTS)]
    TooManyBreakpoints {
        /// The rejected breakpoint count.
        count: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::layout::{SegmentName, SegmentSpan, Stability};

    fn sample_map() -> SegmentMap {
        let lengths = [10usize, 20, 5, 0, 8, 40, 6];
        let names = [
            SegmentName::System,
            SegmentName::ToolSchema,
            SegmentName::Rules,
            SegmentName::Skills,
            SegmentName::Repomap,
            SegmentName::History,
            SegmentName::VolatileTail,
        ];
        let mut spans = Vec::new();
        let mut cursor = 0;
        for (index, name) in names.into_iter().enumerate() {
            spans.push(SegmentSpan {
                name,
                start: cursor,
                len: lengths[index],
                stability: if name == SegmentName::VolatileTail {
                    Stability::Volatile
                } else {
                    Stability::Stable
                },
            });
            cursor += lengths[index];
        }
        SegmentMap::new(spans)
    }

    #[test]
    fn relocation_sorts_dedupes_and_rejects_overflow() {
        let map = sample_map();
        let plan = CachePlan::plan(CachePolicy::Anthropic, &map);
        let relocated = plan
            .relocate(&[map.l2(), map.l0(), map.l0()], CacheTtl::OneHour)
            .expect("relocate within budget");
        let ats: Vec<usize> = relocated.breakpoints().iter().map(|b| b.at).collect();
        assert_eq!(ats, vec![map.l0(), map.l2()]);
        assert!(
            relocated
                .breakpoints()
                .iter()
                .all(|b| b.ttl == CacheTtl::OneHour)
        );
        assert!(matches!(
            plan.relocate(&[1, 2, 3, 4, 5], CacheTtl::FiveMinutes),
            Err(CachePlanError::TooManyBreakpoints { count: 5 })
        ));
    }
}
