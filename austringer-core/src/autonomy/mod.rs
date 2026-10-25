//! Autonomy (plan todo 8): the mode matrix, the matrix-driven inspector
//! gate, the Auto-mode three-layer classifier, SQLite permission
//! persistence, NL permission hints, and the `/mode` command.
//!
//! # Composition
//!
//! [`autonomy_chain`] mounts the [`AutonomyGate`] as the FIRST inspector
//! of the T7 chain (see [`crate::inspectors::chain`]) and wraps the ask
//! policy with [`AutonomyAskPolicy`]. The full precedence story:
//!
//! 1. plan-mode freeze and softDeny deny outright;
//! 2. the secret guard, the destructive floor, denial escalation, mode
//!    asks, and classifier asks all surface as chain asks — a permission
//!    DENY rule still blocks them, and the surviving ask routes to the
//!    (wrapped) ask policy: headless default refuses;
//! 3. capability toggles and stored grants are authoritative allows that
//!    suppress later ask-generators (the secret guard stays ahead of
//!    toggles inside the gate: secret-pattern targets ask first);
//! 4. repetition `Intervene` still blocks regardless.
//!
//! Mode switches NEVER touch the serialized tool schema (todo 6's
//! invariant): `/mode` writes the shared matrix; the schema render is
//! mode-independent by construction and pinned byte-identical by tests.

pub mod ask_policy;
pub mod classifier;
pub mod gate;
pub mod matrix;
pub mod mode_command;
pub mod nl_hints;
pub mod sanitize;
pub mod store_sqlite;

pub use ask_policy::AutonomyAskPolicy;
pub use classifier::{
    ClassifierConfig, ClassifyOutcome, FastPath, Intent, IntentStage, Risk, RiskStage,
    SanitizedRequest, SmartClassifier,
};
pub use gate::{AutonomyGate, DANGEROUS_COMMANDS, DENIAL_ESCALATION_THRESHOLD, capability_of};
pub use matrix::{
    AutonomyHandle, AutonomyMatrix, AutonomyMode, Capability, Fallback, ReviewPolicy,
};
pub use mode_command::ModeCommand;
pub use nl_hints::{HintParse, HintRefusal, ParsedHint, apply_hint, parse_hint};
pub use sanitize::{ALLOW_STRIP_PHRASES, sanitize};
pub use store_sqlite::{Clock, SqlitePermissionStore};

use std::sync::Arc;

use crate::inspectors::chain::{InspectorChain, InspectorConfig};
use crate::inspectors::secret_files::TranscriptRedactor;
use crate::inspectors::store::{AskPolicy, PermissionStore};

/// Everything [`autonomy_chain`] needs, grouped: the wiring is one value.
pub struct AutonomyParts {
    /// The shared matrix (`AutonomyHandle::shared` for every consumer).
    pub matrix: AutonomyHandle,
    /// The grant store (SQLite in production, `MemoryStore` in tests).
    pub store: Arc<dyn PermissionStore>,
    /// The T7 inspector configuration (rules, secret patterns, …).
    pub inspectors: InspectorConfig,
    /// The user-facing ask policy (headless: `RefuseAsks`).
    pub ask: Box<dyn AskPolicy>,
    /// Classifier stage A (intent) — injected, deterministic in tests.
    pub intent: Box<dyn IntentStage>,
    /// Classifier stage B (risk) — injected, deterministic in tests.
    pub risk: Box<dyn RiskStage>,
    /// Classifier tuning (loop-guard cap, fast-path list).
    pub classifier: ClassifierConfig,
}

/// Builds the autonomy-wired inspector chain: the gate first, then the
/// standard T7 order, with the ask policy wrapped for denial recording
/// and grant-memory downgrading.
#[must_use]
pub fn autonomy_chain(parts: AutonomyParts) -> InspectorChain {
    let redactor = TranscriptRedactor::from_globs(parts.inspectors.secret_files.patterns.clone());
    let gate = AutonomyGate::new(
        parts.matrix.shared(),
        parts.store.clone(),
        redactor,
        SmartClassifier::new(parts.classifier, parts.intent, parts.risk),
    );
    InspectorChain::with_prefix_inspectors(
        parts.inspectors,
        parts.store.clone(),
        vec![Box::new(gate)],
    )
    .with_ask_policy(Box::new(AutonomyAskPolicy::new(
        parts.ask,
        parts.store,
        parts.matrix,
    )))
}
