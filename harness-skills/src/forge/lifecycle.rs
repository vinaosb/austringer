//! /curator-style lifecycle over an injected clock (Qwen pattern):
//! unused 30 days → stale, 90 days → archive; explicit pins protect.
//! Everything is a typed classification/suggestion — the curator never
//! deletes or moves files on its own.

use std::collections::BTreeMap;

/// Days without invocation before a skill is stale.
pub const STALE_AFTER_DAYS: u64 = 30;
/// Days without invocation before a skill is archived.
pub const ARCHIVE_AFTER_DAYS: u64 = 90;
/// Seconds per day.
const SECS_PER_DAY: u64 = 86_400;

/// Lifecycle classification for one skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleStage {
    /// Recently invoked.
    Active,
    /// Not invoked within [`STALE_AFTER_DAYS`] (or never invoked).
    Stale,
    /// Not invoked within [`ARCHIVE_AFTER_DAYS`].
    Archived,
    /// Explicitly pinned: never ages.
    Pinned,
}

/// The usage facts lifecycle decisions are made from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillVitals {
    /// Last invocation (None = never invoked).
    pub last_invoked_unix: Option<u64>,
    /// Explicit pin (protects from aging entirely).
    pub pinned: bool,
}

/// Classifies one skill at `now_unix`. Never-invoked skills classify as
/// stale (the strongest underuse signal); archive additionally requires
/// an actual age, which never-invoked skills have no data for.
pub fn classify(vitals: &SkillVitals, now_unix: u64) -> LifecycleStage {
    if vitals.pinned {
        return LifecycleStage::Pinned;
    }
    let Some(last) = vitals.last_invoked_unix else {
        return LifecycleStage::Stale;
    };
    let days = now_unix.saturating_sub(last) / SECS_PER_DAY;
    if days >= ARCHIVE_AFTER_DAYS {
        LifecycleStage::Archived
    } else if days >= STALE_AFTER_DAYS {
        LifecycleStage::Stale
    } else {
        LifecycleStage::Active
    }
}

/// What the curator suggests for one skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CuratorAction {
    /// Recently used: nothing to do.
    Keep,
    /// Unused past the stale threshold.
    MarkStale,
    /// Unused past the archive threshold.
    Archive,
    /// Pinned: kept forever.
    KeepPinned,
}

/// One curator decision (skill, stage, action).
#[derive(Debug, Clone, PartialEq)]
pub struct LifecycleAction {
    /// Skill the action applies to.
    pub skill: String,
    /// Classified stage.
    pub stage: LifecycleStage,
    /// Suggested action (suggestion only — no auto-delete exists).
    pub action: CuratorAction,
}

/// One idle-curator pass over all vitals.
pub fn curator_pass(
    records: &BTreeMap<String, SkillVitals>,
    now_unix: u64,
) -> Vec<LifecycleAction> {
    records
        .iter()
        .map(|(skill, vitals)| {
            let stage = classify(vitals, now_unix);
            let action = match stage {
                LifecycleStage::Active => CuratorAction::Keep,
                LifecycleStage::Stale => CuratorAction::MarkStale,
                LifecycleStage::Archived => CuratorAction::Archive,
                LifecycleStage::Pinned => CuratorAction::KeepPinned,
            };
            LifecycleAction {
                skill: skill.clone(),
                stage,
                action,
            }
        })
        .collect()
}
