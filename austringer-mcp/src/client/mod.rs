//! The MCP client core (todo 29): rmcp-backed connections over stdio or
//! Streamable HTTP with per-server connect timeouts, bounded in-flight
//! operations, typed unresponsive handling (never a hung await), roots
//! confinement on every call, and resources/prompts surfaces. Connection
//! establishment lives in [`connect`]; wire value types in
//! [`descriptors`].

mod connect;
mod descriptors;

pub use descriptors::{PromptDescriptor, ResourceDescriptor, ToolDescriptor};

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use rmcp::model::{CallToolRequestParams, GetPromptRequestParams, ReadResourceRequestParams};
use rmcp::service::{RoleClient, RunningService};
use serde_json::Value;
use tokio::sync::Semaphore;

use crate::config::ServerEntry;
use crate::error::McpClientError;
use crate::roots::Roots;

use connect::AustringerClientHandler;
use descriptors::{describe_tool, join_text};

/// Bounded concurrent in-flight operations per server (Droid pattern):
/// beyond this ceiling calls reject fast with a typed error instead of
/// queueing onto an unresponsive peer.
pub const IN_FLIGHT_LIMIT: usize = 8;

/// Default per-call deadline.
pub const DEFAULT_CALL_TIMEOUT: Duration = Duration::from_secs(60);

/// A connected MCP server handle.
pub struct McpClient {
    entry: ServerEntry,
    service: RunningService<RoleClient, AustringerClientHandler>,
    roots: Roots,
    permits: Semaphore,
    call_timeout_ms: AtomicU64,
    poisoned: AtomicBool,
}

impl McpClient {
    /// Overrides the per-call deadline (tests pin short budgets).
    #[must_use]
    pub fn with_call_timeout(self, timeout: Duration) -> Self {
        self.set_call_timeout(timeout);
        self
    }

    /// Re-arms the per-call deadline on a live (shared) client.
    pub fn set_call_timeout(&self, timeout: Duration) {
        let millis = timeout.as_millis().min(u128::from(u64::MAX)) as u64;
        self.call_timeout_ms.store(millis, Ordering::Relaxed);
    }

    fn call_timeout(&self) -> Duration {
        Duration::from_millis(self.call_timeout_ms.load(Ordering::Relaxed))
    }

    /// The entry this client connected through.
    #[must_use]
    pub fn entry(&self) -> &ServerEntry {
        &self.entry
    }

    /// Whether a call deadline fired and poisoned this connection (the
    /// pool uses this to drop instead of reuse).
    #[must_use]
    pub fn is_poisoned(&self) -> bool {
        self.poisoned.load(Ordering::Relaxed)
    }

    /// Lists the server's tools.
    pub async fn list_tools(&self) -> Result<Vec<ToolDescriptor>, McpClientError> {
        let tools = self
            .bounded(async {
                self.service
                    .list_all_tools()
                    .await
                    .map_err(|error| service_error(&self.entry.name, error))
            })
            .await?;
        Ok(tools.into_iter().map(describe_tool).collect())
    }

