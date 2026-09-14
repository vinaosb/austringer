//! The benchmark routing-table feeder + retraction-in-place.
//!
//! NORMATIVE CROSS-CRATE CONTRACT (doc-constant, sync-tested): the row
//! shape is EXACTLY todo 42's `BenchmarkEntrySpec` in
//! `harness-intel/src/routing/config.rs` — field names `model`
//! (String), `score` (f32), `price_per_mtok` (f64) — so the rows this
//! crate emits paste verbatim into a routing config's `benchmarks`
//! list. The sync test below pins the field names; changing either
//! side without the other fails it.
//!
//! Retraction-in-place (benchmark honesty): a claim that fails audit
//! is RETRACTED in place — the row stays, marked with the reason —
//! never silently deleted.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The pinned field names of the feeder row (doc-constant).
pub const BENCHMARK_ROW_FIELDS: [&str; 3] = ["model", "score", "price_per_mtok"];

/// One routing-table benchmark row (the todo-42 feeder shape).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BenchmarkRow {
    /// The proven model.
    pub model: String,
    /// Its benchmark score (Pass^k floor from the evals report).
    pub score: f32,
    /// List price per million tokens (drives "cheapest proven").
    pub price_per_mtok: f64,
    /// Retraction reason when the claim failed audit (`None` = live).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retracted: Option<String>,
}

impl BenchmarkRow {
    /// True unless retracted.
    #[must_use]
    pub fn live(&self) -> bool {
        self.retracted.is_none()
    }
}

/// Builds feeder rows from a ranking: score = the pair's Pass^k floor
/// (floors, not pass@1 peaks). A model appearing in several profiles
/// keeps its BEST floor (deterministic total order via `total_cmp`).
/// Unknown prices are rejected — the routing table never invents
/// costs.
///
/// # Errors
/// a ranked model missing from `prices`.
pub fn rows_from_ranking(
    ranking: &[crate::runner::RankedPair],
    prices: &BTreeMap<String, f64>,
) -> Result<Vec<BenchmarkRow>, String> {
    let mut best: BTreeMap<String, BenchmarkRow> = BTreeMap::new();
    for row in ranking {
        let price = prices
            .get(&row.pair.model)
            .ok_or_else(|| format!("no price for model {:?}", row.pair.model))?;
        let score = row.pass_k as f32;
        let entry = best
            .entry(row.pair.model.clone())
            .or_insert_with(|| BenchmarkRow {
                model: row.pair.model.clone(),
                score,
                price_per_mtok: *price,
                retracted: None,
            });
        if score.total_cmp(&entry.score) == std::cmp::Ordering::Greater {
            entry.score = score;
        }
    }
    Ok(best.into_values().collect())
}

/// Retracts a model's row IN PLACE (the row stays, marked, never
/// deleted).
pub fn retract(rows: &mut [BenchmarkRow], model: &str, reason: &str) {
    for row in rows {
        if row.model == model && row.retracted.is_none() {
            row.retracted = Some(reason.to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{RankedPair, SweepPair};

    fn ranked(model: &str, pass_k: f64) -> RankedPair {
        RankedPair {
            pair: SweepPair {
                model: model.to_owned(),
                profile: "p".to_owned(),
            },
            pass_k,
            mean: pass_k,
            tokens_solved: None,
        }
    }

    /// DOC-CONSTANT SYNC TEST: serialized rows carry exactly the
    /// todo-42 `BenchmarkEntrySpec` field names.
    #[test]
    fn serialized_rows_match_the_routing_table_field_contract() {
        let row = BenchmarkRow {
            model: "mini".to_owned(),
            score: 0.75,
            price_per_mtok: 0.25,
            retracted: None,
        };
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&row).unwrap_or_default())
                .unwrap_or_default();
        let mut keys: Vec<&str> = json
            .as_object()
            .map_or(Vec::new(), |m| m.keys().map(String::as_str).collect());
        keys.sort_unstable();
        let mut expected: Vec<&str> = BENCHMARK_ROW_FIELDS.to_vec();
        expected.sort_unstable();
        assert_eq!(keys, expected, "routing feeder shape drifted");
        assert_eq!(json["price_per_mtok"], serde_json::json!(0.25));
    }

    #[test]
    fn rows_use_pass_k_floors_and_reject_unknown_prices() {
        let mut prices = BTreeMap::new();
        prices.insert("mini".to_owned(), 0.25);
        prices.insert("big".to_owned(), 5.0);
        let ranking = vec![ranked("mini", 0.5), ranked("big", 1.0)];
        let rows = rows_from_ranking(&ranking, &prices).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(rows.len(), 2);
        let big = rows
            .iter()
            .find(|r| r.model == "big")
            .cloned()
            .unwrap_or_default();
        assert!((big.score - 1.0).abs() < 1e-6);
        assert!((big.price_per_mtok - 5.0).abs() < 1e-9);
        prices.remove("mini");
        assert!(rows_from_ranking(&ranking, &prices).is_err());
    }

    #[test]
    fn retraction_marks_in_place_never_deletes() {
        let mut rows = vec![BenchmarkRow {
            model: "mini".to_owned(),
            score: 0.5,
            price_per_mtok: 0.25,
            retracted: None,
        }];
        retract(&mut rows, "mini", "audit found cache-prompt leakage");
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].live());
        assert_eq!(
            rows[0].retracted.as_deref(),
            Some("audit found cache-prompt leakage")
        );
    }
}
