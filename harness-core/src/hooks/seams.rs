//! Execution seams for the four hook kinds. Everything here is an
//! injected trait: this crate never spawns a process, opens a socket,
//! or calls a model — production adapters live in sibling crates and
//! tests run entirely on mocks.
//!
//! # Production wiring (documented, deliberately not a dependency)
//!
//! - **Shell**: `harness-exec`'s `PtyRunner` implements [`ShellRunner`]
//!   by building an `ExecSpec` from `command` with the hook's timeout as
//!   the command ceiling, feeding the payload JSON as stdin, and
//!   returning `model_text` as stdout (the daemon wires this; see todo
//!   19/29).
//! - **HTTP**: `harness-providers`' reqwest transport adapts onto
//!   [`HttpTransport`] (the T10 auth-flow seam pattern, reused for
//!   local hook calls).
//! - **MCP tool**: `harness-mcp`'s rmcp client implements
//!   [`McpToolInvoker`] once todo 29 lands.
//! - **Prompt**: the daemon injects an LLM-backed [`PromptEvaluator`]
//!   bound to the session's own model.

use async_trait::async_trait;
use serde_json::Value;

/// One typed hook-transport failure (runner/HTTP/MCP/evaluator level,
/// before any output parsing).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("hook transport failed: {message}")]
pub struct HookTransportError {
    /// What failed.
    pub message: String,
}

/// The outcome of one shell-hook run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellOutput {
    /// The command's exit code (non-zero is a typed hook failure).
    pub exit_code: i32,
    /// Captured stdout (parsed as hook-response JSON when non-empty).
    pub stdout: String,
}

/// HTTP request shape for http-kind hooks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookHttpRequest {
    /// Request method.
    pub method: HttpMethod,
    /// Target URL.
    pub url: String,
    /// Request body (the deterministic payload JSON).
    pub body: String,
}

/// HTTP method for http-kind hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpMethod {
    /// GET.
    Get,
    /// POST (the default for hook delivery).
    #[default]
    Post,
}

impl HttpMethod {
    /// Parses a foreign method string; anything unrecognized is POST.
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        if raw.eq_ignore_ascii_case("get") {
            Self::Get
        } else {
            Self::Post
        }
    }

    /// The uppercase wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
        }
    }
}

/// HTTP response shape for http-kind hooks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookHttpResponse {
    /// Response status (2xx expected; anything else is a typed failure).
    pub status: u16,
    /// Response body (parsed as hook-response JSON when non-empty).
    pub body: String,
}

/// Runs shell-kind hooks. Implementations own process spawning,
/// capture, and kill trees; they never outlive their await.
#[async_trait]
pub trait ShellRunner: Send + Sync {
    /// Runs `command`, handing it the deterministic payload.
    async fn run(&self, command: &str, payload: &Value) -> Result<ShellOutput, HookTransportError>;
}

/// Sends http-kind hooks. Zero live network in tests.
#[async_trait]
pub trait HttpTransport: Send + Sync {
    /// Sends one typed request.
    async fn send(&self, request: HookHttpRequest) -> Result<HookHttpResponse, HookTransportError>;
}

/// Invokes MCP-tool-kind hooks (todo 29 wires the real client).
#[async_trait]
pub trait McpToolInvoker: Send + Sync {
    /// Calls `tool` on `server` with the payload; returns the tool
    /// result text.
    async fn invoke(
        &self,
        server: &str,
        tool: &str,
        payload: &Value,
    ) -> Result<String, HookTransportError>;
}

/// The typed verdict an LLM-evaluated policy hook returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyVerdict {
    /// No objection / criterion met (gate-neutral, exit-sanctioned).
    Proceed,
    /// The goal is not met; keep iterating with `reason` (Stop events).
    ContinueWorking {
        /// Why another iteration is needed.
        reason: String,
    },
    /// Explicit gate approval (tool events).
    Allow,
    /// Explicit gate refusal (tool events).
    Deny {
        /// Why the gated call is refused.
        reason: String,
    },
}

/// Evaluates prompt-kind policy hooks against a payload. No live LLM in
/// tests — the daemon injects the session-model-backed adapter.
#[async_trait]
pub trait PromptEvaluator: Send + Sync {
    /// Judges `policy` against `payload`.
    async fn evaluate(
        &self,
        policy: &str,
        payload: &Value,
    ) -> Result<PolicyVerdict, HookTransportError>;
}
