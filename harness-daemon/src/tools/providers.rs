//! The four in-process CORE_TOOLS executor implementations behind the
//! registry contracts (todo 3, ecosystem alignment). The behaviors live
//! in their owning crates — `harness-tools` (bounded reads, anchored
//! edits), `harness-exec` (PTY pipeline), `harness-lsp` (navigation
//! router) — these adapters only parse arguments, call the behavior,
//! and render its bounded result bytes.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use harness_lsp::LspProvider;
use serde_json::Value;

use harness_core::tools::{ProviderError, ToolProvider, ToolSchema, core_schema};

use super::safety::{SafetyDecision, gate, render_block, render_report};

/// Default `execute_command` timeout when the call omits `timeout_ms`.
const DEFAULT_COMMAND_TIMEOUT_MS: u64 = 120_000;

pub(crate) fn str_arg<'a>(arguments: &'a Value, key: &str) -> Result<&'a str, ProviderError> {
    arguments.get(key).and_then(Value::as_str).ok_or_else(|| {
        ProviderError::InvalidArguments {
            message: format!("arguments must include a string {key:?} field"),
        }
    })
}

pub(crate) fn optional_u64_arg(arguments: &Value, key: &str) -> Result<Option<u64>, ProviderError> {
    match arguments.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_u64().map(Some).ok_or_else(|| {
            ProviderError::InvalidArguments {
                message: format!("{key:?} must be a non-negative integer"),
            }
        }),
    }
}

fn provider_error(error: harness_tools::ToolsError) -> ProviderError {
    let message = error.to_string();
    match error {
        // A line outside the file is the caller's contract violation,
        // not a tool failure.
        harness_tools::ToolsError::LineOutOfRange { .. } => {
            ProviderError::InvalidArguments { message }
        }
        _ => ProviderError::Execution { message },
    }
}

/// `view_window` over `harness-tools`' bounded, elision-guarded reader.
pub(crate) struct ViewWindowTool;

#[async_trait]
impl ToolProvider for ViewWindowTool {
    fn schema(&self) -> ToolSchema {
        core_schema("view_window")
    }

    async fn execute(&self, arguments: &Value) -> Result<Vec<u8>, ProviderError> {
        let path = str_arg(arguments, "path")?;
        let start_line = optional_u64_arg(arguments, "start_line")?.map(|line| line as usize);
        let view = harness_tools::window::view_window_path(Path::new(path), start_line)
            .map_err(provider_error)?;
        Ok(view.render().into_bytes())
    }
}

/// `edit_lines` over `harness-tools`' anchored, uniqueness-validated,
/// syntax-gated edits; the model sees only the changed-region fringe.
pub(crate) struct EditLinesTool;

#[async_trait]
impl ToolProvider for EditLinesTool {
    fn schema(&self) -> ToolSchema {
        core_schema("edit_lines")
    }

    async fn execute(&self, arguments: &Value) -> Result<Vec<u8>, ProviderError> {
        let path = str_arg(arguments, "path")?;
        let raw_edits = arguments.get("edits").and_then(Value::as_array).ok_or_else(|| {
            ProviderError::InvalidArguments {
                message: "arguments must include an 'edits' array".to_owned(),
            }
        })?;
        let mut edits = Vec::with_capacity(raw_edits.len());
        for raw in raw_edits {
            let line = optional_u64_arg(raw, "line")?.ok_or_else(|| {
                ProviderError::InvalidArguments {
                    message: "each edit needs a 1-based 'line'".to_owned(),
                }
            })?;
            if line == 0 {
                return Err(ProviderError::InvalidArguments {
                    message: "'line' is 1-based; got 0".to_owned(),
                });
            }
            edits.push(harness_tools::edit::LineEdit {
                line: line as usize,
                old_text: str_arg(raw, "old_text")?.to_owned(),
                new_text: str_arg(raw, "new_text")?.to_owned(),
                hash: None,
            });
        }
        let outcome = harness_tools::edit::apply_edits_path(Path::new(path), &edits, None)
            .map_err(provider_error)?;
        Ok(outcome.fringe.render().into_bytes())
    }
}

/// `execute_command` over `harness-exec`'s PTY pipeline behind the
/// per-command safety gate (todo 4): the command line runs under the
/// resolved default shell, output is the bounded screen-delta
/// rendering, never raw bytes, and every result carries the
/// requested-vs-enforced isolation report (a network-shaped command
/// under the deny-all egress policy is typed-blocked before spawn).
pub(crate) struct ExecuteCommandTool {
    pub(crate) runner: Arc<harness_exec::PtyRunner>,
}

