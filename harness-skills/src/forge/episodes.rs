//! Closed skill loop (harness-rs §26): episodes (situation → trace →
//! outcome) accumulate; PROVEN episodes promote through the distiller
//! into routable SKILL.md drafts; [`SkillUseTrace`]s where a skill
//! MISLED trigger [`SkillReviser`] fix-drafts. Self-evolution lives in
//! auditable, lintable files — never in hidden state.

use crate::forge::delta::{DeltaError, SectionChange, SkillDelta, next_version};
use crate::forge::document::SkillSection;
use crate::forge::quality::{
    SessionTrajectory, TaskOutcome, TrajectoryExport, TrajectoryTurn, TurnRole,
};

/// Minimum reward for an episode to count as PROVEN.
pub const PROVEN_REWARD: f64 = 0.5;

/// One captured episode.
#[derive(Debug, Clone, PartialEq)]
pub struct Episode {
    /// The situation the agent faced.
    pub situation: String,
    /// What the agent did, in order.
    pub trace: Vec<String>,
    /// Scalar reward for the outcome.
    pub reward: f64,
    /// How the task ended.
    pub outcome: TaskOutcome,
}

impl Episode {
    /// True when the episode is PROVEN (success + sufficient reward) and
    /// therefore promotable.
    pub fn is_proven(&self) -> bool {
        self.reward >= PROVEN_REWARD && matches!(self.outcome, TaskOutcome::Success)
    }

    /// Converts to the Atropos-style export the distiller consumes.
    pub fn into_export(self) -> TrajectoryExport {
        let situation = self.situation.clone();
        TrajectoryExport::new(
            SessionTrajectory {
                session_id: format!("episode:{situation}"),
                turns: std::iter::once(TrajectoryTurn {
                    role: TurnRole::User,
                    text: situation.clone(),
                })
                .chain(self.trace.into_iter().map(|step| TrajectoryTurn {
                    role: TurnRole::Assistant,
                    text: step,
                }))
                .collect(),
                originating_prompts: vec![situation],
                negative_prompts: Vec::new(),
                outcome: self.outcome,
            },
            self.reward,
        )
    }
}

/// Ordered episode accumulation.
#[derive(Debug, Clone, Default)]
pub struct EpisodeStore {
    episodes: Vec<Episode>,
}

impl EpisodeStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one episode.
    pub fn record(&mut self, episode: Episode) {
        self.episodes.push(episode);
    }

    /// Only the PROVEN episodes (the promotable set).
    pub fn proven(&self) -> Vec<&Episode> {
        self.episodes.iter().filter(|e| e.is_proven()).collect()
    }

    /// Total recorded episodes.
    pub fn len(&self) -> usize {
        self.episodes.len()
    }

    /// True when nothing is recorded.
    pub fn is_empty(&self) -> bool {
        self.episodes.is_empty()
    }
}

/// One observed skill use that may indicate the skill is wrong.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillUseTrace {
    /// Skill that was used.
    pub skill: String,
    /// Situation it was used in.
    pub situation: String,
    /// Whether the skill MISLED the agent.
    pub misled: bool,
    /// Evidence of the misleading (quoted in the fix-draft).
    pub evidence: String,
}

/// Produces fix-drafts from misleading use traces: a Guardrails section
/// delta documenting when NOT to use the skill, with the version bump
/// every delta carries. Non-misleading traces produce nothing.
pub struct SkillReviser;

const GUARDRAILS: &str = "Guardrails";

impl SkillReviser {
    /// Builds the fix-draft for one trace (or `None` when the skill did
    /// not mislead).
    pub fn fix_draft(
        trace: &SkillUseTrace,
        current_sections: &[SkillSection],
        current_version: &str,
    ) -> Result<Option<SkillDelta>, DeltaError> {
        if !trace.misled {
            return Ok(None);
        }
        let to_version = next_version(current_version)?;
        let body = format!("Do NOT use for {}: {}.", trace.situation, trace.evidence);
        let change = if current_sections.iter().any(|s| s.heading == GUARDRAILS) {
            SectionChange::Changed {
                heading: GUARDRAILS.to_string(),
                body,
            }
        } else {
            SectionChange::Added {
                heading: GUARDRAILS.to_string(),
                body,
            }
        };
        Ok(Some(SkillDelta {
            skill: trace.skill.clone(),
            from_version: current_version.to_string(),
            to_version,
            changes: vec![change],
        }))
    }
}
