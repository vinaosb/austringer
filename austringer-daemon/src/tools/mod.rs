//! In-process CORE_TOOLS execution (todo 3, ecosystem alignment): the
//! executor bundle constructed once at daemon startup, the per-session
//! registry it binds into, and the loop-stage adapter that dispatches
//! approved calls through that registry. `tool_search` stays
//! registry-internal and `task_completed` is loop-intercepted, so with
//! the four providers in [`providers`] bound all six core tools are
//! production-served. The worker protocol (out-of-process execution)
//! remains the default executor for un-injected states; unbound tools
//! keep their typed NoExecutor dispatch error.

pub(crate) mod providers;
pub mod run_tests;
mod safety;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;

use austringer_core::agent_loop::inference::PendingCall;
use austringer_core::agent_loop::stages::{StageCtx, StageError, ToolExecutor, ToolOutcome};
use austringer_core::tools::PinnedToolSet;
use austringer_core::tools::{IoCap, ToolProvider, ToolRegistry};

use providers::{EditLinesTool, ExecuteCommandTool, LspQueryTool, ViewWindowTool};

use crate::protocol::SessionEvent;

/// Builds one session's tool-execution stage (the daemon's injectable
/// executor seam; see [`crate::server::DaemonState::executor_factory`]).
pub type ExecutorFactory = Arc<
    dyn Fn(&Path, tokio::sync::mpsc::Sender<SessionEvent>) -> Box<dyn ToolExecutor> + Send + Sync,
>;

/// The in-process executor set, constructed once at daemon startup and
/// shared across every session's registry (one PTY runner, one LSP
/// router per daemon — language servers spawn lazily on first use).
pub struct CoreExecutors {
    view_window: Arc<dyn ToolProvider>,
    edit_lines: Arc<dyn ToolProvider>,
    execute_command: Arc<dyn ToolProvider>,
    lsp: LspQueryTool,
    run_tests: crate::tools::run_tests::RunTestsTool,
    /// The delegation services (todo 13); set once by the binary AFTER
    /// DaemonState exists — sessions open later, so every registry sees
    /// the bound services. Unbound = the `task` tool is absent.
    task: std::sync::OnceLock<Arc<crate::task::TaskServices>>,
}

impl CoreExecutors {
    /// The production in-process set. The LSP workspace root is the
    /// daemon's working directory.
    #[must_use]
    pub fn in_process() -> Self {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            view_window: Arc::new(ViewWindowTool),
            edit_lines: Arc::new(EditLinesTool),
            execute_command: Arc::new(ExecuteCommandTool {
                runner: Arc::new(austringer_exec::PtyRunner::new()),
            }),
            lsp: LspQueryTool {
                router: austringer_lsp::LspRouter::new(austringer_lsp::LspConfig::new(
                    root.clone(),
                )),
                root: root.clone(),
            },
            run_tests: crate::tools::run_tests::RunTestsTool::new(root, None),
            task: std::sync::OnceLock::new(),
        }
    }

    /// Sets the user-global test-command override (todo 17; read from
    /// `austringer_TEST_COMMAND` by the binary — never a repo-local file).
    #[must_use]
    pub fn with_test_command(mut self, command: Option<String>) -> Self {
        let root = self.lsp.root.clone();
        self.run_tests = crate::tools::run_tests::RunTestsTool::new(root, command);
        self
    }

    /// Binds the task-delegation services; every registry opened
    /// afterwards serves the `task` tool.
    pub fn bind_task_services(&self, services: Arc<crate::task::TaskServices>) {
        let _ = self.task.set(services);
    }

    /// Starts the RSS-sampled LSP server guard (must run inside a
    /// tokio runtime; no-op until a server is live).
    pub fn start_lsp_guard(&self) {
        self.lsp
            .router
            .start_guard(Arc::new(austringer_lsp::guard::SysinfoProbe::new()));
    }

    /// Opens the session's registry with the four in-process executors
    /// bound to their day-one contracts, plus the model-facing
    /// `search_sessions` tool (todo 11) and `run_tests` tool (todo 17:
    /// framework detection + typed TestReports; approval-tier parity
    /// with execute_command).
    ///
    /// # Errors
    /// spill-store creation failure for the session directory.
    pub fn open_registry(&self, session_dir: &Path) -> io::Result<ToolRegistry> {
        let mut registry = ToolRegistry::open(session_dir, IoCap::DEFAULT)?;
        registry
            .bind_executor("view_window", Arc::clone(&self.view_window))
            .map_err(|error| io::Error::other(error.to_string()))?;
        registry
            .bind_executor("edit_lines", Arc::clone(&self.edit_lines))
            .map_err(|error| io::Error::other(error.to_string()))?;
        registry
            .bind_executor("execute_command", Arc::clone(&self.execute_command))
            .map_err(|error| io::Error::other(error.to_string()))?;
        registry
            .bind_executor("lsp_query", Arc::new(self.lsp.clone()))
            .map_err(|error| io::Error::other(error.to_string()))?;
        registry
            .register(Arc::new(crate::search::SearchSessionsTool::new(
                session_dir,
            )))
            .map_err(|error| io::Error::other(error.to_string()))?;
        registry
            .register(Arc::new(self.run_tests.clone()))
            .map_err(|error| io::Error::other(error.to_string()))?;
        if let Some(services) = self.task.get() {
            registry
                .register(Arc::new(crate::task::TaskTool::new(
                    session_dir,
                    Arc::clone(services),
                )))
                .map_err(|error| io::Error::other(error.to_string()))?;
        }
        Ok(registry)
    }

    /// The injectable executor factory: each session's stage opens a
    /// registry over its own directory. A registry that cannot open
    /// degrades loudly to the no-backend stage (tool results say so)
    /// rather than failing the session.
    #[must_use]
    pub fn executor_factory(self: &Arc<Self>) -> ExecutorFactory {
        let executors = Arc::clone(self);
        Arc::new(
            move |session_dir, _event_sink| match executors.open_registry(session_dir) {
                Ok(registry) => Box::new(RegistryToolExecutor::open(registry)),
                Err(error) => {
                    eprintln!(
                        "austringer-daemon: tool registry for {} unavailable: {error}",
                        session_dir.display()
                    );
                    Box::new(austringer_core::agent_loop::stages::defaults::NoToolExecutor)
                }
            },
        )
    }
}

