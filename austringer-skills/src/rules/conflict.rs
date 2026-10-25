//! Merge-conflict reporting: a typed [`RulesConflict`] is produced when a
//! lower-precedence layer attempts to override a higher-precedence rule for
//! the same id. Team-floor violations are flagged explicitly.

use super::{RuleLayer, RulesStack};

/// A merge conflict over one rule id.
#[derive(Debug, Clone, PartialEq)]
pub struct RulesConflict {
    /// The contested rule id.
    pub id: String,
    /// Layer that won.
    pub winner_layer: RuleLayer,
    /// Layer whose rule lost.
    pub offending_layer: RuleLayer,
    /// The losing rule's text.
    pub offending_text: String,
    /// True when a team rule beat a lower layer.
    pub team_floor: bool,
}

impl RulesConflict {
    /// True when this conflict is a team-floor enforcement.
    pub fn is_team_floor_violation(&self) -> bool {
        self.team_floor
    }
}

/// Computes the conflict report for a stack: every id with more than one
/// retained entry yields one conflict per losing entry.
pub(super) fn build_conflicts(stack: &RulesStack) -> Vec<RulesConflict> {
    let mut ids: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for e in &stack.entries {
        ids.insert(&e.id);
    }
    let mut conflicts = Vec::new();
    for id in ids {
        let winner = match stack.winner(id) {
            Some(w) => w,
            None => continue,
        };
        for loser in stack
            .entries
            .iter()
            .filter(|e| e.id == id && e.order != winner.order)
        {
            conflicts.push(RulesConflict {
                id: id.to_string(),
                winner_layer: winner.layer,
                offending_layer: loser.layer,
                offending_text: loser.text.clone(),
                team_floor: winner.layer == RuleLayer::Team && loser.layer != RuleLayer::Team,
            });
        }
    }
    conflicts
}
