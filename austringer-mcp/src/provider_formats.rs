//! Per-provider config dialects (todo 29). Every parser is DEFENSIVE: a
//! malformed value becomes a diagnostic, never a panic. All dialects
//! share the `mcpServers` map shape; they differ in location and in
//! which nesting level the map lives at.

use serde_json::Value;

use crate::config::{ProviderKind, ServerEntry, TransportSpec};
use crate::manager::PathDiagnostic;
use std::path::Path;

/// Extracts the `mcpServers` map from one parsed document, honoring the
/// dialect's nesting.
pub fn mcp_servers_root(
    document: &Value,
    provider: ProviderKind,
) -> Option<&serde_json::Map<String, Value>> {
    let root = match provider {
        ProviderKind::Codeium => document.get("experimental").unwrap_or(document),
        _ => document,
    };
    root.get("mcpServers")?.as_object()
}

/// Parses one `mcpServers` map into entries; every malformed SERVER
/// becomes a diagnostic (its name recorded), never a scan-wide failure.
pub fn parse_servers(
    servers: &serde_json::Map<String, Value>,
    provider: ProviderKind,
    source: &Path,
    diagnostics: &mut Vec<PathDiagnostic>,
) -> Vec<ServerEntry> {
    let mut entries = Vec::new();
    for (name, raw) in servers {
        let mut entry = match raw.as_object() {
            Some(fields) => match server_from_fields(name, fields) {
                Ok(entry) => entry,
                Err(message) => {
                    diagnostics.push(PathDiagnostic {
                        path: source.to_path_buf(),
                        message: format!("server {name:?}: {message}"),
                    });
                    continue;
                }
            },
            None => {
                diagnostics.push(PathDiagnostic {
                    path: source.to_path_buf(),
                    message: format!("server {name:?}: body must be an object"),
                });
                continue;
            }
        };
        if let Err(message) = entry.validate() {
            diagnostics.push(PathDiagnostic {
                path: source.to_path_buf(),
                message: format!("server {name:?}: {message}"),
            });
            continue;
        }
        entry.source_provider = Some(provider);
        entries.push(entry);
    }
    entries
}

fn server_from_fields(
    name: &str,
    fields: &serde_json::Map<String, Value>,
) -> Result<ServerEntry, String> {
    let transport = if let Some(command) = fields.get("command").and_then(Value::as_str) {
        let args = fields
            .get("args")
            .and_then(Value::as_array)
            .map(|array| {
                array
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();
        TransportSpec::Stdio {
            command: command.to_owned(),
            args,
        }
    } else if let Some(url) = fields.get("url").and_then(Value::as_str) {
        TransportSpec::Http {
            url: url.to_owned(),
        }
    } else {
        return Err("entry needs either a stdio `command` or an http `url`".to_owned());
    };
    let mut entry = ServerEntry::new(name.to_owned(), transport);
    if let Some(env) = fields.get("env").and_then(Value::as_object) {
        for (key, value) in env {
            let rendered = match value {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            entry.env.insert(key.clone(), rendered);
        }
    }
    if let Some(roots) = fields.get("roots").and_then(Value::as_array) {
        for root in roots {
            if let Some(text) = root.as_str() {
                entry.roots.push(std::path::PathBuf::from(text));
            }
        }
    }
    if let Some(trusted) = fields.get("trusted_autostart").and_then(Value::as_bool) {
        entry.trusted_autostart = trusted;
    }
    if let Some(timeout_ms) = fields.get("connect_timeout_ms").and_then(Value::as_u64) {
        entry.connect_timeout_ms = timeout_ms;
    }
    Ok(entry)
}

/// Parses a TOML document into the same JSON shape, then reuses the JSON
/// path (one dialect surface, `toml` only widens the file formats). Note
/// `toml::Value`'s `FromStr` parses a bare VALUE expression in toml 0.9;
/// documents deserialize as `toml::Table`.
pub fn toml_to_json(text: &str) -> Result<Value, String> {
    let value: toml::Table =
        toml::from_str(text).map_err(|error| format!("invalid toml: {error}"))?;
    serde_json::to_value(&value).map_err(|error| format!("toml re-encode failed: {error}"))
}
