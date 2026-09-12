//! The autonomy gate (plan todo 8): the inspector that turns the
//! [`AutonomyMatrix`] into per-call verdicts. Mounted FIRST in the
//! inspector chain, so the standard T7 precedence applies on top:
//! permission deny rules and repetition still block, stored grants and
//! allow rules still carry, and surviving asks route to the ask policy.
//!
//! Decision order inside the gate (first match wins):
//!
//! 1. **plan freeze** — plan mode denies `edit_lines`/`execute_command`
//!    outright (write tools frozen at the dispatch gate);
//! 2. **secret-pattern target** — reads or writes of secret-looking
//!    paths ask (unless `SecretRead` is toggled on);
//! 3. **destructive floor** — a known-destructive command pattern asks,
//!    in every mode, before any grant is consulted;
//! 4. **denial escalation** — a tool+target the user already refused
//!    three times is denied with repetition feedback;
//! 5. **stored grant** — an `a` answer in the permission store is an
//!    authoritative allow;
//! 6. **capability toggle** — an on-toggle is an authoritative allow
//!    (the safe-tool allowlist layer of the Auto classifier);
//! 7. **mode fallback** — Chat/Approve ask; Auto/SmartApprove consult
//!    the three-layer classifier.

use async_trait::async_trait;

use crate::agent_loop::PendingCall;
use crate::agent_loop::stages::StageError;
use crate::autonomy::classifier::{ClassifyOutcome, SmartClassifier};
use crate::autonomy::matrix::{AutonomyHandle, Capability, Fallback};
use crate::inspectors::glob::Glob;
use crate::inspectors::permission::{TargetKind, extract_target};
use crate::inspectors::secret_files::TranscriptRedactor;
use crate::inspectors::store::PermissionStore;
use crate::inspectors::verdict::{CallInspector, Verdict};
use std::sync::Arc;

/// Command glob patterns that ALWAYS ask, regardless of mode, toggle, or
/// grant — the hard floor under Auto. `**` tails: recursive deletes take
/// path arguments with separators. Pinned compile-valid by tests.
pub const DANGEROUS_COMMANDS: &[&str] = &[
    "rm -*rf**",
    "rm -*fr**",
    "rm -r *-f**",
    "rm -f *-r**",
    "rm --recursive**",
    "sudo rm **",
    "del */s**",
    "del */q**",
    "rd */s**",
    "rmdir */s**",
    "Remove-Item *-Recurse**",
    "format **",
    "mkfs**",
    "dd if=**",
    "shutdown**",
    "chmod -R **",
    "chown -R **",
    "curl *|*sh**",
    "curl *|*bash**",
    "wget *|*sh**",
    "wget *|*bash**",
    "*|*rm **",
    ":(){ **",
];

/// Refusals at or above this count escalate to a typed deny.
pub const DENIAL_ESCALATION_THRESHOLD: u32 = 3;

/// Maps a tool name onto its capability column (`None` for tools outside
/// the matrix — those ask).
#[must_use]
pub fn capability_of(tool: &str) -> Option<Capability> {
    match tool {
        "view_window" => Some(Capability::Read),
        "edit_lines" => Some(Capability::Edit),
        "execute_command" => Some(Capability::Exec),
        "lsp_query" => Some(Capability::Context),
        "tool_search" => Some(Capability::Search),
        "task_completed" => Some(Capability::Completion),
        _ => None,
    }
}

/// The matrix-driven inspector. Cloning the handle shares the matrix with
/// `/mode`; the classifier is owned (its loop guard resets per turn via
/// [`Self::reset_turn`]).
pub struct AutonomyGate {
    matrix: AutonomyHandle,
    store: Arc<dyn PermissionStore>,
    redactor: TranscriptRedactor,
    classifier: SmartClassifier,
    dangerous: Vec<Glob>,
}

