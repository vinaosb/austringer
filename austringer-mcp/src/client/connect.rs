//! Client-side connection establishment (todo 29): the ClientHandler
//! answering roots negotiation, and the stdio / Streamable-HTTP /
//! caller-supplied-transport connect paths, all bounded by the entry's
//! connect timeout.

use std::sync::atomic::{AtomicBool, AtomicU64};
use std::time::Duration;

use rmcp::ClientHandler;
use rmcp::model::{ClientCapabilities, Implementation, InitializeRequestParams};
use rmcp::service::{RoleClient, RunningService, serve_client};
use rmcp::transport::child_process::TokioChildProcess;
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use tokio::process::Command;

use super::{DEFAULT_CALL_TIMEOUT, IN_FLIGHT_LIMIT, McpClient};
use crate::config::{ServerEntry, TransportSpec};
use crate::error::McpClientError;
use crate::http_backend::ReqwestSseClient;
use crate::roots::Roots;
use crate::stderr_bridge::spawn_stderr_drain;
use tokio::sync::Semaphore;

/// Base delay before the first Streamable HTTP connect retry (milliseconds).
const CONNECT_RETRY_BASE_MS: u64 = 20;

/// Ceiling for the exponential Streamable HTTP connect-retry delay
/// (milliseconds).
const CONNECT_RETRY_CAP_MS: u64 = 500;

/// Delay before the next Streamable HTTP connect attempt after `attempt`
/// failures (0-based): 20 ms doubling per attempt, capped at 500 ms. A
/// dead endpoint sees a handful of dials inside the connect budget
/// instead of one every 20 ms; the outer deadline still yields the typed
/// [`McpClientError::ConnectTimeout`] on every OS.
#[must_use]
pub fn connect_retry_delay(attempt: u32) -> Duration {
    // 20 << 5 = 640 ms already exceeds the cap; later attempts saturate
    // there, which also keeps the shift overflow-free for any `attempt`.
    let shift = attempt.min(5);
    Duration::from_millis((CONNECT_RETRY_BASE_MS << shift).min(CONNECT_RETRY_CAP_MS))
}

pub(super) struct AustringerClientHandler {
    roots: Vec<String>,
}

impl ClientHandler for AustringerClientHandler {
    fn get_info(&self) -> InitializeRequestParams {
        InitializeRequestParams::new(
            ClientCapabilities::default(),
            Implementation::from_build_env(),
        )
    }

    // Roots negotiation is SEP-2577-deprecated upstream but still the
    // wire path compliant servers use to ask which boundaries they may
    // touch; we answer it deliberately while the enforced confinement
    // stays client-side (Roots::check_arguments).
    #[allow(deprecated, reason = "roots remain in the protocol surface")]
    async fn list_roots(
        &self,
        _context: rmcp::service::RequestContext<RoleClient>,
    ) -> Result<rmcp::model::ListRootsResult, rmcp::ErrorData> {
        let roots = self
            .roots
            .iter()
            .map(|path| {
                rmcp::model::Root::new(format!("file://{}", path.replace('\\', "/")))
                    .with_name(path.as_str())
            })
            .collect();
        Ok(rmcp::model::ListRootsResult::new(roots))
    }
}

