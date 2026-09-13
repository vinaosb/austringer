//! Post-task `/learn` distiller: a cheap-model-shaped seam
//! ([`DistillModel`], injected — mocked in tests, never a live LLM)
//! proposes SKILL.md content from the session trajectory. Proposals are
//! DRAFT-tier only: quality-gated input, untrusted-output scanning,
//! replay cases attached, and zero authority to touch the skills tree.

use crate::forge::ForgeError;
use crate::forge::delta::SkillDelta;
use crate::forge::document::{SkillSection, SkillSpec};
use crate::forge::eval::ReplayCase;
use crate::forge::journal::CorrectionJournal;
use crate::forge::quality::{QualityFilter, SessionTrajectory, TrajectoryExport};
use crate::forge::quarantine::{InjectionScanner, ScanFinding};

/// The request handed to the distill model.
#[derive(Debug, Clone, PartialEq)]
pub struct DistillRequest {
    /// The (already quality-admitted) trajectory.
    pub trajectory: SessionTrajectory,
    /// Rendered correction journal (already redacted).
    pub journal_excerpt: String,
}

/// A model-shaped proposal. UNTRUSTED OUTPUT: everything here is scanned
/// before it becomes a draft, and activation authority is zero.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DistillProposal {
    /// Proposed frontmatter.
    pub spec: SkillSpec,
    /// Proposed body sections.
    pub sections: Vec<SkillSection>,
    /// Why the model thinks this is a skill (shown to the approver).
    pub rationale: String,
}

/// Cheap-model seam for skill distillation. Implemented by a provider
/// adapter in production and by deterministic mocks in tests.
///
/// `async fn` (not `impl Future + Send`) deliberately: the forge awaits
/// this seam inline on its caller's task, and neither the mock nor the
/// gate needs a Send bound — forcing one would drag a runtime dependency
/// into a crate that otherwise has none.
#[allow(async_fn_in_trait)]
pub trait DistillModel {
    /// Proposes a skill from the request. `async` because the real seam
    /// is a network model call.
    async fn propose(&self, request: &DistillRequest) -> Result<DistillProposal, ForgeError>;
}

/// A staged draft: a proposal plus everything the trust gates need.
/// Drafts live in a namespace the loader never scans; activation is the
/// gate's job ([`crate::forge::gates`]).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DraftRecord {
    /// Stable draft id (content-hash prefix).
    pub id: String,
    /// The proposal.
    pub proposal: DistillProposal,
    /// Version of the live skill this updates (`None` = new skill).
    pub from_version: Option<String>,
    /// Scanner findings over proposal content (non-empty ⇒ inert).
    pub findings: Vec<ScanFinding>,
    /// Scripted replay set for eval-before-activate.
    pub replay_cases: Vec<ReplayCase>,
    /// When the draft was created (caller clock).
    pub created_unix: u64,
    /// For update drafts: the section delta to merge (required to touch
    /// an existing skill; whole-skill rewrites are unrepresentable).
    pub delta: Option<SkillDelta>,
}

impl DraftRecord {
    /// True when scanner findings make this draft unactivatable.
    pub fn is_inert(&self) -> bool {
        !self.findings.is_empty()
    }
}

/// Distill outcome for one session.
#[derive(Debug, Clone, PartialEq)]
pub enum LearnOutcome {
    /// A draft was produced (still needs approval + eval to activate).
    Distilled(Box<DraftRecord>),
    /// The quality filter rejected the session; nothing was proposed.
    Filtered {
        /// Which gate rejected it.
        reason: String,
    },
}

/// The `/learn` engine: quality filter → model call → untrusted-output
/// scan → draft.
pub struct SkillDistiller<M: DistillModel> {
    model: M,
    filter: QualityFilter,
    scanner: InjectionScanner,
}

impl<M: DistillModel> SkillDistiller<M> {
    /// Distiller over an injected model and quality floor.
    pub fn new(model: M, filter: QualityFilter) -> Self {
        Self {
            model,
            filter,
            scanner: InjectionScanner,
        }
    }

    /// Runs the post-task learn pass over one exported session.
    pub async fn learn(
        &self,
        export: &TrajectoryExport,
        journal: &CorrectionJournal,
    ) -> Result<LearnOutcome, ForgeError> {
        if let crate::forge::quality::FilterDecision::Reject { reason } = self.filter.admits(export)
        {
            return Ok(LearnOutcome::Filtered { reason });
        }
        let request = DistillRequest {
            trajectory: export.trajectory.clone(),
            journal_excerpt: journal.render(),
        };
        let proposal = self.model.propose(&request).await?;
        let findings = self.scan_proposal(&proposal);
        let replay_cases = export
            .trajectory
            .originating_prompts
            .iter()
            .map(|p| ReplayCase {
                prompt: p.clone(),
                should_route: true,
                expect_section: proposal.sections.first().map(|s| s.heading.clone()),
            })
            .chain(
                export
                    .trajectory
                    .negative_prompts
                    .iter()
                    .map(|p| ReplayCase {
                        prompt: p.clone(),
                        should_route: false,
                        expect_section: None,
                    }),
            )
            .collect();
        let id = draft_id(&proposal);
        Ok(LearnOutcome::Distilled(Box::new(DraftRecord {
            id,
            proposal,
            from_version: None,
            findings,
            replay_cases,
            created_unix: 0,
            delta: None,
        })))
    }

    /// Distills fetched source content (/learn-style synthesis) into a
    /// draft. Sessions get the quality filter; source content does not
    /// — but untrusted-output scanning applies to both, always.
    pub async fn distill_content(
        &self,
        content: &crate::forge::sources::SourceContent,
    ) -> Result<DraftRecord, ForgeError> {
        let request = DistillRequest {
            trajectory: crate::forge::sources::content_trajectory(content),
            journal_excerpt: String::new(),
        };
        let proposal = self.model.propose(&request).await?;
        let findings = self.scan_proposal(&proposal);
        let replay_cases = vec![ReplayCase {
            prompt: content.title.clone(),
            should_route: true,
            expect_section: proposal.sections.first().map(|s| s.heading.clone()),
        }];
        Ok(DraftRecord {
            id: draft_id(&proposal),
            proposal,
            from_version: None,
            findings,
            replay_cases,
            created_unix: 0,
            delta: None,
        })
    }

    /// Scans every surface of the proposal: model output is untrusted
    /// regardless of how good the session score was.
    fn scan_proposal(&self, proposal: &DistillProposal) -> Vec<ScanFinding> {
        let mut surface = proposal.rationale.clone();
        surface.push('\n');
        surface.push_str(&proposal.spec.description);
        for trigger in proposal
            .spec
            .triggers
            .iter()
            .chain(&proposal.spec.negative_triggers)
        {
            surface.push('\n');
            surface.push_str(trigger);
        }
        for section in &proposal.sections {
            surface.push('\n');
            surface.push_str(&section.heading);
            surface.push('\n');
            surface.push_str(&section.body);
        }
        self.scanner.scan(&surface)
    }
}

/// Content-derived draft id (blake3 prefix over the rendered essence).
fn draft_id(proposal: &DistillProposal) -> String {
    let mut essence = proposal.spec.name.as_str().to_string();
    essence.push_str(&proposal.spec.version);
    essence.push_str(&proposal.rationale);
    crate::lock::SkillsLockFile::hash_bytes(essence.as_bytes())
        .chars()
        .take(12)
        .collect()
}
