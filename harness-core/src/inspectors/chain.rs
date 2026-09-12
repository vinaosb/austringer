//! The inspector chain: sequential per-tool-call evaluation with deny-wins
//! precedence and authoritative-allow ask suppression, wired to the todo-5
//! `ToolApproval` stage.
//!
//! Execution happens only when the whole chain passes: the chain
//! implements [`ToolApproval`], and the agent loop's execution stage only
//! runs calls the approval stage kept — bypassing the chain requires
//! constructing the execution call directly. A `Deny` (or repetition
//! `Intervene`) blocks with a typed model-feedback payload; a surviving
//! `Ask` is routed to the ask policy (headless default refuses).

use std::sync::Arc;

use async_trait::async_trait;

use crate::agent_loop::PendingCall;
use crate::agent_loop::stages::{Decision, StageCtx, StageError, ToolApproval};
use crate::event::Event;
use crate::inspectors::permission::{PermissionConfig, PermissionInspector, extract_target};
use crate::inspectors::repetition::{RepetitionConfig, RepetitionInspector};
use crate::inspectors::secret_files::{SecretFileConfig, SecretFileInspector, TranscriptRedactor};
use crate::inspectors::security::{SecurityConfig, SecurityInspector};
use crate::inspectors::store::{
    AskPolicy, AskRequest, AskResolution, Grant, MemoryStore, PermissionStore, RefuseAsks,
};
use crate::inspectors::verdict::{CallInspector, Verdict};

/// Full inspector-chain configuration.
#[derive(Debug, Clone, Default)]
pub struct InspectorConfig {
    /// Permission rules (empty = everything asks).
    pub permission: PermissionConfig,
    /// Secret-file guard patterns and read-tool globs.
    pub secret_files: SecretFileConfig,
    /// Repetition detector tuning.
    pub repetition: RepetitionConfig,
    /// Optional LLM risk review (off by default).
    pub security: SecurityConfig,
}

/// The chain's final decision for one call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainOutcome {
    /// Execute the call; `notice` is an optional LLM-visible transcript
    /// notice to record alongside the approval (e.g. secret-read
    /// redaction placeholder).
    Allow {
        /// Optional transcript notice for the approval layer to append.
        notice: Option<String>,
    },
    /// Block execution; `feedback` becomes the typed model-feedback
    /// payload of the tool result.
    Block {
        /// Model-safe, redacted reason the call was refused.
        feedback: String,
    },
}

/// The sequential inspector chain over pending tool calls.
pub struct InspectorChain {
    inspectors: Vec<Box<dyn CallInspector>>,
    ask_policy: Box<dyn AskPolicy>,
    store: Arc<dyn PermissionStore>,
    redactor: TranscriptRedactor,
}

impl InspectorChain {
    /// Builds the default chain (permission -> secret-files -> repetition
    /// -> security) over a fresh in-memory grant store with the headless
    /// ask policy.
    #[must_use]
    pub fn new(config: InspectorConfig) -> Self {
        Self::with_store(config, Arc::new(MemoryStore::new()))
    }

    /// Builds the default chain over a caller-supplied grant store (todo 8
    /// installs the SQLite-backed store here).
    #[must_use]
    pub fn with_store(config: InspectorConfig, store: Arc<dyn PermissionStore>) -> Self {
        Self::with_prefix_inspectors(config, store, Vec::new())
    }

    /// Builds the default chain with caller-supplied inspectors run
    /// BEFORE the default order (todo 8's autonomy gate mounts here).
    /// Prefix inspectors follow the standard precedence: their
    /// `Deny`/`Intervene` short-circuits, their authoritative `Allow`
    /// suppresses later ask-generators, their `Ask` routes through the
    /// chain's ask policy.
    #[must_use]
    pub fn with_prefix_inspectors(
        config: InspectorConfig,
        store: Arc<dyn PermissionStore>,
        prefix: Vec<Box<dyn CallInspector>>,
    ) -> Self {
        let redactor = TranscriptRedactor::from_globs(config.secret_files.patterns.clone());
        let mut inspectors: Vec<Box<dyn CallInspector>> = prefix;
        inspectors.push(Box::new(PermissionInspector::new(
            config.permission,
            store.clone(),
            redactor.clone(),
        )));
        inspectors.push(Box::new(SecretFileInspector::new(config.secret_files)));
        inspectors.push(Box::new(RepetitionInspector::new(config.repetition)));
        inspectors.push(Box::new(SecurityInspector::new(config.security)));
        Self {
            inspectors,
            ask_policy: Box::new(RefuseAsks),
            store,
            redactor,
        }
    }

