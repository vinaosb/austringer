//! The hook manager: config set + trust registry + execution seams +
//! the firing journal. All firing paths converge on
//! [`HookManager::fire_matching`], which enforces the trust gate
//! (inert until approved, re-inert on hash mismatch), runs trusted
//! hooks under their timeouts, and journals every firing with its
//! deterministic payload snapshot.

use serde_json::{Value, json};

use super::HookEvent;
use super::config::{FailPolicy, HookSpec, spec_hash};
use super::executor::{HookDeps, run_hook};
use super::trust::{TrustCheck, TrustRegistry};
use crate::event::content::ContentBlock;
use crate::inspectors::safe_outputs::SafeOutputs;

/// Default stop-hook iterate-until-goal budget (blocks per manager
/// lifetime; T5's per-run block cap remains the structural backstop).
pub const DEFAULT_STOP_LOOP_LIMIT: u32 = 3;

/// What one hook concluded on one firing (raw — fail policies are
/// applied by the pipeline surfaces).
#[derive(Debug, Clone, PartialEq)]
pub enum HookOutcome {
    /// Never ran: no approval on record.
    InertPendingReview,
    /// Never ran: content hash no longer matches the pin.
    InertModified,
    /// Ran; the opinion may still be neutral ([`HookOpinion::None`]).
    Succeeded(super::decision::HookOpinion),
    /// Ran and failed (timeout, transport, non-zero exit, bad output).
    Failed(super::executor::HookError),
}

impl HookOutcome {
    /// True when the hook stayed inert pending review.
    #[must_use]
    pub const fn is_inert_pending_review(&self) -> bool {
        matches!(self, Self::InertPendingReview)
    }

    /// True when the hook stayed inert because its config changed.
    #[must_use]
    pub const fn is_inert_modified(&self) -> bool {
        matches!(self, Self::InertModified)
    }

    /// True for either inert outcome.
    #[must_use]
    pub const fn is_inert(&self) -> bool {
        matches!(self, Self::InertPendingReview | Self::InertModified)
    }

    /// True when the hook actually executed.
    #[must_use]
    pub const fn ran(&self) -> bool {
        !self.is_inert()
    }

    /// True when the hook ran and failed.
    #[must_use]
    pub const fn is_failure(&self) -> bool {
        matches!(self, Self::Failed(_))
    }
}

/// One journal record: which hook, which event, the exact payload, and
/// the typed outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct HookFiring {
    /// The hook's name.
    pub hook: String,
    /// The trigger point.
    pub event: HookEvent,
    /// The deterministic payload snapshot (sorted-key JSON).
    pub payload: Value,
    /// The hook's fail policy (what the caller applies on failure).
    pub fail_policy: FailPolicy,
    /// What happened.
    pub outcome: HookOutcome,
}

/// Owns the hook configuration and drives every firing.
pub struct HookManager {
    specs: Vec<HookSpec>,
    deps: HookDeps,
    trust: TrustRegistry,
    journal: Vec<HookFiring>,
    started: bool,
    safe: SafeOutputs,
    stop_limit: u32,
    stop_used: u32,
}

impl HookManager {
    /// A manager whose every config starts pending review (the Codex
    /// first-use posture).
    #[must_use]
    pub fn new(specs: Vec<HookSpec>, deps: HookDeps) -> Self {
        Self {
            specs,
            deps,
            trust: TrustRegistry::new(),
            journal: Vec::new(),
            started: false,
            safe: SafeOutputs::default(),
            stop_limit: DEFAULT_STOP_LOOP_LIMIT,
            stop_used: 0,
        }
    }

    /// Pins the CURRENT content hash of every configured hook
    /// (test/review-tool convenience; interactive surfaces approve
    /// one-by-one).
    pub fn approve_all(&mut self) {
        for spec in &self.specs {
            let hash = spec_hash(spec);
            self.trust.approve(&spec.name, hash);
        }
    }

    /// Pins the current content hash of one hook by name. Returns
    /// whether a hook with that name exists.
    pub fn approve(&mut self, name: &str) -> bool {
        let Some(spec) = self.specs.iter().find(|spec| spec.name == name) else {
            return false;
        };
        self.trust.approve(name, spec_hash(spec));
        true
    }

    /// Drops a hook's pin (back to pending review).
    pub fn revoke(&mut self, name: &str) {
        self.trust.revoke(name);
    }

    /// The trust registry (serialize a clone for restart persistence).
    #[must_use]
    pub fn trust_registry(&self) -> &TrustRegistry {
        &self.trust
    }

