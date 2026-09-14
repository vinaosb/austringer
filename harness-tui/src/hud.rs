//! HUD view model: context gauge, cache-hit percentage, per-model cost
//! breakdown, and routing-savings estimate. Honest-unknown posture:
//! fields the daemon does not feed yet render as explicitly absent,
//! never as invented numbers.

/// Per-model cost row (Devin /session-stats pattern).
#[derive(Debug, Clone, PartialEq)]
pub struct ModelCost {
    /// Model name.
    pub model: String,
    /// Accumulated cost in USD.
    pub cost: f64,
    /// Turns billed on this model.
    pub turns: u64,
}

/// Typed delta feeding the HUD as usage data arrives.
#[derive(Debug, Clone, PartialEq)]
pub struct HudUpdate {
    /// Cache-hit percentage, when the provider reports one.
    pub cache_hit_pct: Option<f64>,
    /// Model the turn ran on.
    pub model: String,
    /// Cost delta for that turn.
    pub cost_delta: f64,
    /// Routing savings vs the manual pick for that turn.
    pub routing_savings: f64,
}

/// HUD state.
#[derive(Debug, Clone, PartialEq)]
pub struct Hud {
    /// Configured context window in tokens.
    pub context_window_tokens: u64,
    /// Estimated tokens in context (chars/4 over the transcript);
    /// `None` until any transcript text exists.
    pub context_used_tokens: Option<u64>,
    /// Cache-hit percentage, when reported.
    pub cache_hit_pct: Option<f64>,
    /// Per-model cost rows.
    pub model_costs: Vec<ModelCost>,
    /// Accumulated routing savings, when any priced turn ran.
    pub routing_savings: Option<f64>,
}

/// chars→tokens approximation for the context gauge (documented
/// estimate; real usage counters arrive with the model wiring).
const CHARS_PER_TOKEN: u64 = 4;

