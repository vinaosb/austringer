//! The overflow-recovery rule (plan todo 13, Cline lesson): a
//! deterministic basic-compaction fallback that NEVER depends on
//! another successful LLM call.
//!
//! When the provider rejects an over-long context (typed
//! [`ContextOverflow`] — adapters map their context-length errors onto
//! it) or the condensing model fails, [`basic_compact`] folds the
//! session deterministically: the SAME structural selection the
//! doctrine path runs (markers + recent files + capped subagent
//! summaries), the head/tail windows, and T12 folding over the
//! retained tool results. Recovery works with ZERO LLM availability —
//! a fallback that needs the model is not a fallback.

use super::CompactedSession;
use super::doctrine::{self, DoctrineConfig};
use crate::event::RecordedEvent;
use crate::eviction::{EvictionConfig, ToolReturnTable, fold_history};

/// The typed input when a provider rejects an over-long context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextOverflow {
    /// Tokens the rejected request carried.
    pub used_tokens: u64,
    /// The model's context window.
    pub window_tokens: u64,
}

impl ContextOverflow {
    /// Wraps the rejection numbers.
    #[must_use]
    pub const fn new(used_tokens: u64, window_tokens: u64) -> Self {
        Self {
            used_tokens,
            window_tokens,
        }
    }
}

/// The deterministic basic compaction. Zero LLM by construction: the
/// function has no condenser parameter at all. Tool results inside the
/// retained window fold through T12 (`<folded_tool_output …/>`
/// markers); folding is best-effort here — when the return-code table
/// is incomplete the ORIGINAL events survive rather than dropping a
/// code.
#[must_use]
pub fn basic_compact(
    events: &[RecordedEvent],
    tool_returns: &ToolReturnTable,
    config: &DoctrineConfig,
    overflow: &ContextOverflow,
) -> CompactedSession {
    let summary = doctrine::select(events, config);
    let folded = fold_history(&summary.retained, &fold_config(config), tool_returns)
        .unwrap_or(summary.retained.clone());
    let header = format!(
        "<basic_compaction reason=\"context-overflow\" used=\"{}\" window=\"{}\" \
         method=\"deterministic-structural-selection\"/>\n",
        overflow.used_tokens, overflow.window_tokens
    );
    let mut retained = summary.head.clone();
    retained.extend(folded);
    doctrine::compacted_from(
        format!("{header}{}", summary.render()),
        events,
        dedup_by_seq(retained),
        true,
    )
}

/// The T12 folding policy over the retained window: default protection
/// (first ~3 / last ~20 immune), default pruning budgets.
fn fold_config(config: &DoctrineConfig) -> EvictionConfig {
    EvictionConfig {
        protection: crate::eviction::ProtectionConfig {
            head_messages: config.head_events,
            ..crate::eviction::ProtectionConfig::default()
        },
        ..EvictionConfig::default()
    }
}

/// Merges the protected head into the retained window without
/// duplicating sequences (head first, then retained, skipping repeats).
fn dedup_by_seq(events: Vec<RecordedEvent>) -> Vec<RecordedEvent> {
    let mut seen = std::collections::BTreeSet::new();
    events
        .into_iter()
        .filter(|recorded| seen.insert(recorded.seq))
        .collect()
}
