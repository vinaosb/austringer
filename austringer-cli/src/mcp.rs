//! `austringer mcp` (plan todo 7): the MCP subcommand family over
//! austringer-mcp. `list`/`scan` render the config scan (parse-only,
//! zero connections), `serve` runs the austringer-as-MCP-server over
//! stdio, and `call` smoke-tests one configured server by name.
//! Nothing here auto-enables or starts a server: the only spawn is
//! the one `call`/`serve` explicitly names.
//!
//! Standalone seams: without a daemon there is no operator to ask and
//! no session store to drive, so `serve` answers initialize/list-tools
//! honestly while every stateful tool fails with a typed refusal
//! naming what is missing.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use austringer_mcp::client::McpClient;
use austringer_mcp::config::{ProviderKind, TransportSpec};
use austringer_mcp::manager::{self, AutostartDecision, Installation, ScanReport};
use austringer_mcp::server::{AskSeam, AustringerMcpServer, StatusSeam};
use austringer_mcp::session_surface::{
    CreateRequest, EventChunk, GatherRegistry, InteractRequest, InteractionOutcome, SessionControl,
    SessionId, SessionSummary, SurfaceError,
};

use crate::cli::{McpCallArgs, McpCommand, McpListArgs};
use crate::error::CliError;

/// Dispatch one `mcp` subcommand; returns the exit code.
///
/// # Errors
/// typed [`CliError::Mcp`] on scan diagnostics, unknown servers,
/// bad `--args` JSON, or any typed transport failure from the client.
pub async fn run(command: &McpCommand) -> Result<i32, CliError> {
    match command {
        McpCommand::List(args) => list(args),
        McpCommand::Serve => serve().await,
        McpCommand::Call(args) => call(args).await,
    }
}

/// Scan roots: the `--dir` overrides, or the working directory.
fn scan_roots(dirs: &[PathBuf]) -> Vec<PathBuf> {
    if dirs.is_empty() {
        vec![std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))]
    } else {
        dirs.to_vec()
    }
}

/// The first scan diagnostic as the typed failure (file + offending
/// key both come from the manager's bounded message).
fn first_diagnostic(report: &ScanReport) -> Option<CliError> {
    report.diagnostics.first().map(|diagnostic| CliError::Mcp {
        detail: format!("{}: {}", diagnostic.path.display(), diagnostic.message),
    })
}

/// `mcp list` (alias `scan`): render every discovered server entry.
fn list(args: &McpListArgs) -> Result<i32, CliError> {
    let report = manager::scan_directories(&scan_roots(&args.dirs));
    if let Some(error) = first_diagnostic(&report) {
        return Err(error);
    }
    if report.installations.is_empty() {
        println!("no MCP servers configured");
        return Ok(0);
    }
    for installation in &report.installations {
        println!("{}", render_installation(installation));
    }
    Ok(0)
}

/// One human-readable entry line (doctor's plain-line style).
fn render_installation(installation: &Installation) -> String {
    let entry = &installation.entry;
    let transport = match &entry.transport {
        TransportSpec::Stdio { command, args } => {
            if args.is_empty() {
                format!("stdio:{command}")
            } else {
                format!("stdio:{command} {}", args.join(" "))
            }
        }
        TransportSpec::Http { url } => format!("http:{url}"),
    };
    // The manager's autostart policy is the honest state surface;
    // ServerEntry carries no enabled/disabled field, so none is
    // invented here.
    let start = match ScanReport::autostart_for(entry) {
        AutostartDecision::StartNow => "start:autostart",
        AutostartDecision::DeferredOnFirstUse => "start:on-first-use",
    };
    format!(
        "{:<20} {:<11} {start}  {transport}",
        entry.name,
        provider_label(installation.provider),
    )
}

fn provider_label(provider: ProviderKind) -> &'static str {
    match provider {
        ProviderKind::InProcess => "in-process",
        ProviderKind::Austringer => "austringer",
        ProviderKind::Claude => "claude",
        ProviderKind::Cursor => "cursor",
        ProviderKind::Codeium => "codeium",
    }
}