/// The loop's ToolExecutor stage over one session's pinned registry.
/// Dispatch failures (unknown tool, no executor, invalid arguments,
/// execution failure) become tool-result text — the session continues
/// and the model sees the typed error (split-brain preserve).
pub struct RegistryToolExecutor {
    registry: ToolRegistry,
    pinned: PinnedToolSet,
}

impl RegistryToolExecutor {
    /// Wraps a registry, pinning its current version set.
    #[must_use]
    pub fn open(registry: ToolRegistry) -> Self {
        Self {
            pinned: registry.pin_current(),
            registry,
        }
    }

    /// Dispatches one call and renders its result text (usable without
    /// a loop context).
    pub async fn execute_call(&self, call: PendingCall) -> String {
        match self
            .registry
            .dispatch(&self.pinned, &call.tool, &call.arguments)
            .await
        {
            Ok(result) => result.observation,
            Err(error) => format!("(tool dispatch: {error})"),
        }
    }
}

#[async_trait]
impl ToolExecutor for RegistryToolExecutor {
    async fn execute(
        &mut self,
        _ctx: &mut StageCtx<'_>,
        call: PendingCall,
    ) -> Result<ToolOutcome, StageError> {
        Ok(ToolOutcome::text(self.execute_call(call).await))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending(tool: &str, arguments: serde_json::Value) -> PendingCall {
        PendingCall {
            call_id: "c1".into(),
            tool: tool.into(),
            arguments,
        }
    }

    #[tokio::test]
    async fn executor_bound_no_executor_path_preserved_for_unbound_tools() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Schema-only registry: the shape an embedder gets without
        // binding — the core contracts exist, no executor answers.
        let executor = RegistryToolExecutor::open(
            ToolRegistry::open(dir.path(), IoCap::DEFAULT).expect("registry"),
        );
        let text = executor
            .execute_call(pending("view_window", serde_json::json!({"path": "x"})))
            .await;
        assert!(
            text.contains("no executor registered for tool \"view_window\""),
            "NoExecutor must stay the typed answer for unbound tools: {text}"
        );
    }

    #[tokio::test]
    async fn executor_bound_registry_opens_with_all_four_bound() {
        let dir = tempfile::tempdir().expect("tempdir");
        let executors = CoreExecutors::in_process();
        let registry = executors.open_registry(dir.path()).expect("registry");
        let executor = RegistryToolExecutor::open(registry);
        // All four bound tools answer (a missing-file view is a typed
        // execution failure, NOT a NoExecutor miss).
        let text = executor
            .execute_call(pending(
                "view_window",
                serde_json::json!({"path": "does-not-exist.txt"}),
            ))
            .await;
        assert!(
            !text.contains("no executor registered"),
            "view_window must be bound: {text}"
        );
    }
}
