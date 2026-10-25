//! Builtin price tables (USD per million tokens, pinned as micro-USD).
//!
//! Point-in-time snapshot of each provider's published list prices for the
//! austringer's default model set. Vendors change prices; the correction path
//! is a runtime [`CostTable::with_override`](super::CostTable::with_override)
//! from user config, never downstream edits here.

use std::collections::BTreeMap;

use super::ModelPrices;

type Rows = &'static [(&'static str, ModelPrices)];

/// Anthropic list prices; cache writes bill at 1.25x input (5m TTL tier).
const ANTHROPIC: Rows = &[
    (
        "claude-sonnet-4-5",
        ModelPrices::new(3_000_000, 15_000_000, 300_000, 3_750_000),
    ),
    (
        "claude-opus-4-5",
        ModelPrices::new(5_000_000, 25_000_000, 500_000, 6_250_000),
    ),
    (
        "claude-haiku-4-5",
        ModelPrices::new(1_000_000, 5_000_000, 100_000, 1_250_000),
    ),
];

/// OpenAI list prices; cached input bills at half rate, cache writes are free.
const OPENAI: Rows = &[
    (
        "gpt-5.2",
        ModelPrices::new(1_250_000, 10_000_000, 125_000, 0),
    ),
    (
        "gpt-4o",
        ModelPrices::new(2_500_000, 10_000_000, 1_250_000, 0),
    ),
    ("gpt-4o-mini", ModelPrices::new(150_000, 600_000, 75_000, 0)),
];

/// Google list prices; implicit caching bills at the listed read rate.
const GOOGLE: Rows = &[
    (
        "gemini-2.5-pro",
        ModelPrices::new(1_250_000, 10_000_000, 310_000, 0),
    ),
    (
        "gemini-2.5-flash",
        ModelPrices::new(300_000, 2_500_000, 75_000, 0),
    ),
];

/// xAI list prices.
const XAI: Rows = &[
    (
        "grok-4",
        ModelPrices::new(3_000_000, 15_000_000, 300_000, 0),
    ),
    (
        "grok-3",
        ModelPrices::new(3_000_000, 15_000_000, 300_000, 0),
    ),
];

/// Assembles the builtin provider tables.
pub(super) fn builtin_tables() -> BTreeMap<String, BTreeMap<String, ModelPrices>> {
    [
        ("anthropic", ANTHROPIC),
        ("openai", OPENAI),
        ("google", GOOGLE),
        ("xai", XAI),
    ]
    .into_iter()
    .map(|(provider, rows)| {
        (
            provider.to_owned(),
            rows.iter()
                .map(|(model, prices)| ((*model).to_owned(), *prices))
                .collect::<BTreeMap<_, _>>(),
        )
    })
    .collect()
}
