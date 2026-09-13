//! Lexical trigger routing over skill metadata surfaces.
//!
//! Matching contract (documented; bodies are never consulted):
//! * explicit `triggers:` phrases match by case-insensitive containment,
//!   with word-boundary checks for single-ASCII-word triggers (CJK and
//!   other scripts match by plain containment — no word boundaries exist);
//! * `USE FOR:` phrases derived from descriptions match the same way;
//! * the `/name` command always matches;
//! * any `do-not-trigger-on` / `DO NOT USE FOR` hit blocks the skill even
//!   when a positive trigger matched (negative scoping wins);
//! * a skill with a monorepo subtree scope routes only for work paths
//!   inside that subtree; ≥2 distinct name words present is a weak match.

use std::path::{Path, PathBuf};

use crate::consent::ConsentGate;
use crate::skill::{SkillMeta, SkillName};

/// Routing query: prompt text plus the path work is happening under.
#[derive(Debug, Clone, Copy)]
pub struct RouteQuery<'a> {
    /// The prompt to route.
    pub text: &'a str,
    /// Directory/file the work targets (None = no subtree context).
    pub work_path: Option<&'a Path>,
}

/// Full routing decision: matched skills, negative-scope blocks, subtree
/// filters, and consent-pending skills.
#[derive(Debug, Clone, Default)]
pub struct RoutingDecision {
    /// Skills that positively matched and are routable.
    pub matched: Vec<RouteMatch>,
    /// Skills whose positive trigger matched but a negative trigger won.
    pub blocked: Vec<BlockedMatch>,
    /// Skills filtered because the work path is outside their subtree.
    pub scope_filtered: Vec<ScopeFiltered>,
    /// Matched-but-inert skills awaiting install consent.
    pub pending_consent: Vec<SkillName>,
}

/// A positive route hit.
#[derive(Debug, Clone)]
pub struct RouteMatch {
    /// Matched skill.
    pub name: SkillName,
    /// Which triggers fired.
    pub matched_triggers: Vec<String>,
}

/// A negative-scope block (DO-NOT-USE respected).
#[derive(Debug, Clone)]
pub struct BlockedMatch {
    /// The blocked skill.
    pub name: SkillName,
    /// Positive triggers that would have matched.
    pub matched_triggers: Vec<String>,
    /// Negative triggers that blocked the route.
    pub blocked_by: Vec<String>,
}

/// A subtree scope filter record.
#[derive(Debug, Clone)]
pub struct ScopeFiltered {
    /// The filtered skill.
    pub name: SkillName,
    /// Subtree the skill is scoped to.
    pub scope: PathBuf,
}

/// Routes a prompt against the given metadata surfaces.
pub fn route(metas: &[SkillMeta], query: &RouteQuery<'_>) -> RoutingDecision {
    let lowered = query.text.to_lowercase();
    let mut decision = RoutingDecision::default();
    for meta in metas {
        if !in_scope(meta, query.work_path, &mut decision) {
            continue;
        }
        let negatives: Vec<&String> = meta
            .negative_triggers
            .iter()
            .filter(|t| contains_ci(&lowered, t))
            .collect();
        if !negatives.is_empty() {
            decision.blocked.push(BlockedMatch {
                name: meta.name.clone(),
                matched_triggers: positive_hits(meta, &lowered),
                blocked_by: negatives.into_iter().cloned().collect(),
            });
            continue;
        }
        let hits = positive_hits(meta, &lowered);
        if !hits.is_empty() {
            decision.matched.push(RouteMatch {
                name: meta.name.clone(),
                matched_triggers: hits,
            });
        }
    }
    decision
}

/// Routes with a consent gate: unconsented matched skills become
/// `pending_consent` instead of routable.
pub fn route_with_consent(
    metas: &[SkillMeta],
    query: &RouteQuery<'_>,
    gate: &ConsentGate,
) -> RoutingDecision {
    let mut decision = route(metas, query);
    let (ready, pending): (Vec<RouteMatch>, Vec<RouteMatch>) = decision
        .matched
        .drain(..)
        .partition(|m| gate.is_granted(m.name.as_str()));
    decision.matched = ready;
    decision.pending_consent = pending.into_iter().map(|m| m.name).collect();
    decision
}

fn positive_hits(meta: &SkillMeta, lowered_text: &str) -> Vec<String> {
    let mut hits: Vec<String> = meta
        .triggers
        .iter()
        .filter(|t| trigger_matches(lowered_text, t))
        .cloned()
        .collect();
    if hits.is_empty() && name_words_matched(meta, lowered_text) {
        hits.push(meta.name.as_str().to_string());
    }
    hits
}

fn trigger_matches(lowered_text: &str, trigger: &str) -> bool {
    let t = trigger.to_lowercase();
    if t.is_ascii() && !t.contains(' ') && !t.contains('/') {
        return word_boundaries_contain(lowered_text, &t);
    }
    lowered_text.contains(&t)
}

fn word_boundaries_contain(haystack: &str, needle: &str) -> bool {
    let mut from = 0;
    while let Some(idx) = haystack[from..].find(needle) {
        let start = from + idx;
        let end = start + needle.len();
        let before_ok = haystack[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let after_ok = haystack[end..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric());
        if before_ok && after_ok {
            return true;
        }
        from = start + 1;
    }
    false
}

fn name_words_matched(meta: &SkillMeta, lowered_text: &str) -> bool {
    let words: Vec<String> = meta
        .name
        .as_str()
        .split('-')
        .filter(|w| w.len() >= 2 && w.chars().all(|c| c.is_ascii_alphanumeric()))
        .map(str::to_string)
        .collect();
    if words.len() < 2 {
        return false;
    }
    words
        .iter()
        .filter(|w| word_boundaries_contain(lowered_text, w))
        .count()
        >= 2
}

fn contains_ci(lowered_text: &str, phrase: &str) -> bool {
    lowered_text.contains(&phrase.to_lowercase())
}

fn in_scope(meta: &SkillMeta, work: Option<&Path>, decision: &mut RoutingDecision) -> bool {
    match (&meta.scope, work) {
        (None, _) => true,
        (Some(scope), Some(w)) => {
            if path_within(w, scope) {
                true
            } else {
                decision.scope_filtered.push(ScopeFiltered {
                    name: meta.name.clone(),
                    scope: scope.clone(),
                });
                false
            }
        }
        (Some(scope), None) => {
            decision.scope_filtered.push(ScopeFiltered {
                name: meta.name.clone(),
                scope: scope.clone(),
            });
            false
        }
    }
}

/// Component-wise path containment over normal path components only
/// (prefix/root dropped so verbatim and relative paths compare equally),
/// case-insensitive for Windows path semantics.
fn path_within(path: &Path, scope: &Path) -> bool {
    fn norm_parts(p: &Path) -> Vec<String> {
        p.components()
            .filter_map(|c| match c {
                std::path::Component::Normal(part) => Some(part.to_string_lossy().to_lowercase()),
                _ => None,
            })
            .collect()
    }
    let path_parts = norm_parts(path);
    let scope_parts = norm_parts(scope);
    path_parts.len() >= scope_parts.len() && path_parts[..scope_parts.len()] == scope_parts[..]
}
