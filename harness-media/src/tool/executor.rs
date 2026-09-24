//! The subprocess-tier tool executor (plan M15): availability over the
//! declarative manifests, the fail-closed sandbox policy gate, env
//! redaction at the command seam, the bounded-output spawn with hard
//! timeout and kernel kill-tree, and the SHA-256 install gate. Where
//! each guarantee is enforced (and the AppContainer env-tradeoff) is
//! documented at the [`tool`](crate::tool) module.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use harness_safety::policy::decl::{
    SandboxPolicy, SandboxTier, ViolationEvent, ViolationKind,
};
use harness_safety::scrub::EnvScrub;
use harness_safety::supply::{self, ReleaseAsset};

use crate::descriptor::Operation;
use crate::error::{InstallFailure, MediaError};
use crate::format::FormatId;
use crate::tool::manifest::{self, render_args, ExternalToolSpec};
use crate::tool::spawn::{argv_line, find_program, redacted_child_env, wait_bounded};
use crate::tool::types::{ToolAvailability, ToolError, ToolRun};

/// Per-tool opt-in state; a tool runs only when present in `enabled`.
/// The default is empty - the supply-chain carve-out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolConfig {
    /// Enabled tools by manifest name, with their overrides.
    pub enabled: BTreeMap<String, ToolOverride>,
    /// The explicit auto-install opt-in (the SHA-256 gate applies on
    /// top of it).
    pub install_enabled: bool,
    /// The session media temp dir: the only writable scope a tool gets.
    pub session_temp: PathBuf,
}

/// Program/argv substitution for one tool - the mocktool hermetic lane
/// (T40 precedent: the override argv REPLACES the rendered template).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolOverride {
    /// Explicit program path replacing the manifest program.
    pub program: Option<String>,
    /// Whole-argv replacement for the op template.
    pub args: Vec<String>,
    /// Wall-clock timeout override.
    pub timeout_ms: Option<u64>,
}

/// One subprocess invocation request.
#[derive(Debug, Clone)]
pub struct ToolCall<'a> {
    /// Manifest tool name.
    pub tool: &'a str,
    /// Op name (a manifest template key).
    pub op: &'a str,
    /// Placeholder values for the op template (`input`, `output`, ...).
    pub vars: BTreeMap<String, String>,
    /// Caller environment; redaction applies before the child sees it.
    pub env: Option<&'a BTreeMap<String, String>>,
}

impl<'a> ToolCall<'a> {
    /// A call for `tool` / `op`.
    #[must_use]
    pub fn new(tool: &'a str, op: &'a str) -> Self {
        Self {
            tool,
            op,
            vars: BTreeMap::new(),
            env: None,
        }
    }

    /// The caller environment to redact and pass.
    #[must_use]
    pub fn env(mut self, env: &'a BTreeMap<String, String>) -> Self {
        self.env = Some(env);
        self
    }
}

/// The subprocess-tier executor. Immutable config; cheap to clone.
#[derive(Debug, Clone, Default)]
pub struct ToolExecutor {
    config: ToolConfig,
}

impl ToolExecutor {
    /// An executor bound to `config`.
    #[must_use]
    pub fn new(config: ToolConfig) -> Self {
        Self { config }
    }

    /// The bound config (read-only).
    #[must_use]
    pub fn config(&self) -> &ToolConfig {
        &self.config
    }

