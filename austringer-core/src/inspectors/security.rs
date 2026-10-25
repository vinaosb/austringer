//! Optional SecurityInspector: LLM risky-call review, OFF BY DEFAULT.
//!
//! When enabled, calls that look risky (any configured marker substring
//! found in the tool name or in any string value of the arguments, at any
//! nesting depth) are routed to an injected [`RiskReviewer`] before
//! execution. There is NO live API here: tests inject mock reviewers, and
//! the real LLM-backed reviewer arrives with the provider stack. A review
//! that exceeds [`SecurityConfig::review_timeout`] fails closed (deny),
//! as does an enabled inspector with no reviewer configured.

use std::time::Duration;

use async_trait::async_trait;

use crate::agent_loop::PendingCall;
use crate::agent_loop::stages::StageError;
use crate::inspectors::verdict::{CallInspector, Verdict};

/// Security reviewer configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityConfig {
    /// Whether risky calls are reviewed at all. Off by default.
    pub enabled: bool,
    /// Heuristic markers: a call is risky-looking when any marker is a
    /// substring of the tool name or of any string value in the
    /// arguments.
    pub markers: Vec<String>,
    /// Review deadline; expiry denies the call (fail closed).
    pub review_timeout: Duration,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            markers: Vec::new(),
            review_timeout: Duration::from_secs(10),
        }
    }
}

/// One reviewer judgment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewOutcome {
    /// The call may proceed.
    Approve,
    /// The call is refused; `reason` reaches the model as feedback.
    Deny {
        /// Why the reviewer refused the call.
        reason: String,
    },
}

/// Async risk-review seam over one risky-looking call.
#[async_trait]
pub trait RiskReviewer: Send {
    /// Judges the call. Implementations must be prompt-responsive; the
    /// inspector enforces the timeout.
    async fn review(&mut self, call: &PendingCall) -> ReviewOutcome;
}

/// Fail-closed default reviewer: an enabled inspector without a reviewer
/// denies every risky call rather than allowing it unreviewed.
#[derive(Debug, Clone, Copy, Default)]
pub struct DenyReviewer;

#[async_trait]
impl RiskReviewer for DenyReviewer {
    async fn review(&mut self, _call: &PendingCall) -> ReviewOutcome {
        ReviewOutcome::Deny {
            reason: "no reviewer configured".to_owned(),
        }
    }
}

/// The optional security inspector.
pub struct SecurityInspector {
    config: SecurityConfig,
    reviewer: Box<dyn RiskReviewer>,
}

impl SecurityInspector {
    /// Builds the inspector with the fail-closed default reviewer.
    #[must_use]
    pub fn new(config: SecurityConfig) -> Self {
        Self {
            config,
            reviewer: Box::new(DenyReviewer),
        }
    }

    /// Builds the inspector with an injected reviewer.
    #[must_use]
    pub fn with_reviewer(config: SecurityConfig, reviewer: Box<dyn RiskReviewer>) -> Self {
        Self { config, reviewer }
    }

    fn is_risky(&self, call: &PendingCall) -> bool {
        let marker_in_text = |text: &str| {
            self.config
                .markers
                .iter()
                .any(|marker| !marker.is_empty() && text.contains(marker.as_str()))
        };
        marker_in_text(&call.tool) || any_string_value(&call.arguments, &marker_in_text)
    }
}

/// Whether any string value in the JSON tree satisfies `predicate`.
fn any_string_value(value: &serde_json::Value, predicate: &dyn Fn(&str) -> bool) -> bool {
    match value {
        serde_json::Value::String(text) => predicate(text),
        serde_json::Value::Array(items) => {
            items.iter().any(|item| any_string_value(item, predicate))
        }
        serde_json::Value::Object(map) => {
            map.values().any(|item| any_string_value(item, predicate))
        }
        _ => false,
    }
}

#[async_trait]
impl CallInspector for SecurityInspector {
    fn name(&self) -> &'static str {
        "security"
    }

    async fn inspect(&mut self, call: &PendingCall) -> Result<Verdict, StageError> {
        if !self.config.enabled || !self.is_risky(call) {
            return Ok(Verdict::Allow {
                authoritative: false,
            });
        }
        let review =
            tokio::time::timeout(self.config.review_timeout, self.reviewer.review(call)).await;
        match review {
            Ok(ReviewOutcome::Approve) => Ok(Verdict::Allow {
                authoritative: false,
            }),
            Ok(ReviewOutcome::Deny { reason }) => Ok(Verdict::Deny {
                reason: format!("security review denied: {reason}"),
            }),
            Err(_elapsed) => Ok(Verdict::Deny {
                reason: format!(
                    "security review timed out after {}ms — failing closed",
                    self.config.review_timeout.as_millis()
                ),
            }),
        }
    }
}