/// `mcp serve`: run the austringer MCP server over stdio until the peer
/// disconnects. No daemon is contacted; stateful tools answer with
/// the typed refusals below.
async fn serve() -> Result<i32, CliError> {
    eprintln!("austringer mcp: serving on stdio (standalone; no daemon attached)");
    let server = AustringerMcpServer::new(
        Arc::new(StandaloneAsk),
        Arc::new(StandaloneStatus),
        Arc::new(StandaloneSessions),
        Arc::new(GatherRegistry::new(16)),
    );
    let service = server
        .serve_over((tokio::io::stdin(), tokio::io::stdout()))
        .await
        .map_err(|error| CliError::Mcp {
            detail: format!("serve failed to initialize: {error}"),
        })?;
    let _ = service.waiting().await;
    Ok(0)
}

struct StandaloneAsk;

#[async_trait]
impl AskSeam for StandaloneAsk {
    async fn ask(&self, _question: &str) -> Result<String, SurfaceError> {
        Err(SurfaceError::Engine(
            "austringer_ask needs a daemon operator; `austringer mcp serve` runs standalone"
                .to_owned(),
        ))
    }
}

struct StandaloneStatus;

impl StatusSeam for StandaloneStatus {
    fn status(&self) -> String {
        "austringer mcp serve: standalone (no daemon attached)".to_owned()
    }
}

struct StandaloneSessions;

#[async_trait]
impl SessionControl for StandaloneSessions {
    async fn create(&self, _request: CreateRequest) -> Result<SessionSummary, SurfaceError> {
        Err(session_refusal("session_create"))
    }

    async fn search(&self, _query: &str) -> Result<Vec<SessionSummary>, SurfaceError> {
        Err(session_refusal("session_search"))
    }

    async fn interact(
        &self,
        _request: InteractRequest,
    ) -> Result<InteractionOutcome, SurfaceError> {
        Err(session_refusal("session_interact"))
    }

    async fn events(
        &self,
        _session: &SessionId,
        _since: u64,
    ) -> Result<Vec<EventChunk>, SurfaceError> {
        Err(session_refusal("session_events"))
    }
}

fn session_refusal(tool: &str) -> SurfaceError {
    SurfaceError::Engine(format!(
        "{tool} needs the daemon session surface; `austringer mcp serve` runs standalone"
    ))
}

/// `mcp call <server>`: scan, connect to the named server (the
/// explicit spawn this command exists for), then list tools or call
/// one. A full RPC console is out of scope.
async fn call(args: &McpCallArgs) -> Result<i32, CliError> {
    let report = manager::scan_directories(&scan_roots(&args.dirs));
    if let Some(error) = first_diagnostic(&report) {
        return Err(error);
    }
    let Some(installation) = report
        .installations
        .iter()
        .find(|installation| installation.entry.name == args.server)
    else {
        let known: Vec<&str> = report
            .installations
            .iter()
            .map(|installation| installation.entry.name.as_str())
            .collect();
        return Err(CliError::Mcp {
            detail: format!(
                "server {:?} not found in scanned configs (known: {})",
                args.server,
                if known.is_empty() {
                    "none".to_owned()
                } else {
                    known.join(", ")
                }
            ),
        });
    };
    let client = McpClient::connect(installation.entry.clone())
        .await
        .map_err(|error| CliError::Mcp {
            detail: format!("call {}: {error}", args.server),
        })?;
    let outcome = match &args.tool {
        None => list_tools(&client).await,
        Some(tool) => call_tool(&client, tool, args.args.as_deref()).await,
    };
    let rendered = outcome.map_err(|error| CliError::Mcp {
        detail: format!("call {}: {error}", args.server),
    })?;
    println!("{rendered}");
    let _ = client.shutdown().await;
    Ok(0)
}

async fn list_tools(client: &McpClient) -> Result<String, austringer_mcp::error::McpClientError> {
    let tools = client.list_tools().await?;
    Ok(tools
        .iter()
        .map(|tool| format!("{}: {}", tool.name, tool.description))
        .collect::<Vec<_>>()
        .join("\n"))
}

async fn call_tool(
    client: &McpClient,
    tool: &str,
    arguments: Option<&str>,
) -> Result<String, austringer_mcp::error::McpClientError> {
    let arguments = match arguments {
        None => serde_json::json!({}),
        Some(raw) => serde_json::from_str(raw).map_err(|error| {
            austringer_mcp::error::McpClientError::Protocol {
                server: client.entry().name.clone(),
                message: format!("--args is not valid JSON: {error}"),
            }
        })?,
    };
    let result = client.call_tool(tool, &arguments).await?;
    Ok(String::from_utf8_lossy(&result).into_owned())
}