    /// Is the tool enabled AND its program resolvable? A tool absent
    /// from the enabled map is unavailable even when the binary exists
    /// on PATH - the disabled-by-default carve-out.
    #[must_use]
    pub fn availability(&self, tool: &str) -> ToolAvailability {
        let Some(spec) = manifest::manifests().get(tool) else {
            return ToolAvailability::Unavailable {
                install_hint: format!("no manifest is registered for tool {tool}"),
            };
        };
        let Some(r#override) = self.config.enabled.get(tool) else {
            return ToolAvailability::Unavailable {
                install_hint: spec.install_hint.to_owned(),
            };
        };
        let program = match &r#override.program {
            Some(explicit) => {
                let path = PathBuf::from(explicit);
                path.is_file().then_some(path)
            }
            None => find_program(spec.program),
        };
        match program {
            Some(program) => ToolAvailability::Available {
                program,
                version_pin: spec.version_pin.to_owned(),
            },
            None => ToolAvailability::Unavailable {
                install_hint: spec.install_hint.to_owned(),
            },
        }
    }

    /// Run one op through the sandbox gate, the redacted-env spawn, and
    /// the kill-tree timeout.
    ///
    /// # Errors
    /// Typed [`ToolError`] for every failure mode; a non-zero exit or
    /// timeout never yields a partial artifact.
    pub fn run(&self, call: &ToolCall<'_>) -> Result<ToolRun, ToolError> {
        let spec = manifest::manifests().get(call.tool).ok_or_else(|| {
            ToolError::ToolUnavailable {
                tool: call.tool.to_owned(),
                install_hint: format!("no manifest is registered for tool {}", call.tool),
            }
        })?;
        let ToolAvailability::Available {
            program,
            version_pin,
        } = self.availability(call.tool)
        else {
            return Err(ToolError::ToolUnavailable {
                tool: call.tool.to_owned(),
                install_hint: spec.install_hint.to_owned(),
            });
        };
        self.sandbox_gate(call.tool)?;
        let argv = self.argv(spec, call);
        let command_line = argv_line(&program, &argv);
        let child_env = redacted_child_env(spec, call.env);
        std::fs::create_dir_all(&self.config.session_temp).map_err(|detail| {
            ToolError::Spawn {
                tool: call.tool.to_owned(),
                detail: format!("session temp: {detail}"),
            }
        })?;
        let mut command = Command::new(&program);
        command
            .args(&argv)
            .env_clear()
            .envs(&child_env)
            .current_dir(&self.config.session_temp)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command.spawn().map_err(|detail| ToolError::Spawn {
            tool: call.tool.to_owned(),
            detail: detail.to_string(),
        })?;
        let timeout = Duration::from_millis(
            self.config
                .enabled
                .get(call.tool)
                .and_then(|r#override| r#override.timeout_ms)
                .unwrap_or(spec.timeout_ms),
        );
        wait_bounded(&mut child, call.tool, timeout, &command_line, &version_pin)
    }

    /// The opt-in SHA-256 install gate: verify FIRST, write only on a
    /// matching pin; malformed pins fail closed inside
    /// [`verify_sha256`](harness_safety::supply::verify_sha256). Nothing
    /// executes during install.
    ///
    /// # Errors
    /// [`ToolError::InstallFailed`] (mismatch / fetch-write failure) or
    /// [`ToolError::ToolUnavailable`] when install is not opted in.
    pub fn install_asset(
        &self,
        tool: &str,
        pin: &ReleaseAsset,
        bytes: &[u8],
    ) -> Result<PathBuf, ToolError> {
        let spec = manifest::manifests().get(tool).ok_or_else(|| {
            ToolError::ToolUnavailable {
                tool: tool.to_owned(),
                install_hint: format!("no manifest is registered for tool {tool}"),
            }
        })?;
        if !self.config.install_enabled {
            return Err(ToolError::ToolUnavailable {
                tool: tool.to_owned(),
                install_hint: "auto-install is opt-in: set install_enabled before installing \
                               pinned assets"
                    .into(),
            });
        }
        let pin_ok =
            pin.version == spec.version_pin && supply::verify_sha256(bytes, &pin.sha256);
        if !pin_ok {
            return Err(ToolError::InstallFailed {
                tool: tool.to_owned(),
                reason: InstallFailure::ShaMismatch,
            });
        }
        let store = self.config.session_temp.join("toolstore");
        let write = || -> std::io::Result<PathBuf> {
            std::fs::create_dir_all(&store)?;
            let dest = store.join(format!("{tool}-{}.bin", pin.version));
            std::fs::write(&dest, bytes)?;
            Ok(dest)
        };
        write().map_err(|_| ToolError::InstallFailed {
            tool: tool.to_owned(),
            reason: InstallFailure::Download,
        })
    }

    /// The registry seam (M6 dispatch): what dispatching a Subprocess
    /// binding answers while the per-format adapter handler is not
    /// bound yet (the M16-M21 todos bind theirs).
    pub(crate) fn subprocess_refusal(
        &self,
        tool: &str,
        binding_hint: &str,
        format: FormatId,
        op: Operation,
    ) -> MediaError {
        match self.availability(tool) {
            ToolAvailability::Available { .. } => MediaError::UnsupportedOperation {
                format,
                op,
                reason: format!(
                    "tool {tool} is enabled and installed; the {format} subprocess adapter \
                     arrives with its wave todo (M16-M21)"
                ),
            },
            ToolAvailability::Unavailable { install_hint } => {
                let hint = if install_hint.starts_with("no manifest") {
                    binding_hint.to_owned()
                } else {
                    install_hint
                };
                MediaError::ToolUnavailable {
                    tool: tool.to_owned(),
                    install_hint: hint,
                }
            }
        }
    }

    fn sandbox_gate(&self, tool: &str) -> Result<(), ToolError> {
        let policy = SandboxPolicy {
            tier: SandboxTier::Readonly,
            writable_roots: vec![self.config.session_temp.clone()],
            readable_roots: Vec::new(),
            protected_paths: Vec::new(),
            deny_globs: Vec::new(),
            egress: harness_safety::egress::EgressPolicy::none(),
            env_scrub: EnvScrub::default(),
        };
        match policy.validate() {
            Ok(()) => Ok(()),
            Err(_) => Err(ToolError::SandboxDenied {
                tool: tool.to_owned(),
                violation: ViolationEvent::new(
                    ViolationKind::WriteOutsideRoot,
                    &format!(
                        "tool sandbox policy rejected: the writable scope must be an absolute \
                         session temp path (got {:?}) and network must be none",
                        self.config.session_temp
                    ),
                ),
            }),
        }
    }

    fn argv(&self, spec: &ExternalToolSpec, call: &ToolCall<'_>) -> Vec<String> {
        if let Some(r#override) = self.config.enabled.get(call.tool)
            && !r#override.args.is_empty()
        {
            return r#override.args.clone();
        }
        spec.op_template(call.op)
            .map(|template| render_args(template.args, &call.vars))
            .unwrap_or_default()
    }
}
