//! Orchestration-tree cost rollup (credit rollover, Warp §25): a parent
//! agent aggregates the spend of its whole subagent subtree, and each
//! child's subtree total is visible at the parent.

use super::money::Spend;

/// One node of the orchestration tree: an agent's own spend plus its
/// children. Aggregation is checked u128 arithmetic — a tree whose sum
/// overflows is a typed error, never a wrapped value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollupNode {
    /// The agent's label (arbitrary unicode).
    pub label: String,
    /// The agent's own spend (children NOT included).
    pub own: Spend,
    /// The agent's child agents.
    pub children: Vec<RollupNode>,
}

impl RollupNode {
    /// A node without children.
    #[must_use]
    pub fn leaf(label: &str, own: Spend) -> Self {
        Self {
            label: label.to_owned(),
            own,
            children: Vec::new(),
        }
    }

    /// A node with children.
    #[must_use]
    pub fn branch(label: &str, own: Spend, children: Vec<Self>) -> Self {
        Self {
            label: label.to_owned(),
            own,
            children,
        }
    }

    /// The subtree total: this node's own spend plus every descendant's,
    /// aggregated bottom-up with checked math.
    pub fn total(&self) -> Result<Spend, RollupError> {
        self.fold().ok_or(RollupError {
            label: self.label.clone(),
        })
    }

    /// Per-child subtree totals in child order — the parent's view of
    /// what each subagent (with its own subtree) cost.
    pub fn child_spend(&self) -> Result<Vec<(&str, Spend)>, RollupError> {
        self.children
            .iter()
            .map(|child| child.total().map(|total| (child.label.as_str(), total)))
            .collect()
    }

    fn fold(&self) -> Option<Spend> {
        let mut sum = self.own;
        for child in &self.children {
            sum = sum.checked_add(child.fold()?)?;
        }
        Some(sum)
    }
}

/// A typed rollup failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("rollup overflow aggregating subtree {label}")]
pub struct RollupError {
    /// The subtree whose aggregation overflowed.
    pub label: String,
}

#[cfg(test)]
mod tests {
    use super::{RollupError, RollupNode};
    use crate::budget::money::{MicroUsd, Spend, TokenCount};

    #[test]
    fn leaf_total_is_its_own_spend() {
        let leaf = RollupNode::leaf("a", Spend::priced(3, 30));
        assert_eq!(leaf.total(), Ok(Spend::priced(3, 30)));
    }

    #[test]
    fn overflow_reports_the_subtree_label() {
        let maxed = RollupNode::leaf("maxed", Spend::priced(1, u128::MAX));
        let root = RollupNode::branch("root", Spend::priced(1, 1), vec![maxed]);
        assert_eq!(
            root.total(),
            Err(RollupError {
                label: "root".to_owned()
            })
        );
    }

    #[test]
    fn unpriced_children_contribute_tokens_only() {
        let root = RollupNode::branch(
            "root",
            Spend::priced(10, 100),
            vec![RollupNode::leaf("c", Spend::unpriced(5))],
        );
        let total = root.total();
        assert!(total.is_ok(), "rollup failed: {total:?}");
        let Ok(total) = total else {
            return;
        };
        assert_eq!(total.tokens, TokenCount(15));
        assert_eq!(total.money(), MicroUsd(100));
    }
}