impl AutonomyGate {
    /// Builds the gate over the shared matrix, grant store, secret
    /// redactor, and classifier.
    #[must_use]
    pub fn new(
        matrix: AutonomyHandle,
        store: Arc<dyn PermissionStore>,
        redactor: TranscriptRedactor,
        classifier: SmartClassifier,
    ) -> Self {
        Self {
            matrix,
            store,
            redactor,
            classifier,
            dangerous: DANGEROUS_COMMANDS
                .iter()
                .filter_map(|pattern| Glob::new(pattern).ok())
                .collect(),
        }
    }

    /// Resets the classifier loop guard at a turn boundary.
    pub fn reset_turn(&mut self) {
        self.classifier.reset_turn();
    }

    fn ask(reason: String) -> Verdict {
        Verdict::Ask {
            reason,
            notice_on_approval: None,
        }
    }
}

#[async_trait]
impl CallInspector for AutonomyGate {
    fn name(&self) -> &'static str {
        "autonomy"
    }

    async fn inspect(&mut self, call: &PendingCall) -> Result<Verdict, StageError> {
        let Some(capability) = capability_of(&call.tool) else {
            return Ok(Self::ask(format!(
                "tool '{}' is not covered by the autonomy matrix; approval required",
                call.tool
            )));
        };
        let target = extract_target(call);
        let (plan, secret_read_allowed, allowed, fallback) = self.matrix.read(|matrix| {
            (
                matrix.is_plan(),
                matrix.allows(Capability::SecretRead),
                matrix.allows(capability),
                matrix.fallback(),
            )
        });
        // 1. Plan freeze: write tools are frozen at the dispatch gate.
        if plan && matches!(capability, Capability::Edit | Capability::Exec) {
            return Ok(Verdict::Deny {
                reason: format!(
                    "plan mode: write tool '{}' is frozen at the dispatch gate; finish the plan and switch modes via /mode to make changes",
                    call.tool
                ),
            });
        }
        // 2. Secret-pattern targets ask for both reads and writes.
        if let Some(target) = target.as_ref()
            && self.redactor.is_secret_path(&target.value)
            && !secret_read_allowed
        {
            return Ok(Self::ask(format!(
                "secret-pattern target '{}' requires explicit approval",
                self.redactor.mask_path(&target.value)
            )));
        }
        // 3. Destructive floor: before grants, before toggles, always.
        if target.as_ref().is_some_and(|target| {
            target.kind == TargetKind::Command
                && self
                    .dangerous
                    .iter()
                    .any(|glob| glob.matches(&target.value))
        }) {
            return Ok(Self::ask(format!(
                "destructive command pattern on tool '{}' requires explicit approval",
                call.tool
            )));
        }
        // 4. Denial escalation: repetition feedback for refused pairs.
        let denial_key = target.as_ref().map(|target| target.value.as_str());
        if self.store.denial_count(&call.tool, denial_key) >= DENIAL_ESCALATION_THRESHOLD {
            return Ok(Verdict::Deny {
                reason: format!(
                    "tool '{}' with this target was already denied {} times by the user; change the approach or ask them to adjust permissions",
                    call.tool, DENIAL_ESCALATION_THRESHOLD
                ),
            });
        }
        // 5. Stored grants are authoritative allows.
        if self.store.lookup(&call.tool, denial_key).is_some() {
            return Ok(Verdict::Allow {
                authoritative: true,
            });
        }
        // 6. Capability toggles (classifier layer 2).
        if allowed {
            return Ok(Verdict::Allow {
                authoritative: true,
            });
        }
        // 7. Mode fallback.
        match fallback {
            Fallback::Ask => Ok(Self::ask(format!(
                "autonomy mode '{}' requires approval for capability {capability:?} (toggle off)",
                self.matrix.read(|matrix| matrix.mode().name())
            ))),
            Fallback::Classify => Ok(match self.classifier.decide(call).await {
                ClassifyOutcome::Allow => Verdict::Allow {
                    authoritative: true,
                },
                ClassifyOutcome::Ask { reason } => Self::ask(reason),
                ClassifyOutcome::SoftDeny { reason } => Verdict::Deny { reason },
            }),
        }
    }
}