pub(crate) fn shell_argv(command: &str) -> Result<(String, Vec<String>), ProviderError> {
    #[cfg(windows)]
    {
        use harness_exec::shell::{PathFinder, ShellKind, resolve_default_shell};
        let resolved = resolve_default_shell(None, &PathFinder).map_err(|error| {
            ProviderError::Execution {
                message: error.to_string(),
            }
        })?;
        let program = resolved.path.to_string_lossy().into_owned();
        let args = match resolved.kind {
            ShellKind::Cmd => vec!["/d".into(), "/c".into(), command.to_owned()],
            _ => vec!["-NoProfile".into(), "-Command".into(), command.to_owned()],
        };
        Ok((program, args))
    }
    #[cfg(not(windows))]
    {
        Ok((
            "/bin/sh".to_owned(),
            vec!["-c".to_owned(), command.to_owned()],
        ))
    }
}

#[async_trait]
impl ToolProvider for ExecuteCommandTool {
    fn schema(&self) -> ToolSchema {
        core_schema("execute_command")
    }

    async fn execute(&self, arguments: &Value) -> Result<Vec<u8>, ProviderError> {
        let command = str_arg(arguments, "command")?;
        let timeout_ms = optional_u64_arg(arguments, "timeout_ms")?;
        match gate(command) {
            SafetyDecision::Block { violation, backend } => {
                Ok(render_block(&violation, backend).into_bytes())
            }
            SafetyDecision::Pass(verdict) => {
                let (program, args) = shell_argv(command)?;
                let spec = harness_exec::ExecSpec {
                    program,
                    args,
                    cwd: None,
                    env: BTreeMap::new(),
                    env_scrub: Some(verdict.policy.env_scrub.clone()),
                    timeout: Duration::from_millis(
                        timeout_ms.unwrap_or(DEFAULT_COMMAND_TIMEOUT_MS),
                    )
                    .min(harness_exec::spec::MAX_COMMAND_TIMEOUT),
                    size: harness_exec::ScreenSize::default(),
                    run_in: harness_exec::RunIn::Native,
                };
                let runner = Arc::clone(&self.runner);
                let outcome = tokio::task::spawn_blocking(move || runner.run(&spec))
                    .await
                    .map_err(|join| ProviderError::Execution {
                        message: format!("command task failed: {join}"),
                    })?
                    .map_err(|error| ProviderError::Execution {
                        message: error.to_string(),
                    })?;
                let status = match outcome.status {
                    harness_exec::ExecStatus::Completed { exit_code } => {
                        format!("exit code {exit_code}")
                    }
                    harness_exec::ExecStatus::TimedOut => {
                        format!("timed out after {:?}", outcome.elapsed)
                    }
                    harness_exec::ExecStatus::Cancelled => "cancelled".to_owned(),
                };
                let safety = render_report(&verdict);
                Ok(format!("{status}\n{}\n{safety}", outcome.model_text).into_bytes())
            }
        }
    }
}

/// `lsp_query` over `harness-lsp`'s multi-server router (lazy spawn per
/// language; a missing server is a typed failure, degradation to
/// lexical search is a result, not an error).
#[derive(Clone)]
pub(crate) struct LspQueryTool {
    pub(crate) router: harness_lsp::LspRouter,
    pub(crate) root: std::path::PathBuf,
}

#[async_trait]
impl ToolProvider for LspQueryTool {
    fn schema(&self) -> ToolSchema {
        core_schema("lsp_query")
    }

    async fn execute(&self, arguments: &Value) -> Result<Vec<u8>, ProviderError> {
        let kind = str_arg(arguments, "kind")?;
        let file = str_arg(arguments, "file")?;
        // The day-one schema: 1-based `line`, 0-based `character`;
        // DocumentPosition is 1-based line AND column.
        let line = optional_u64_arg(arguments, "line")?.ok_or_else(|| {
            ProviderError::InvalidArguments {
                message: "'line' is required (1-based)".to_owned(),
            }
        })?;
        let character = optional_u64_arg(arguments, "character")?.ok_or_else(|| {
            ProviderError::InvalidArguments {
                message: "'character' is required (0-based)".to_owned(),
            }
        })?;
        let position = harness_lsp::DocumentPosition {
            file: std::path::PathBuf::from(file),
            line: line as u32,
            column: character as u32 + 1,
        };
        let answer = match kind {
            "definition" => self.router.goto_definition(position).await,
            "references" => self.router.find_references(position).await,
            "hover" => self.router.hover(position).await,
            other => {
                return Err(ProviderError::InvalidArguments {
                    message: format!(
                        "'kind' must be definition, references, or hover; got {other:?}"
                    ),
                });
            }
        };
        match answer {
            Ok(harness_lsp::Answer::Locations(locations)) => {
                Ok(harness_lsp::render_locations(&self.root, &locations).into_bytes())
            }
            Ok(harness_lsp::Answer::Hover(text)) => Ok(text.as_str().to_owned().into_bytes()),
            Ok(harness_lsp::Answer::DegradedToSearch { reason, hint }) => {
                Ok(format!("{hint} ({reason})").into_bytes())
            }
            Err(error) => Err(ProviderError::Execution {
                message: error.to_string(),
            }),
        }
    }
}
