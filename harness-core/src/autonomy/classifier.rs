//! The Auto-mode three-layer classifier (Qwen, research §18).
//!
//! Layer 1 — fast-path exceptions: a config list of (tool, target) globs
//! that allow without any model call (read-only single-file ops, safe
//! commands).
//!
//! Layer 2 — safe-tool allowlist: the autonomy matrix's capability
//! toggles (see [`crate::autonomy::matrix`]); the gate consults that layer
//! before reaching this engine, so the engine only sees off-capability
//! calls.
//!
//! Layer 3 — two-stage LLM classifier with injected stage closures (NO
//! live APIs; tests use deterministic mocks): stage A classifies intent,
//! stage B rules on risk. The engine enforces three floors of its own:
//!
//! - **softDeny-on-exact-intent**: a stage-A `HarmfulExact` ruling soft-
//!   denies regardless of what stage B says;
//! - **loop guard**: the LLM stages are consulted at most K times per
//!   turn, then the engine falls back to Ask;
//! - **allow-stripping**: inputs are sanitized by
//!   [`crate::autonomy::sanitize`] before the stages see them.
//!
//! Stage transport failures fail safe to Ask — never to Allow.

use async_trait::async_trait;

use crate::agent_loop::PendingCall;
use crate::agent_loop::stages::StageError;
use crate::autonomy::sanitize::sanitize;
use crate::inspectors::glob::Glob;
use crate::inspectors::permission::extract_target;

/// Stage A's intent classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    /// The request is plainly benign.
    Benign,
    /// The request LITERALLY asks for the harmful thing (softDeny floor).
    HarmfulExact,
    /// Neither stage A nor the caller could tell.
    Unclear,
}

/// Stage B's risk ruling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Risk {
    /// Low risk of unintended consequences.
    Low,
    /// High risk: ask the user.
    High,
}

/// The sanitized request the classifier stages receive: tool, normalized
/// target, and an allow-stripped summary of the call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizedRequest {
    /// Tool name.
    pub tool: String,
    /// Normalized target (path or command), if the call carried one.
    pub target: Option<String>,
    /// Allow-stripped rendering of the call (bounded length).
    pub summary: String,
}

/// Stage A: intent classification over the sanitized request. Injected as
/// a closure-backed trait so tests stay deterministic and no live API is
/// ever required.
#[async_trait]
pub trait IntentStage: Send {
    /// Classifies the request's intent.
    async fn intent(&mut self, request: &SanitizedRequest) -> Result<Intent, StageError>;
}

/// Stage B: risk ruling, told what stage A concluded.
#[async_trait]
pub trait RiskStage: Send {
    /// Rules on the request's risk given `intent`.
    async fn risk(
        &mut self,
        request: &SanitizedRequest,
        intent: Intent,
    ) -> Result<Risk, StageError>;
}

/// One layer-1 fast-path exception: a tool glob plus an optional target
/// glob.
#[derive(Debug, Clone)]
pub struct FastPath {
    /// Glob over the tool name.
    pub tool: Glob,
    /// Optional glob over the normalized target.
    pub target: Option<Glob>,
}

/// Classifier tuning.
#[derive(Debug, Clone)]
pub struct ClassifierConfig {
    /// Loop guard: at most this many LLM consultations per turn (then
    /// fall back to Ask).
    pub max_consults_per_turn: u32,
    /// Layer-1 exception list.
    pub fast_path: Vec<FastPath>,
}

impl Default for ClassifierConfig {
    fn default() -> Self {
        Self {
            max_consults_per_turn: 3,
            fast_path: default_fast_path(),
        }
    }
}

fn default_fast_path() -> Vec<FastPath> {
    [
        "git status*",
        "git diff*",
        "git log*",
        "git show*",
        "ls*",
        "dir*",
    ]
    .iter()
    .filter_map(|pattern| {
        Some(FastPath {
            tool: Glob::new("execute_command").ok()?,
            target: Glob::new(pattern).ok(),
        })
    })
    .collect()
}

/// The layer-3 outcome the autonomy gate maps onto inspector verdicts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassifyOutcome {
    /// Layer 1 or a benign + low-risk ruling: proceed.
    Allow,
    /// Loop guard, unclear intent, high risk, or a stage failure: route
    /// to the ask policy.
    Ask {
        /// Model-safe reason.
        reason: String,
    },
    /// The request literally asked for the harmful thing.
    SoftDeny {
        /// Model-safe reason.
        reason: String,
    },
}