    /// Calls one tool: roots-confines the arguments, bounds concurrency,
    /// enforces the call deadline (a hang yields typed `Unresponsive` and
    /// poisons the connection), and returns the joined text content.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: &Value,
    ) -> Result<Vec<u8>, McpClientError> {
        if self.is_poisoned() {
            return Err(McpClientError::Unresponsive {
                server: self.entry.name.clone(),
                timeout_ms: self.call_timeout().as_millis().min(u128::from(u64::MAX)) as u64,
            });
        }
        self.roots.check_arguments(name, arguments)?;
        let arguments = arguments
            .as_object()
            .cloned()
            .ok_or_else(|| McpClientError::Protocol {
                server: self.entry.name.clone(),
                message: "tool arguments must be a JSON object".to_owned(),
            })?;
        let params = CallToolRequestParams::new(name.to_owned()).with_arguments(arguments);
        let permit = self.try_permit()?;
        let server = self.entry.name.clone();
        let timeout_ms = self.call_timeout().as_millis().min(u128::from(u64::MAX)) as u64;
        let call = async {
            self.service
                .call_tool(params)
                .await
                .map_err(|error| service_error(&server, error))
        };
        let outcome = tokio::time::timeout(self.call_timeout(), call).await;
        drop(permit);
        let result = outcome.map_err(|_| {
            self.poisoned.store(true, Ordering::Relaxed);
            McpClientError::Unresponsive { server, timeout_ms }
        })??;
        if result.is_error == Some(true) {
            return Err(McpClientError::Protocol {
                server: self.entry.name.clone(),
                message: format!(
                    "tool {name} reported an error: {}",
                    join_text(&result.content)
                ),
            });
        }
        Ok(join_text(&result.content).into_bytes())
    }

    /// Lists the server's resources.
    pub async fn list_resources(&self) -> Result<Vec<ResourceDescriptor>, McpClientError> {
        let resources = self
            .bounded(async {
                self.service
                    .list_all_resources()
                    .await
                    .map_err(|error| service_error(&self.entry.name, error))
            })
            .await?;
        Ok(resources
            .into_iter()
            .map(|resource| ResourceDescriptor {
                uri: resource.uri.to_string(),
                name: resource.name.clone(),
            })
            .collect())
    }

    /// Reads one resource's text content.
    pub async fn read_resource(&self, uri: &str) -> Result<Vec<u8>, McpClientError> {
        let uri_owned = uri.to_owned();
        let contents = self
            .bounded(async move {
                let result = self
                    .service
                    .read_resource(ReadResourceRequestParams::new(uri_owned))
                    .await
                    .map_err(|error| service_error(&self.entry.name, error))?;
                Ok(result.contents)
            })
            .await?;
        let mut joined = Vec::new();
        for content in contents {
            if let rmcp::model::ResourceContents::TextResourceContents { text, .. } = content {
                joined.extend_from_slice(text.as_bytes());
            }
        }
        Ok(joined)
    }

    /// Lists the server's prompts.
    pub async fn list_prompts(&self) -> Result<Vec<PromptDescriptor>, McpClientError> {
        let prompts = self
            .bounded(async {
                self.service
                    .list_all_prompts()
                    .await
                    .map_err(|error| service_error(&self.entry.name, error))
            })
            .await?;
        Ok(prompts
            .into_iter()
            .map(|prompt| PromptDescriptor {
                name: prompt.name.clone(),
                description: prompt.description.clone().unwrap_or_default(),
                arguments: prompt
                    .arguments
                    .unwrap_or_default()
                    .into_iter()
                    .map(|argument| argument.name)
                    .collect(),
            })
            .collect())
    }

    /// Gets one prompt's rendered message texts.
    pub async fn get_prompt(&self, name: &str) -> Result<Vec<String>, McpClientError> {
        let name_owned = name.to_owned();
        let messages = self
            .bounded(async move {
                let result = self
                    .service
                    .get_prompt(GetPromptRequestParams::new(name_owned))
                    .await
                    .map_err(|error| service_error(&self.entry.name, error))?;
                Ok(result.messages)
            })
            .await?;
        Ok(messages
            .into_iter()
            .map(|message| join_text(std::slice::from_ref(&message.content)))
            .collect())
    }

    /// Graceful shutdown (cancels the service and waits out the peer).
    pub async fn shutdown(self) -> Result<(), McpClientError> {
        let name = self.entry.name.clone();
        self.service
            .cancel()
            .await
            .map(|_| ())
            .map_err(|error| McpClientError::Protocol {
                server: name,
                message: error.to_string(),
            })
    }

    fn try_permit(&self) -> Result<tokio::sync::SemaphorePermit<'_>, McpClientError> {
        self.permits
            .try_acquire()
            .map_err(|_| McpClientError::InFlightLimit {
                server: self.entry.name.clone(),
                limit: IN_FLIGHT_LIMIT,
            })
    }

    /// Runs one future under the in-flight bound and the call deadline;
    /// elapsing poisons the connection (unresponsive peers never hang us).
    async fn bounded<F, T>(&self, future: F) -> Result<T, McpClientError>
    where
        F: std::future::Future<Output = Result<T, McpClientError>>,
    {
        if self.is_poisoned() {
            return Err(McpClientError::Unresponsive {
                server: self.entry.name.clone(),
                timeout_ms: self.call_timeout().as_millis().min(u128::from(u64::MAX)) as u64,
            });
        }
        let permit = self.try_permit()?;
        let outcome = tokio::time::timeout(self.call_timeout(), future).await;
        drop(permit);
        outcome.map_err(|_| {
            self.poisoned.store(true, Ordering::Relaxed);
            McpClientError::Unresponsive {
                server: self.entry.name.clone(),
                timeout_ms: self.call_timeout().as_millis().min(u128::from(u64::MAX)) as u64,
            }
        })?
    }
}

fn service_error(server: &str, error: rmcp::ServiceError) -> McpClientError {
    McpClientError::Protocol {
        server: server.to_owned(),
        message: error.to_string(),
    }
}
