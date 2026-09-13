//! Delta updates (ACE §7 mitigation): skills change through typed
//! section diffs — added/changed/removed — never whole-skill rewrites.
//! [`apply_delta`] merges a delta into the current sections preserving
//! every untouched section; version strings increment via
//! [`next_version`]. There is deliberately no "replace document" API.

use crate::forge::document::SkillSection;

/// A typed delta failure (the delta does not apply to these sections).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum DeltaError {
    /// A delta must carry at least one change.
    #[error("delta contains no changes")]
    NoChanges,
    /// `Changed`/`Removed` targeted a heading that does not exist.
    #[error("heading {heading:?} not found in current sections")]
    HeadingNotFound {
        /// The missing heading.
        heading: String,
    },
    /// `Added` collided with an existing heading.
    #[error("heading {heading:?} already exists")]
    HeadingAlreadyExists {
        /// The colliding heading.
        heading: String,
    },
    /// Two changes in one delta targeted the same heading.
    #[error("duplicate change for heading {heading:?}")]
    DuplicateChange {
        /// The doubly-targeted heading.
        heading: String,
    },
    /// Section headings must be non-empty (preamble is not targetable).
    #[error("empty heading")]
    EmptyHeading,
    /// Version string is not `major.minor.patch`.
    #[error("bad version {got:?}: expected major.minor.patch")]
    BadVersion {
        /// The offending string.
        got: String,
    },
}

/// One section-level change.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SectionChange {
    /// Append a new section.
    Added {
        /// New heading.
        heading: String,
        /// New body.
        body: String,
    },
    /// Replace the body of an existing section (position preserved).
    Changed {
        /// Existing heading.
        heading: String,
        /// Replacement body.
        body: String,
    },
    /// Drop a section.
    Removed {
        /// Existing heading.
        heading: String,
    },
}

impl SectionChange {
    /// The heading this change targets.
    pub fn heading(&self) -> &str {
        match self {
            Self::Added { heading, .. }
            | Self::Changed { heading, .. }
            | Self::Removed { heading } => heading,
        }
    }
}

/// A skill update: section changes plus the version transition this
/// update represents. The activation gate verifies `from_version`
/// against the live file before applying (stale drafts are typed errors).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkillDelta {
    /// Target skill name.
    pub skill: String,
    /// Version the delta was computed against.
    pub from_version: String,
    /// Version the merged document will carry.
    pub to_version: String,
    /// Ordered section changes.
    pub changes: Vec<SectionChange>,
}

/// Increments the patch segment (`1.0.0` → `1.0.1`).
pub fn next_version(current: &str) -> Result<String, DeltaError> {
    let parts: Vec<&str> = current.split('.').collect();
    if parts.len() != 3 {
        return Err(DeltaError::BadVersion {
            got: current.to_string(),
        });
    }
    let mut nums = [0u64; 3];
    for (slot, part) in nums.iter_mut().zip(&parts) {
        *slot = part.parse().map_err(|_| DeltaError::BadVersion {
            got: current.to_string(),
        })?;
    }
    nums[2] = nums[2].saturating_add(1);
    Ok(format!("{}.{}.{}", nums[0], nums[1], nums[2]))
}

/// Merges a delta into the current sections. Untouched sections are
/// preserved structurally identical — this is the structural guarantee
/// that updates are refinements, not rewrites.
pub fn apply_delta(
    current: &[SkillSection],
    delta: &SkillDelta,
) -> Result<Vec<SkillSection>, DeltaError> {
    if delta.changes.is_empty() {
        return Err(DeltaError::NoChanges);
    }
    let mut seen: Vec<&str> = Vec::new();
    for change in &delta.changes {
        let heading = change.heading();
        if heading.is_empty() {
            return Err(DeltaError::EmptyHeading);
        }
        if seen.contains(&heading) {
            return Err(DeltaError::DuplicateChange {
                heading: heading.to_string(),
            });
        }
        seen.push(heading);
    }
    let mut merged = current.to_vec();
    for change in &delta.changes {
        match change {
            SectionChange::Added { heading, body } => {
                if merged.iter().any(|s| s.heading == *heading) {
                    return Err(DeltaError::HeadingAlreadyExists {
                        heading: heading.clone(),
                    });
                }
                merged.push(SkillSection {
                    heading: heading.clone(),
                    body: body.clone(),
                });
            }
            SectionChange::Changed { heading, body } => {
                let Some(section) = merged.iter_mut().find(|s| s.heading == *heading) else {
                    return Err(DeltaError::HeadingNotFound {
                        heading: heading.clone(),
                    });
                };
                section.body = body.clone();
            }
            SectionChange::Removed { heading } => {
                let before = merged.len();
                merged.retain(|s| s.heading != *heading);
                if merged.len() == before {
                    return Err(DeltaError::HeadingNotFound {
                        heading: heading.clone(),
                    });
                }
            }
        }
    }
    Ok(merged)
}