fn round_cents(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

impl Hud {
    /// HUD for a window of `context_window_tokens`.
    #[must_use]
    pub fn new(context_window_tokens: u64) -> Self {
        Self {
            context_window_tokens,
            context_used_tokens: None,
            cache_hit_pct: None,
            model_costs: Vec::new(),
            routing_savings: None,
        }
    }

    /// Re-estimate context usage from transcript size.
    pub fn estimate_context(&mut self, context_chars: usize) {
        self.context_used_tokens = Some(context_chars as u64 / CHARS_PER_TOKEN);
    }

    /// Apply one priced turn (or a cache-report-only update with a
    /// zero-cost delta). Money aggregates at cent precision (HUD
    /// estimate posture; the billing-exact ledger is todo 34's job).
    pub fn apply(&mut self, update: &HudUpdate) {
        if let Some(pct) = update.cache_hit_pct {
            self.cache_hit_pct = Some(pct);
        }
        if let Some(row) = self
            .model_costs
            .iter_mut()
            .find(|r| r.model == update.model)
        {
            row.cost = round_cents(row.cost + update.cost_delta);
            row.turns += 1;
        } else {
            self.model_costs.push(ModelCost {
                model: update.model.clone(),
                cost: round_cents(update.cost_delta),
                turns: 1,
            });
        }
        if update.cost_delta > 0.0 || update.routing_savings > 0.0 {
            let saved = round_cents(self.routing_savings.unwrap_or(0.0) + update.routing_savings);
            self.routing_savings = Some(saved);
        }
    }

    /// Gauge percentage 0-100 of the context window, when anything is
    /// measured.
    #[must_use]
    pub fn gauge_pct(&self) -> Option<u16> {
        let used = self.context_used_tokens?;
        let pct = (used * 100) / self.context_window_tokens.max(1);
        Some(pct.min(100) as u16)
    }

    /// One-line cache status for the HUD bar.
    #[must_use]
    pub fn cache_line(&self) -> String {
        match self.cache_hit_pct {
            Some(pct) => format!("cache: {pct:.1}%"),
            None => "cache: n/a".to_owned(),
        }
    }

    /// One-line cost summary: per-model rows plus routing savings, or
    /// the honest unpriced state.
    #[must_use]
    pub fn cost_line(&self) -> String {
        let total: f64 = self.model_costs.iter().map(|r| r.cost).sum();
        if total <= 0.0 && self.routing_savings.unwrap_or(0.0) <= 0.0 {
            return "cost: unpriced (no usage events yet)".to_owned();
        }
        let mut line = String::from("cost:");
        for row in &self.model_costs {
            line.push_str(&format!(" {} ${:.2}", row.model, row.cost));
        }
        if let Some(saved) = self.routing_savings
            && saved > 0.0
        {
            line.push_str(&format!(" | saved ${saved:.2} routing"));
        }
        line
    }

    /// One-line context summary (`used≈N/window [pct%]`).
    #[must_use]
    pub fn context_line(&self) -> String {
        match self.context_used_tokens {
            Some(used) => format!(
                "ctx: \u{2248}{}/{} ({}%)",
                used,
                self.context_window_tokens,
                self.gauge_pct().unwrap_or(0)
            ),
            None => format!("ctx: n/a/{}", self.context_window_tokens),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_hud_renders_honest_unknowns() {
        let hud = Hud::new(200_000);
        assert_eq!(hud.gauge_pct(), None);
        assert_eq!(hud.cache_line(), "cache: n/a");
        assert_eq!(hud.cost_line(), "cost: unpriced (no usage events yet)");
        assert_eq!(hud.context_line(), "ctx: n/a/200000");
    }

    #[test]
    fn context_estimate_is_labeled_and_bounded_by_window() {
        let mut hud = Hud::new(100);
        hud.estimate_context(4);
        assert_eq!(hud.context_used_tokens, Some(1));
        assert_eq!(hud.gauge_pct(), Some(1));
        assert_eq!(hud.context_line(), "ctx: \u{2248}1/100 (1%)");
        hud.estimate_context(4_000);
        assert_eq!(
            hud.gauge_pct(),
            Some(100),
            "gauge saturates, never overflows"
        );
    }

    #[test]
    fn priced_turns_accumulate_per_model_and_savings() {
        let mut hud = Hud::new(100);
        hud.apply(&HudUpdate {
            cache_hit_pct: Some(90.0),
            model: "frontier-x".into(),
            cost_delta: 0.10,
            routing_savings: 0.02,
        });
        hud.apply(&HudUpdate {
            cache_hit_pct: None,
            model: "cheap-y".into(),
            cost_delta: 0.01,
            routing_savings: 0.05,
        });
        hud.apply(&HudUpdate {
            cache_hit_pct: Some(50.0),
            model: "frontier-x".into(),
            cost_delta: 0.20,
            routing_savings: 0.0,
        });
        assert_eq!(hud.model_costs.len(), 2);
        assert_eq!(hud.model_costs[0].cost, 0.30);
        assert_eq!(hud.model_costs[0].turns, 2);
        assert_eq!(hud.model_costs[1].cost, 0.01);
        assert_eq!(hud.routing_savings, Some(0.07));
        assert_eq!(hud.cache_line(), "cache: 50.0%");
        let line = hud.cost_line();
        assert!(line.contains("frontier-x $0.30"), "{line}");
        assert!(line.contains("cheap-y $0.01"), "{line}");
        assert!(line.contains("saved $0.07 routing"), "{line}");
    }

    #[test]
    fn zero_cost_delta_alone_stays_unpriced() {
        let mut hud = Hud::new(100);
        hud.apply(&HudUpdate {
            cache_hit_pct: Some(10.0),
            model: "m".into(),
            cost_delta: 0.0,
            routing_savings: 0.0,
        });
        assert_eq!(hud.cost_line(), "cost: unpriced (no usage events yet)");
        assert_eq!(hud.cache_line(), "cache: 10.0%");
    }
}