/// The three-layer engine. Owns the injected stages and the per-turn
/// consult counter.
pub struct SmartClassifier {
    config: ClassifierConfig,
    intent: Box<dyn IntentStage>,
    risk: Box<dyn RiskStage>,
    consults: u32,
    last_summary: Option<String>,
}

impl SmartClassifier {
    /// Assembles the engine over injected stages.
    pub fn new(
        config: ClassifierConfig,
        intent: Box<dyn IntentStage>,
        risk: Box<dyn RiskStage>,
    ) -> Self {
        Self {
            config,
            intent,
            risk,
            consults: 0,
            last_summary: None,
        }
    }

    /// LLM consultations consumed this turn (loop-guard probe).
    #[must_use]
    pub fn consults_this_turn(&self) -> u32 {
        self.consults
    }

    /// Resets the loop guard at a turn boundary (frontends call this when
    /// a new inference turn starts).
    pub fn reset_turn(&mut self) {
        self.consults = 0;
    }

    /// The sanitized summary built for the last decided call (sanitization
    /// probe).
    #[must_use]
    pub fn last_request_summary(&self) -> Option<&str> {
        self.last_summary.as_deref()
    }

    /// Runs the three layers over one call.
    pub async fn decide(&mut self, call: &PendingCall) -> ClassifyOutcome {
        let request = self.build_request(call);
        // Layer 1: fast-path exceptions, no model call, no consult.
        for exception in &self.config.fast_path {
            if !exception.tool.matches(&request.tool) {
                continue;
            }
            let target_hit = exception
                .target
                .as_ref()
                .is_none_or(|glob| request.target.as_ref().is_some_and(|t| glob.matches(t)));
            if target_hit {
                return ClassifyOutcome::Allow;
            }
        }
        // Loop guard: cap LLM consultations per turn.
        if self.consults >= self.config.max_consults_per_turn {
            return ClassifyOutcome::Ask {
                reason: format!(
                    "autonomy classifier loop guard: {} consultations this turn (cap {}); ask the user instead",
                    self.consults, self.config.max_consults_per_turn
                ),
            };
        }
        self.consults += 1;
        // Stage A — intent.
        let intent = match self.intent.intent(&request).await {
            Ok(intent) => intent,
            Err(error) => {
                return ClassifyOutcome::Ask {
                    reason: format!(
                        "classifier stage A unavailable, failing safe to ask: {}",
                        error.message
                    ),
                };
            }
        };
        // Stage B — risk ruling.
        let risk = match self.risk.risk(&request, intent).await {
            Ok(risk) => risk,
            Err(error) => {
                return ClassifyOutcome::Ask {
                    reason: format!(
                        "classifier stage B unavailable, failing safe to ask: {}",
                        error.message
                    ),
                };
            }
        };
        match (intent, risk) {
            (Intent::HarmfulExact, _) => ClassifyOutcome::SoftDeny {
                reason: "soft-denied: the request explicitly asks for a harmful action".to_owned(),
            },
            (Intent::Benign, Risk::Low) => ClassifyOutcome::Allow,
            (Intent::Benign, Risk::High) | (Intent::Unclear, _) => ClassifyOutcome::Ask {
                reason: format!(
                    "classifier defers to the user: intent {intent:?} risk {risk:?} for tool '{}' target '{}'",
                    request.tool,
                    request.target.as_deref().unwrap_or("(none)")
                ),
            },
        }
    }

    fn build_request(&mut self, call: &PendingCall) -> SanitizedRequest {
        let target = extract_target(call).map(|target| target.value);
        let mut summary = format!(
            "tool={} target={} arguments={}",
            call.tool,
            target.as_deref().unwrap_or("(none)"),
            serde_json::to_string(&call.arguments).unwrap_or_default()
        );
        if summary.chars().count() > 400 {
            summary = summary.chars().take(400).collect();
        }
        let summary = sanitize(&summary);
        self.last_summary = Some(summary.clone());
        SanitizedRequest {
            tool: call.tool.clone(),
            target,
            summary,
        }
    }
}
