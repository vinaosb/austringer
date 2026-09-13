//! The harness AS an MCP server (todo 29): a transport-agnostic
//! `ServerHandler` core exposing ask/status/session-control tools. The
//! daemon wires the transport (stdio via `transport-io`, Streamable HTTP
//! via `transport-streamable-http-server`) later; every tool is typed
//! and routed through seams so the surface is testable in-process.
//!
//! Untrusted-text rule: seam replies are DATA — serialized verbatim into
//! tool results, never evaluated, never interpreted as instructions.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rmcp::ErrorData as McpError;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, InitializeResult, ServerCapabilities};
use rmcp::schemars;
use rmcp::service::RoleServer;
use rmcp::service::serve_server;
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use serde::Deserialize;
use serde_json::json;

use crate::session_surface::{
    CreateRequest, GatherOutcome, GatherRegistry, InteractRequest, SessionControl, SessionId,
    SurfaceError,
};

/// The ask seam: submit one question, get one answer (the daemon routes
/// it to the operator or the engine's ask flow).
#[async_trait]
pub trait AskSeam: Send + Sync {
    /// Answers one question.
    async fn ask(&self, question: &str) -> Result<String, SurfaceError>;
}

/// The status seam: one bounded status line.
pub trait StatusSeam: Send + Sync {
    /// Current harness status.
    fn status(&self) -> String;
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AskArgs {
    pub question: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateArgs {
    pub title: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchArgs {
    pub query: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InteractArgs {
    pub session_id: String,
    pub prompt: String,
    /// Hard cost ceiling in micro-USD for this one interaction.
    pub cost_limit_micro_usd: Option<u128>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventsArgs {
    pub session_id: String,
    pub since: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GatherArgs {
    pub session_ids: Vec<String>,
    pub timeout_ms: u64,
}

/// The harness MCP server core.
pub struct HarnessMcpServer {
    ask: Arc<dyn AskSeam>,
    status: Arc<dyn StatusSeam>,
    sessions: Arc<dyn SessionControl>,
    gather: Arc<GatherRegistry>,
    #[allow(dead_code, reason = "accessed by generated tool dispatch")]
    tool_router: ToolRouter<Self>,
}

impl HarnessMcpServer {
    /// Assembles the core over its seams.
    #[must_use]
    pub fn new(
        ask: Arc<dyn AskSeam>,
        status: Arc<dyn StatusSeam>,
        sessions: Arc<dyn SessionControl>,
        gather: Arc<GatherRegistry>,
    ) -> Self {
        Self {
            ask,
            status,
            sessions,
            gather,
            tool_router: Self::tool_router(),
        }
    }

    /// Convenience wiring: serves this core over an arbitrary transport
    /// (the daemon's stdio/HTTP entry point).
    // rmcp's ServerInitializeError is 368 bytes upstream; rmcp's own
    // examples carry this same allow.
    #[allow(clippy::result_large_err, reason = "upstream error type size")]
    pub async fn serve_over<T, E, A>(
        self,
        transport: T,
    ) -> Result<rmcp::service::RunningService<RoleServer, Self>, rmcp::service::ServerInitializeError>
    where
        T: rmcp::transport::IntoTransport<RoleServer, E, A>,
        E: std::error::Error + Send + Sync + 'static,
    {
        serve_server(self, transport).await
    }
}

fn surface_error(error: SurfaceError) -> McpError {
    McpError::internal_error(error.to_string(), None)
}

fn json_result(value: serde_json::Value) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![ContentBlock::text(
        value.to_string(),
    )]))
}

#[tool_router]
impl HarnessMcpServer {
    #[tool(description = "Ask the harness operator a question; returns the answer text")]
    async fn harness_ask(
        &self,
        Parameters(args): Parameters<AskArgs>,
    ) -> Result<CallToolResult, McpError> {
        let answer = self.ask.ask(&args.question).await.map_err(surface_error)?;
        json_result(json!(answer))
    }

    #[tool(description = "One-line harness status")]
    fn harness_status(&self) -> Result<CallToolResult, McpError> {
        json_result(json!(self.status.status()))
    }

    #[tool(description = "Create a harness session; returns its summary")]
    async fn session_create(
        &self,
        Parameters(args): Parameters<CreateArgs>,
    ) -> Result<CallToolResult, McpError> {
        let summary = self
            .sessions
            .create(CreateRequest { title: args.title })
            .await
            .map_err(surface_error)?;
        json_result(serde_json::to_value(summary).map_err(|error| {
            McpError::internal_error(format!("summary serialization failed: {error}"), None)
        })?)
    }

    #[tool(description = "Search harness sessions by text")]
    async fn session_search(
        &self,
        Parameters(args): Parameters<SearchArgs>,
    ) -> Result<CallToolResult, McpError> {
        let found = self
            .sessions
            .search(&args.query)
            .await
            .map_err(surface_error)?;
        json_result(serde_json::to_value(found).map_err(|error| {
            McpError::internal_error(format!("search serialization failed: {error}"), None)
        })?)
    }

    #[tool(
        description = "Send one prompt to a session under an optional hard cost limit (micro-USD)"
    )]
    async fn session_interact(
        &self,
        Parameters(args): Parameters<InteractArgs>,
    ) -> Result<CallToolResult, McpError> {
        let outcome = self
            .sessions
            .interact(InteractRequest {
                session: SessionId(args.session_id),
                prompt: args.prompt,
                cost_limit: args
                    .cost_limit_micro_usd
                    .map(harness_core::budget::MicroUsd),
            })
            .await
            .map_err(surface_error)?;
        json_result(serde_json::to_value(outcome).map_err(|error| {
            McpError::internal_error(format!("outcome serialization failed: {error}"), None)
        })?)
    }

    #[tool(description = "Read bounded session event chunks strictly after an ordinal")]
    async fn session_events(
        &self,
        Parameters(args): Parameters<EventsArgs>,
    ) -> Result<CallToolResult, McpError> {
        let chunks = self
            .sessions
            .events(&SessionId(args.session_id), args.since)
            .await
            .map_err(surface_error)?;
        json_result(serde_json::to_value(chunks).map_err(|error| {
            McpError::internal_error(format!("events serialization failed: {error}"), None)
        })?)
    }

    #[tool(description = "GATHER fan-in: await N session completions within a bounded timeout")]
    async fn session_gather(
        &self,
        Parameters(args): Parameters<GatherArgs>,
    ) -> Result<CallToolResult, McpError> {
        let sessions: Vec<SessionId> = args.session_ids.into_iter().map(SessionId).collect();
        let expected = sessions.len();
        let barrier = self.gather.try_barrier(sessions).map_err(surface_error)?;
        let outcome = barrier.wait(Duration::from_millis(args.timeout_ms)).await;
        let payload = match outcome {
            GatherOutcome::AllCompleted {
                completed,
                timed_out,
            }
            | GatherOutcome::Partial {
                completed,
                timed_out,
            } => {
                json!({ "completed": completed, "timedOut": timed_out, "expected": expected })
            }
        };
        json_result(payload)
    }
}

#[tool_handler]
impl ServerHandler for HarnessMcpServer {
    fn get_info(&self) -> InitializeResult {
        InitializeResult::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(rmcp::model::Implementation::new(
                "harness",
                env!("CARGO_PKG_VERSION"),
            ))
    }
}
