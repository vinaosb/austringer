//! Per-response cost accounting over the builtin price tables.
//!
//! Prices are pinned as micro-USD (1e-6 USD) per million tokens and all
//! arithmetic is integer math — no floats anywhere in the money path. A model
//! absent from the table reports tokens-only with the [`Priced::Unpriced`]
//! marker: costs are NEVER invented for unknown models (arbitrary
//! OpenAI-compatible endpoints), and user-supplied price overrides merge on
//! top of the builtin tables at runtime.

mod tables;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::event::Usage;
use crate::identity::ModelIdentity;

/// Prices for one model, in micro-USD per million tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelPrices {
    /// Prompt input tokens.
    pub input_per_mtok: u64,
    /// Completion tokens.
    pub output_per_mtok: u64,
    /// Tokens served from the provider read cache (0 == not offered).
    pub cache_read_per_mtok: u64,
    /// Tokens written to the provider cache this request (0 == not billed).
    pub cache_write_per_mtok: u64,
}

impl ModelPrices {
    /// Prices from micro-USD-per-MTok values in the order
    /// (input, output, cache-read, cache-write).
    #[must_use]
    pub const fn new(
        input_per_mtok: u64,
        output_per_mtok: u64,
        cache_read_per_mtok: u64,
        cache_write_per_mtok: u64,
    ) -> Self {
        Self {
            input_per_mtok,
            output_per_mtok,
            cache_read_per_mtok,
            cache_write_per_mtok,
        }
    }
}

/// Exact cost of one completion under a [`ModelPrices`] entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostBreakdown {
    /// Input-token cost.
    pub input_usd_micros: u64,
    /// Output-token cost.
    pub output_usd_micros: u64,
    /// Cache-read cost.
    pub cache_read_usd_micros: u64,
    /// Cache-write cost.
    pub cache_write_usd_micros: u64,
    /// Sum of all components.
    pub total_usd_micros: u128,
}

impl CostBreakdown {
    /// Integer accounting: `tokens * price_micros_per_mtok / 1_000_000`,
    /// computed in u128 so no intermediate can overflow.
    #[must_use]
    pub fn account(prices: &ModelPrices, usage: Usage) -> Self {
        let line = |tokens: u64, per_mtok: u64| -> u64 {
            (u128::from(tokens) * u128::from(per_mtok) / 1_000_000_u128) as u64
        };
        let input_usd_micros = line(usage.input_tokens, prices.input_per_mtok);
        let output_usd_micros = line(usage.output_tokens, prices.output_per_mtok);
        let cache_read_usd_micros = line(usage.cache_read_tokens, prices.cache_read_per_mtok);
        let cache_write_usd_micros = line(usage.cache_write_tokens, prices.cache_write_per_mtok);
        let total_usd_micros = u128::from(input_usd_micros)
            + u128::from(output_usd_micros)
            + u128::from(cache_read_usd_micros)
            + u128::from(cache_write_usd_micros);
        Self {
            input_usd_micros,
            output_usd_micros,
            cache_read_usd_micros,
            cache_write_usd_micros,
            total_usd_micros,
        }
    }
}

/// Whether the accounting produced dollar figures. The `Unpriced` marker is
/// the typed "tokens-only" state the cost HUD renders for models outside
/// every table (never an invented zero cost).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priced {
    /// Cost figures under the model's price entry.
    Priced(CostBreakdown),
    /// Model absent from the tables: token counts only.
    Unpriced,
}

/// One completion's accounting result: raw usage plus pricing outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostReport {
    /// The usage this report accounts for.
    pub usage: Usage,
    /// Dollar figures, or the unpriced marker.
    pub priced: Priced,
}

/// Provider → (model alias → prices). Model aliases are the stable marketing
/// ids; dated snapshot ids (`claude-sonnet-4-5-20250929`) resolve to their
/// alias entry by version-boundary prefix match.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostTable {
    entries: BTreeMap<String, BTreeMap<String, ModelPrices>>,
}

impl CostTable {
    /// The builtin price tables (see [`tables`]); a point-in-time snapshot —
    /// user overrides are the correction mechanism, never table edits in
    /// downstream code.
    #[must_use]
    pub fn builtin() -> Self {
        Self {
            entries: tables::builtin_tables(),
        }
    }

    /// An empty table (embedder-supplied prices only).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Looks up prices for `model` under `provider`. Exact alias first, then
    /// the longest entry key that is a version-boundary prefix of `model`
    /// (`key` or `key + "-"`).
    #[must_use]
    pub fn lookup(&self, provider: &str, model: &str) -> Option<ModelPrices> {
        let models = self.entries.get(provider)?;
        if let Some(exact) = models.get(model) {
            return Some(*exact);
        }
        models
            .iter()
            .filter(|(key, _)| {
                model.len() > key.len()
                    && model.starts_with(key.as_str())
                    && model.as_bytes()[key.len()] == b'-'
            })
            .max_by_key(|(key, _)| key.len())
            .map(|(_, prices)| *prices)
    }

    /// Merges one user-supplied price entry (replaces an existing entry for
    /// the same provider+model, leaves everything else untouched).
    #[must_use]
    pub fn with_override(mut self, provider: &str, model: &str, prices: ModelPrices) -> Self {
        self.entries
            .entry(provider.to_owned())
            .or_default()
            .insert(model.to_owned(), prices);
        self
    }

    /// Accounts one completion. Unknown models yield [`Priced::Unpriced`].
    #[must_use]
    pub fn account(&self, identity: &ModelIdentity, usage: Usage) -> CostReport {
        let priced = match self.lookup(&identity.provider, &identity.model) {
            Some(prices) => Priced::Priced(CostBreakdown::account(&prices, usage)),
            None => Priced::Unpriced,
        };
        CostReport { usage, priced }
    }

    /// The model aliases known for `provider` (sorted).
    pub fn models(&self, provider: &str) -> impl Iterator<Item = &str> {
        self.entries
            .get(provider)
            .into_iter()
            .flat_map(BTreeMap::keys)
            .map(String::as_str)
    }
}
