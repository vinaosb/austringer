//! The in-repo REFERENCE-SHAPED MCP server (todo 29 interop
//! counterpart): a minimal filesystem server built on rmcp's own server
//! side. Golden interop tests drive OUR client against it in-process
//! (duplex) and over a real stdio child (`mcp-reference-server` bin) —
//! client and server both ours via rmcp = genuine protocol-level interop
//! (disclosed in the evidence; no external reference server binary is
//! required at test time).

use std::path::{Path, PathBuf};

use rmcp::ErrorData as McpError;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, GetPromptResult, InitializeResult, ListResourcesResult,
    PaginatedRequestParams, PromptMessage, ReadResourceRequestParams, ReadResourceResponse,
    ReadResourceResult, Resource, ResourceContents, Role, ServerCapabilities,
};
use rmcp::service::RequestContext;
use rmcp::service::RoleServer;
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReadFileArgs {
    pub path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EchoArgs {
    pub message: String,
}

/// A reference-shaped filesystem MCP server: tools `read_file` / `echo` /
/// `hang`, one resource, one prompt. `read_file` confines to its root
/// server-side (the client's roots gate is the other half of the
/// confinement story).
pub struct ReferenceFsServer {
    root: PathBuf,
    // Read by the #[tool_handler]-generated dispatch (macro-generated
    // access the dead_code lint cannot see; rmcp's own examples carry the
    // same allow).
    #[allow(dead_code, reason = "accessed by generated tool dispatch")]
    tool_router: ToolRouter<Self>,
}

impl ReferenceFsServer {
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        // Canonicalize once: Windows canonicalize() returns \\?\-prefixed
        // paths and the server-side confinement compares like-for-like.
        let root = std::fs::canonicalize(&root).unwrap_or(root);
        Self {
            root,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl ReferenceFsServer {
    #[tool(description = "Read one text file under the server root")]
    async fn read_file(
        &self,
        Parameters(args): Parameters<ReadFileArgs>,
    ) -> Result<CallToolResult, McpError> {
        eprintln!("reference-fs: read_file {}", args.path);
        let candidate = self.root.join(&args.path);
        let confined = std::fs::canonicalize(&candidate)
            .map(|canonical| canonical.starts_with(&self.root))
            .unwrap_or(false);
        if !confined {
            return Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "path outside server root: {}",
                args.path
            ))]));
        }
        match std::fs::read_to_string(&candidate) {
            Ok(text) => Ok(CallToolResult::success(vec![ContentBlock::text(text)])),
            Err(error) => Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "read failed: {error}"
            ))])),
        }
    }

    #[tool(description = "Echo a message back")]
    fn echo(&self, Parameters(args): Parameters<EchoArgs>) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            args.message,
        )]))
    }

    #[tool(description = "Never replies within any sane deadline")]
    async fn hang(&self) -> Result<CallToolResult, McpError> {
        tokio::time::sleep(std::time::Duration::from_secs(120)).await;
        Ok(CallToolResult::success(vec![ContentBlock::text(
            "finally awake",
        )]))
    }
}

#[tool_handler]
impl ServerHandler for ReferenceFsServer {
    fn get_info(&self) -> InitializeResult {
        InitializeResult::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
        .with_server_info(rmcp::model::Implementation::new(
            "reference-fs",
            env!("CARGO_PKG_VERSION"),
        ))
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::ListPromptsResult, McpError> {
        Ok(rmcp::model::ListPromptsResult {
            prompts: vec![rmcp::model::Prompt::new(
                "greet",
                Some::<String>("Greet someone by name (reference prompt)".to_owned()),
                None,
            )],
            ..Default::default()
        })
    }

    async fn get_prompt(
        &self,
        request: rmcp::model::GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::GetPromptResponse, McpError> {
        if request.name != "greet" {
            return Err(McpError::invalid_params("unknown prompt", None));
        }
        Ok(GetPromptResult::new(vec![PromptMessage::new_text(
            Role::Assistant,
            "hello from the reference prompt",
        )])
        .into())
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            resources: vec![Resource::new("fixture://notes", "notes")],
            ..Default::default()
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        if request.uri.as_str() != "fixture://notes" {
            return Err(McpError::resource_not_found("no such resource", None));
        }
        Ok(ReadResourceResult::new(vec![ResourceContents::text(
            "fixture resource body",
            request.uri.clone(),
        )])
        .into())
    }
}

/// Writes one fixture file under a temp root (test helper).
pub fn write_fixture(root: &Path, name: &str, contents: &str) -> PathBuf {
    let path = root.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&path, contents).ok();
    path
}

/// Serves one in-process server on a tokio duplex and returns the CLIENT
/// end of the wire plus a flag set once the serve task finished (the
/// wire closed and the server side dropped — the lifecycle observable
/// for embedding crates' tests, which do not depend on rmcp directly).
pub async fn serve_duplex<S: ServerHandler + 'static>(
    server: S,
) -> (
    tokio::io::DuplexStream,
    std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    use std::sync::atomic::{AtomicBool, Ordering};
    let finished = std::sync::Arc::new(AtomicBool::new(false));
    let flag = std::sync::Arc::clone(&finished);
    let (client_end, server_end) = tokio::io::duplex(8192);
    tokio::spawn(async move {
        if let Ok(service) = rmcp::service::serve_server(server, server_end).await {
            let _ = service.waiting().await;
        }
        flag.store(true, Ordering::SeqCst);
    });
    (client_end, finished)
}
