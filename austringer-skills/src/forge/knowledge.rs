//! Knowledge suggestions (Devin §22 pattern): auto-generated from chat
//! feedback; REGENERABLE (items carry their source quote); updates
//! EXISTING items in place (same quote ⇒ same item); promotion path is
//! Organization → Enterprise only (typed refusal in reverse).

use std::collections::BTreeMap;

use crate::forge::ForgeError;

/// Where a knowledge item applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnowledgeScope {
    /// Org-level (default for chat-derived items).
    Organization,
    /// Enterprise-wide (promotion target; no reverse path).
    Enterprise,
}

/// One regenerable knowledge item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeItem {
    /// Stable id (derived from the source quote).
    pub id: String,
    /// Short title.
    pub title: String,
    /// Current body (regenerable from `source`).
    pub body: String,
    /// Applicability scope.
    pub scope: KnowledgeScope,
    /// The originating quote — the regeneration source of truth.
    pub source: String,
}

/// One piece of chat feedback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatFeedback {
    /// The quoted chat text the feedback refers to.
    pub quote: String,
    /// What the feedback says.
    pub instruction: String,
}

/// What an upsert did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KnowledgeOp {
    /// A new item was created.
    Created {
        /// New item id.
        id: String,
    },
    /// An existing item was updated in place.
    UpdatedInPlace {
        /// Updated item id.
        id: String,
    },
}

/// The knowledge store.
#[derive(Debug, Clone, Default)]
pub struct KnowledgeStore {
    items: BTreeMap<String, KnowledgeItem>,
}

impl KnowledgeStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates or updates the item for a feedback quote (id = content
    /// hash of the quote, so repeat feedback converges on one item).
    pub fn upsert_from_feedback(&mut self, feedback: &ChatFeedback) -> KnowledgeOp {
        let id = item_id(&feedback.quote);
        let body = format!("{} — {}", feedback.quote, feedback.instruction);
        let op = if self.items.contains_key(&id) {
            KnowledgeOp::UpdatedInPlace { id: id.clone() }
        } else {
            KnowledgeOp::Created { id: id.clone() }
        };
        let title = feedback.instruction.chars().take(60).collect::<String>();
        self.items.insert(
            id.clone(),
            KnowledgeItem {
                id: id.clone(),
                title,
                body,
                scope: KnowledgeScope::Organization,
                source: feedback.quote.clone(),
            },
        );
        op
    }

    /// One item by id.
    pub fn get(&self, id: &str) -> Option<&KnowledgeItem> {
        self.items.get(id)
    }

    /// All items.
    pub fn items(&self) -> impl Iterator<Item = &KnowledgeItem> {
        self.items.values()
    }

    /// Regenerates an item's body from its source (regenerable by
    /// design — the stored body is a cache of the source, never the
    /// other way round).
    pub fn regenerate(&mut self, id: &str, body: &str) -> Result<KnowledgeOp, ForgeError> {
        let item = self
            .items
            .get_mut(id)
            .ok_or_else(|| ForgeError::KnowledgeNotFound { id: id.to_string() })?;
        item.body = body.to_string();
        Ok(KnowledgeOp::UpdatedInPlace { id: id.to_string() })
    }

    /// Promotes an org item to enterprise scope (one direction only).
    pub fn promote_to_enterprise(&mut self, id: &str) -> Result<(), ForgeError> {
        let item = self
            .items
            .get_mut(id)
            .ok_or_else(|| ForgeError::KnowledgeNotFound { id: id.to_string() })?;
        match item.scope {
            KnowledgeScope::Organization => {
                item.scope = KnowledgeScope::Enterprise;
                Ok(())
            }
            KnowledgeScope::Enterprise => Err(ForgeError::PromotionPath {
                id: id.to_string(),
                scope: "Enterprise".to_string(),
            }),
        }
    }
}

fn item_id(quote: &str) -> String {
    crate::lock::SkillsLockFile::hash_bytes(quote.as_bytes())
        .chars()
        .take(12)
        .collect()
}
