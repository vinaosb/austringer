//! Hook execution: kind dispatch over the seams under a per-hook
//! timeout, untrusted-output parsing, and typed failures. Fail-policy
//! application belongs to the caller (deny-vs-continue differs per
//! gate); this module reports, callers decide.

use std::time::Duration;

use serde_json::Value;

use super::HookEvent;
use super::config::{HookKind, HookSpec};
use super::decision::{GateAction, HookOpinion, HookResponse, StopAction};
use super::manager::{HookFiring, HookOutcome};
use super::seams::{HookHttpRequest, PolicyVerdict, ShellOutput};

/// The four injected seams, boxed.
pub struct HookDeps {
    /// Shell-kind runner.
    pub shell: Box<dyn super::seams::ShellRunner>,
    /// HTTP-kind transport.
    pub http: Box<dyn super::seams::HttpTransport>,
    /// MCP-tool-kind invoker.
    pub mcp: Box<dyn super::seams::McpToolInvoker>,
    /// Prompt-kind evaluator.
    pub evaluator: Box<dyn super::seams::PromptEvaluator>,
}

/// Why one hook run failed. All variants feed the caller's fail policy.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HookError {
    /// The seam itself failed (spawn, socket, evaluator).
    #[error("hook transport failed: {0}")]
    Transport(super::seams::HookTransportError),
    /// The per-hook timeout fired.
    #[error("hook timed out after {after_secs}s")]
    Timeout {
        /// The configured timeout that fired.
        after_secs: u64,
    },
    /// A shell hook exited non-zero.
    #[error("shell hook exited with code {code}")]
    ShellExit {
        /// The observed exit code.
        code: i32,
    },
    /// An HTTP hook returned a non-2xx status.
    #[error("http hook returned status {status}")]
    HttpStatus {
        /// The observed status code.
        status: u16,
    },
    /// The untrusted output parsed but did not form a valid action.
    #[error("{0}")]
    Parse(super::decision::HookParseError),
    /// A prompt verdict arrived that the event cannot consume.
    #[error("verdict {verdict:?} is not applicable to event {event:?}")]
    VerdictMismatch {
        /// The verdict's name.
        verdict: &'static str,
        /// The event it arrived on.
        event: HookEvent,
    },
}

/// Runs one trusted hook for `event` under its timeout and reports the
/// typed outcome (including parse failures). Trust gating happens
/// before this is called.
pub async fn run_hook(
    spec: &HookSpec,
    event: HookEvent,
    payload: &Value,
    deps: &HookDeps,
) -> HookFiring {
    let outcome = execute(spec, event, payload, deps).await;
    HookFiring {
        hook: spec.name.clone(),
        event,
        payload: payload.clone(),
        fail_policy: spec.fail_policy,
        outcome,
    }
}

async fn execute(
    spec: &HookSpec,
    event: HookEvent,
    payload: &Value,
    deps: &HookDeps,
) -> HookOutcome {
    let timeout = Duration::from_secs(spec.timeout_secs.max(1));
    let ran = tokio::time::timeout(timeout, dispatch(spec, event, payload, deps)).await;
    match ran {
        Err(_) => HookOutcome::Failed(HookError::Timeout {
            after_secs: spec.timeout_secs,
        }),
        Ok(Ok(opinion)) => HookOutcome::Succeeded(opinion),
        Ok(Err(error)) => HookOutcome::Failed(error),
    }
}

async fn dispatch(
    spec: &HookSpec,
    event: HookEvent,
    payload: &Value,
    deps: &HookDeps,
) -> Result<HookOpinion, HookError> {
    match &spec.kind {
        HookKind::Shell { command } => {
            let ShellOutput { exit_code, stdout } = deps
                .shell
                .run(command, payload)
                .await
                .map_err(HookError::Transport)?;
            if exit_code != 0 {
                return Err(HookError::ShellExit { code: exit_code });
            }
            text_opinion(
                event,
                HookResponse::parse(&stdout).map_err(HookError::Parse)?,
            )
        }
        HookKind::Http { method, url } => {
            let request = HookHttpRequest {
                method: *method,
                url: url.clone(),
                body: serde_json::to_string(payload).unwrap_or_default(),
            };
            let response = deps
                .http
                .send(request)
                .await
                .map_err(HookError::Transport)?;
            if !(200..300).contains(&response.status) {
                return Err(HookError::HttpStatus {
                    status: response.status,
                });
            }
            text_opinion(
                event,
                HookResponse::parse(&response.body).map_err(HookError::Parse)?,
            )
        }
        HookKind::Mcp { server, tool } => {
            let body = deps
                .mcp
                .invoke(server, tool, payload)
                .await
                .map_err(HookError::Transport)?;
            text_opinion(event, HookResponse::parse(&body).map_err(HookError::Parse)?)
        }
        HookKind::Prompt { policy } => {
            let verdict = deps
                .evaluator
                .evaluate(policy, payload)
                .await
                .map_err(HookError::Transport)?;
            verdict_opinion(event, verdict)
        }
    }
}

fn text_opinion(
    event: HookEvent,
    response: Option<HookResponse>,
) -> Result<HookOpinion, HookError> {
    let Some(response) = response else {
        return Ok(HookOpinion::None);
    };
    let opinion = match event {
        HookEvent::PreToolUse => {
            HookOpinion::Gate(response.to_gate_action().map_err(HookError::Parse)?)
        }
        HookEvent::PostToolUse | HookEvent::PostToolUseFailure => {
            HookOpinion::PostTool(response.to_post_tool_action().map_err(HookError::Parse)?)
        }
        HookEvent::Stop => HookOpinion::Stop(response.to_stop_action().map_err(HookError::Parse)?),
        HookEvent::UserPromptSubmit => match response.additional_context {
            Some(context) => HookOpinion::Augment(context),
            None => HookOpinion::None,
        },
        _ => HookOpinion::None,
    };
    Ok(opinion)
}

fn verdict_opinion(event: HookEvent, verdict: PolicyVerdict) -> Result<HookOpinion, HookError> {
    let mismatch = |name: &'static str| HookError::VerdictMismatch {
        verdict: name,
        event,
    };
    let opinion = match (event, verdict) {
        (HookEvent::PreToolUse, PolicyVerdict::Allow)
        | (HookEvent::PreToolUse, PolicyVerdict::Proceed) => HookOpinion::Gate(GateAction::Allow),
        (HookEvent::PreToolUse, PolicyVerdict::Deny { reason }) => {
            HookOpinion::Gate(GateAction::Reject { reason })
        }
        (HookEvent::PreToolUse, PolicyVerdict::ContinueWorking { .. }) => {
            return Err(mismatch("continue_working"));
        }
        (HookEvent::Stop, PolicyVerdict::Proceed) => HookOpinion::Stop(StopAction::Proceed),
        (HookEvent::Stop, PolicyVerdict::ContinueWorking { reason }) => {
            HookOpinion::Stop(StopAction::Block { reason })
        }
        (HookEvent::Stop, PolicyVerdict::Allow) => return Err(mismatch("allow")),
        (HookEvent::Stop, PolicyVerdict::Deny { .. }) => return Err(mismatch("deny")),
        (_, PolicyVerdict::Allow) | (_, PolicyVerdict::Proceed) => HookOpinion::None,
        (_, PolicyVerdict::Deny { .. }) => return Err(mismatch("deny")),
        (_, PolicyVerdict::ContinueWorking { .. }) => return Err(mismatch("continue_working")),
    };
    Ok(opinion)
}