impl McpClient {
    /// Connects (stdio spawn or Streamable HTTP) under the entry's connect
    /// timeout; the initialize handshake completes inside `connect` or the
    /// whole future fails typed. Server stderr (stdio only) drains to the
    /// `austringer_mcp::server_stderr` tracing target.
    pub async fn connect(entry: ServerEntry) -> Result<Self, McpClientError> {
        entry
            .validate()
            .map_err(|message| McpClientError::Protocol {
                server: entry.name.clone(),
                message,
            })?;
        let handler = Self::handler_for(&entry);
        let connect_timeout = entry.connect_timeout();
        let connect = async {
            let service = match &entry.transport {
                TransportSpec::Stdio { command, args } => {
                    let mut child = Command::new(command);
                    child.args(args).envs(&entry.env);
                    let builder = TokioChildProcess::builder(child);
                    let spawn = builder
                        .stderr(std::process::Stdio::piped())
                        .spawn()
                        .map_err(|error| McpClientError::Protocol {
                            server: entry.name.clone(),
                            message: format!("spawn failed: {error}"),
                        })?;
                    if let Some(stderr) = spawn.1 {
                        spawn_stderr_drain(stderr, &entry.name);
                    }
                    serve_client(handler, spawn.0).await
                }
                TransportSpec::Http { url } => {
                    // Dead endpoints can fail faster than the connect
                    // budget on RST-fast platforms (ECONNREFUSED < 1 ms
                    // on linux vs firewall-dropped SYNs on windows);
                    // re-attempt until the budget is spent so an
                    // unestablished endpoint surfaces as the typed
                    // ConnectTimeout on every OS. The HTTP client (rustls
                    // provider init + root store parse) is built once
                    // before the loop and cloned per attempt (a cheap
                    // Arc bump), and retries back off exponentially so a
                    // dead endpoint is dialed a handful of times per
                    // budget, not every 20 ms.
                    let sse_client = ReqwestSseClient::new();
                    let mut attempt = 0_u32;
                    loop {
                        let transport = StreamableHttpClientTransport::with_client(
                            sse_client.clone(),
                            StreamableHttpClientTransportConfig::with_uri(url.clone()),
                        );
                        // serve_client consumes the handler; rebuild it
                        // per attempt.
                        match serve_client(Self::handler_for(&entry), transport).await {
                            Ok(service) => break Ok(service),
                            Err(_) => {
                                tokio::time::sleep(connect_retry_delay(attempt)).await;
                                attempt += 1;
                            }
                        }
                    }
                }
            };
            service.map_err(|error| McpClientError::Protocol {
                server: entry.name.clone(),
                message: error.to_string(),
            })
        };
        let service = tokio::time::timeout(connect_timeout, connect)
            .await
            .map_err(|_| McpClientError::ConnectTimeout {
                server: entry.name.clone(),
                timeout_ms: entry.connect_timeout_ms,
            })??;
        Ok(Self::from_service(entry, service))
    }

    /// Connects over a caller-supplied transport (in-process interop and
    /// daemon embedding); the initialize handshake still runs under the
    /// entry's connect timeout.
    pub async fn connect_with_transport<T, E, A>(
        entry: ServerEntry,
        transport: T,
    ) -> Result<Self, McpClientError>
    where
        T: rmcp::transport::IntoTransport<RoleClient, E, A>,
        E: std::error::Error + Send + Sync + 'static,
    {
        entry
            .validate()
            .map_err(|message| McpClientError::Protocol {
                server: entry.name.clone(),
                message,
            })?;
        let connect = async {
            serve_client(Self::handler_for(&entry), transport)
                .await
                .map_err(|error| McpClientError::Protocol {
                    server: entry.name.clone(),
                    message: error.to_string(),
                })
        };
        let service = tokio::time::timeout(entry.connect_timeout(), connect)
            .await
            .map_err(|_| McpClientError::ConnectTimeout {
                server: entry.name.clone(),
                timeout_ms: entry.connect_timeout_ms,
            })??;
        Ok(Self::from_service(entry, service))
    }

    fn handler_for(entry: &ServerEntry) -> AustringerClientHandler {
        AustringerClientHandler {
            roots: entry
                .roots
                .iter()
                .map(|root| root.to_string_lossy().into_owned())
                .collect(),
        }
    }

    pub(super) fn from_service(
        entry: ServerEntry,
        service: RunningService<RoleClient, AustringerClientHandler>,
    ) -> Self {
        let roots = Roots::from_entry(&entry);
        Self {
            roots,
            entry,
            service,
            permits: Semaphore::new(IN_FLIGHT_LIMIT),
            call_timeout_ms: AtomicU64::new(
                DEFAULT_CALL_TIMEOUT.as_millis().min(u128::from(u64::MAX)) as u64,
            ),
            poisoned: AtomicBool::new(false),
        }
    }
}