    /// Replaces the ask policy (tests inject scripted/capturing policies;
    /// the frontends inject the interactive approver).
    #[must_use]
    pub fn with_ask_policy(mut self, policy: Box<dyn AskPolicy>) -> Self {
        self.ask_policy = policy;
        self
    }

    /// Runs every inspector over one call and resolves the outcome.
    ///
    /// Precedence: the first `Deny`/`Intervene` short-circuits the chain
    /// and blocks; an authoritative `Allow` suppresses later `Ask`
    /// verdicts where the policy says so; the first surviving `Ask` goes
    /// to the ask policy (`y` = this call, `a` = stored grant). When no
    /// inspector had any opinion, the chain itself raises a fallback ask
    /// (fail toward the user, never a silent allow).
    pub async fn evaluate(&mut self, call: &PendingCall) -> Result<ChainOutcome, StageError> {
        let mut pending_ask: Option<(String, Option<String>)> = None;
        let mut authoritative_allow = false;
        for inspector in &mut self.inspectors {
            match inspector.inspect(call).await? {
                Verdict::Deny { reason } | Verdict::Intervene { reason } => {
                    return Ok(ChainOutcome::Block { feedback: reason });
                }
                Verdict::Allow { authoritative } => authoritative_allow |= authoritative,
                Verdict::Ask {
                    reason,
                    notice_on_approval,
                } => {
                    if !authoritative_allow && pending_ask.is_none() {
                        pending_ask = Some((reason, notice_on_approval));
                    }
                }
            }
        }
        let target = extract_target(call);
        let request_target = target.as_ref().map(|target| target.value.clone());
        let (reason, notice) = match pending_ask {
            Some(ask) => ask,
            None if authoritative_allow => return Ok(ChainOutcome::Allow { notice: None }),
            // Fallback ask: no inspector had an opinion, so fail toward
            // the user instead of silently allowing.
            None => (
                format!(
                    "approval required: tool '{}' target '{}' has no matching permission rule",
                    call.tool,
                    target.as_ref().map_or_else(
                        || "(no target)".to_owned(),
                        |target| { self.redactor.mask_path(&target.value) }
                    )
                ),
                None,
            ),
        };
        let request = AskRequest {
            tool: call.tool.clone(),
            target: request_target.clone(),
            reason: reason.clone(),
        };
        match self.ask_policy.resolve(request).await {
            AskResolution::Refuse => Ok(ChainOutcome::Block {
                feedback: format!("{reason} — the ask was refused (no approving user)"),
            }),
            AskResolution::GrantOnce => Ok(ChainOutcome::Allow { notice }),
            AskResolution::GrantAlways(scope) => {
                self.store
                    .record(&Grant {
                        tool: call.tool.clone(),
                        target: request_target,
                        scope,
                    })
                    .map_err(|error| StageError {
                        message: error.to_string(),
                    })?;
                Ok(ChainOutcome::Allow { notice })
            }
        }
    }
}

#[async_trait]
impl ToolApproval for InspectorChain {
    async fn approve(&mut self, ctx: &mut StageCtx<'_>) -> Result<Vec<Decision>, StageError> {
        let calls = ctx.pending_calls.clone();
        let mut decisions = Vec::with_capacity(calls.len());
        for call in &calls {
            match self.evaluate(call).await? {
                ChainOutcome::Allow { notice } => {
                    if let Some(text) = notice {
                        ctx.append(Event::System { text })?;
                    }
                    decisions.push(Decision::Allow);
                }
                ChainOutcome::Block { feedback } => {
                    decisions.push(Decision::Deny { reason: feedback });
                }
            }
        }
        Ok(decisions)
    }
}