    /// Restores a trust registry (the restart path).
    pub fn set_trust_registry(&mut self, trust: TrustRegistry) {
        self.trust = trust;
    }

    /// The firing journal.
    #[must_use]
    pub fn journal(&self) -> &[HookFiring] {
        &self.journal
    }

    /// Replaces the output guard (threat scanner + budget) used to
    /// sanitize synthesized/replaced results and cap augmentations.
    #[must_use]
    pub fn with_safe_outputs(mut self, safe: SafeOutputs) -> Self {
        self.safe = safe;
        self
    }

    /// Sets the stop-hook iterate-until-goal budget.
    pub fn set_stop_loop_limit(&mut self, limit: u32) {
        self.stop_limit = limit;
    }

    /// Whether the stop-loop budget is spent.
    #[must_use]
    pub const fn stop_budget_is_exhausted(&self) -> bool {
        self.stop_used >= self.stop_limit
    }

    /// Consumes one stop-loop iteration (caller checked exhaustion).
    pub fn consume_stop_iteration(&mut self) {
        self.stop_used = self.stop_used.saturating_add(1);
    }

    /// The configured stop-loop limit.
    #[must_use]
    pub const fn stop_loop_limit(&self) -> u32 {
        self.stop_limit
    }

    /// Fires every matching, trusted hook for `event` with `payload`,
    /// journaling each firing. Inert hooks journal without executing.
    pub async fn fire(&mut self, event: HookEvent, payload: Value) -> Vec<HookFiring> {
        self.fire_matching(event, None, payload).await
    }

    /// [`HookManager::fire`] with a tool-name filter for the tool
    /// events (the `|`-separated matcher glob list applies).
    pub async fn fire_matching(
        &mut self,
        event: HookEvent,
        tool: Option<&str>,
        payload: Value,
    ) -> Vec<HookFiring> {
        let mut fired = Vec::new();
        for spec in &self.specs {
            if !spec.events.contains(&event) {
                continue;
            }
            if let Some(tool) = tool
                && (matches!(
                    event,
                    HookEvent::PreToolUse | HookEvent::PostToolUse | HookEvent::PostToolUseFailure
                ))
                && !spec.matches_tool(tool)
            {
                continue;
            }
            let hash = spec_hash(spec);
            let outcome = match self.trust.classify(&spec.name, &hash) {
                TrustCheck::Approved => run_hook(spec, event, &payload, &self.deps).await.outcome,
                TrustCheck::PendingReview => HookOutcome::InertPendingReview,
                TrustCheck::Modified { .. } => HookOutcome::InertModified,
            };
            fired.push(HookFiring {
                hook: spec.name.clone(),
                event,
                payload: payload.clone(),
                fail_policy: spec.fail_policy,
                outcome,
            });
        }
        self.journal.extend(fired.iter().cloned());
        fired
    }

    /// Sanitizes untrusted synthesized/replaced result text through the
    /// threat-scan seam and output budget.
    #[must_use]
    pub fn sanitize_result(&self, text: &str) -> Vec<ContentBlock> {
        self.safe
            .sanitize(vec![ContentBlock::Text {
                text: text.to_owned(),
            }])
            .blocks
    }

    /// Budget-caps untrusted prompt-augmentation text.
    #[must_use]
    pub fn cap_augmentation(&self, text: &str) -> String {
        self.safe.budget.truncate(text)
    }

    /// Fires SessionEnd.
    pub async fn session_end(&mut self) {
        self.fire(HookEvent::SessionEnd, json!({"event": "session_end"}))
            .await;
    }

    /// Fires Notification with `message`.
    pub async fn notify(&mut self, message: &str) {
        self.fire(
            HookEvent::Notification,
            json!({"event": "notification", "message": message}),
        )
        .await;
    }

    /// Fires SubagentStart for `agent`.
    pub async fn subagent_start(&mut self, agent: &str) {
        self.fire(
            HookEvent::SubagentStart,
            json!({"agent": agent, "event": "subagent_start"}),
        )
        .await;
    }

    /// Fires SubagentStop for `agent`.
    pub async fn subagent_stop(&mut self, agent: &str) {
        self.fire(
            HookEvent::SubagentStop,
            json!({"agent": agent, "event": "subagent_stop"}),
        )
        .await;
    }

    /// Caps result text for payload snapshots (keeps payloads bounded).
    #[must_use]
    pub fn cap_result_text(&self, text: &str) -> String {
        self.safe.budget.truncate(text)
    }

    /// The once-per-lifetime SessionStart latch (the entry stage trips
    /// it).
    pub(super) fn trip_start(&mut self) -> bool {
        let already = self.started;
        self.started = true;
        !already
    }
}
