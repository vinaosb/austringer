//! The file-based multi-provider MCP manager (todo 29, Warp pattern):
//! scans config files across providers with per-path diagnostics (one bad
//! file never kills the scan), hash-keyed TEMPLATABLE installations with
//! `{{secret:…}}` substitution through the SecretStore seam, and
//! auth-gated deferred autostart.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

pub use crate::config::ProviderKind;
use crate::config::{Fingerprint, ServerEntry, TransportSpec};
use crate::provider_formats::{mcp_servers_root, parse_servers, toml_to_json};
use crate::secret::{SecretError, SecretStore, substitute_secrets};

/// One scanned server installation: provenance plus the raw entry.
#[derive(Debug, Clone)]
pub struct Installation {
    /// Provider file kind the entry came from.
    pub provider: ProviderKind,
    /// File the entry was scanned from.
    pub path: PathBuf,
    /// The scanned entry (may still contain `{{secret:…}}` placeholders).
    pub entry: ServerEntry,
}

/// One per-path parse diagnostic (malformed files, invalid entries).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathDiagnostic {
    /// The offending file.
    pub path: PathBuf,
    /// What was wrong (bounded, no secret values).
    pub message: String,
}

/// The outcome of scanning one or more directories.
#[derive(Debug, Clone, Default)]
pub struct ScanReport {
    /// Successfully parsed installations across every scanned file.
    pub installations: Vec<Installation>,
    /// Per-path problems; the scan always completes.
    pub diagnostics: Vec<PathDiagnostic>,
}

/// Whether a scanned server may start now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutostartDecision {
    /// Start immediately (the entry is explicitly `trusted_autostart`;
    /// the trust gate still binds to the entry fingerprint).
    StartNow,
    /// Do not spawn yet: start on first use behind the approval gate.
    DeferredOnFirstUse,
}

impl ScanReport {
    /// The autostart decision for one scanned entry.
    #[must_use]
    pub fn autostart_for(entry: &ServerEntry) -> AutostartDecision {
        if entry.trusted_autostart {
            AutostartDecision::StartNow
        } else {
            AutostartDecision::DeferredOnFirstUse
        }
    }
}

const HARNESS_FILES: [&str; 2] = ["mcp.json", "mcp.toml"];

/// Scans the well-known provider paths under each base directory.
/// Candidate files: `.harness/mcp.json`, `.harness/mcp.toml`,
/// `.harness/mcp-servers/*.json|*.toml`, `.mcp.json`, `.cursor/mcp.json`,
/// `codeium/config/config.json`. Missing files are skipped silently;
/// present-but-broken ones become diagnostics.
pub fn scan_directories(bases: &[PathBuf]) -> ScanReport {
    let mut report = ScanReport::default();
    for base in bases {
        let harness_dir = base.join(".harness");
        for file in HARNESS_FILES {
            scan_into(&harness_dir.join(file), ProviderKind::Harness, &mut report);
        }
        if let Ok(entries) = std::fs::read_dir(harness_dir.join("mcp-servers")) {
            let mut paths: Vec<PathBuf> = entries
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| {
                    matches!(
                        path.extension().and_then(|ext| ext.to_str()),
                        Some("json" | "toml")
                    )
                })
                .collect();
            paths.sort();
            for path in paths {
                scan_into(&path, ProviderKind::Harness, &mut report);
            }
        }
        scan_into(&base.join(".mcp.json"), ProviderKind::Claude, &mut report);
        scan_into(
            &base.join(".claude.json"),
            ProviderKind::Claude,
            &mut report,
        );
        scan_into(
            &base.join(".cursor").join("mcp.json"),
            ProviderKind::Cursor,
            &mut report,
        );
        scan_into(
            &base.join("codeium").join("config").join("config.json"),
            ProviderKind::Codeium,
            &mut report,
        );
    }
    report
}

/// Parses one known file into a report (any parse failure = diagnostic).
pub fn scan_file(path: &Path, provider: ProviderKind) -> ScanReport {
    let mut report = ScanReport::default();
    scan_into(path, provider, &mut report);
    report
}

fn scan_into(path: &Path, provider: ProviderKind, report: &mut ScanReport) {
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(_) => return,
    };
    let document = if path.extension().and_then(|ext| ext.to_str()) == Some("toml") {
        match toml_to_json(&contents) {
            Ok(value) => value,
            Err(message) => {
                report.diagnostics.push(PathDiagnostic {
                    path: path.to_path_buf(),
                    message,
                });
                return;
            }
        }
    } else {
        match serde_json::from_str::<Value>(&contents) {
            Ok(value) => value,
            Err(error) => {
                report.diagnostics.push(PathDiagnostic {
                    path: path.to_path_buf(),
                    message: format!("invalid json: {error}"),
                });
                return;
            }
        }
    };
    let Some(servers) = mcp_servers_root(&document, provider) else {
        report.diagnostics.push(PathDiagnostic {
            path: path.to_path_buf(),
            message: "no mcpServers map found".to_owned(),
        });
        return;
    };
    let mut entries = parse_servers(servers, provider, path, &mut report.diagnostics);
    for entry in &mut entries {
        entry.source_provider = Some(provider);
    }
    report
        .installations
        .extend(entries.into_iter().map(|entry| Installation {
            provider,
            path: path.to_path_buf(),
            entry,
        }));
}

/// Hash-keyed registry of shareable server TEMPLATES (Warp): templates
/// carry `{{secret:…}}` placeholders; `instantiate` resolves them against
/// a SecretStore, and the RESOLVED entry's fingerprint is what trust
/// binds to (a template hash alone never grants anything).
#[derive(Debug, Default)]
pub struct TemplateRegistry {
    templates: BTreeMap<String, ServerEntry>,
}

impl TemplateRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Installs one template, keyed by the hash of its canonical form.
    #[must_use]
    pub fn install(&mut self, template: ServerEntry) -> Fingerprint {
        let key = template.fingerprint();
        self.templates.insert(key.0.clone(), template);
        key
    }

    /// Resolves a template's placeholders against a store. Fails closed
    /// on any missing secret.
    pub fn instantiate(
        &self,
        key: &Fingerprint,
        store: &dyn SecretStore,
    ) -> Result<ServerEntry, SecretError> {
        let template = self
            .templates
            .get(&key.0)
            .ok_or(SecretError::Missing("<unknown template>".to_owned()))?;
        let mut resolved = template.clone();
        let TransportSpec::Stdio { command, args } = &template.transport else {
            return Ok(resolved);
        };
        let command = substitute_secrets(command, store)?;
        let mut resolved_args = Vec::with_capacity(args.len());
        for arg in args {
            resolved_args.push(substitute_secrets(arg, store)?);
        }
        let mut env = std::collections::BTreeMap::new();
        for (name, value) in &template.env {
            env.insert(name.clone(), substitute_secrets(value, store)?);
        }
        resolved.transport = TransportSpec::Stdio {
            command,
            args: resolved_args,
        };
        resolved.env = env;
        Ok(resolved)
    }
}
