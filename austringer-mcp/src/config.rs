//! MCP server configuration entries and their identity fingerprints.
//!
//! Trust (todo 29) binds to the blake3 of the ENTIRE entry — name, transport
//! (command+args, or URL), environment, and roots — never the entry name
//! alone: a benign-approved entry whose command later swaps has a different
//! fingerprint and must re-ask (Cursor MCPoison CVE-2025-54136 shape).
//!
//! The canonical serialization is byte-stable: struct fields serialize in
//! declaration order, `env` is a `BTreeMap` (sorted keys), and `roots` keep
//! their forward-slash-serialized form; the same workspace rules as the
//! event log (no timestamps, no OS-native separators in the hashed form).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Default per-server connect timeout (milliseconds).
pub const DEFAULT_CONNECT_TIMEOUT_MS: u64 = 10_000;

/// A 32-byte blake3 fingerprint over the canonical entry form, hex-encoded.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Fingerprint(pub String);

/// Which provider config a scanned entry came from (provenance only;
/// NOT part of the identity fingerprint).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProviderKind {
    /// No provenance (constructed in code).
    #[default]
    InProcess,
    /// `.austringer` files.
    Austringer,
    /// `.mcp.json` / `.claude.json`.
    Claude,
    /// `.cursor/mcp.json`.
    Cursor,
    /// codeium-style config.
    Codeium,
}

impl std::fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// How to reach one MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransportSpec {
    /// Spawn a stdio child process.
    Stdio {
        /// The executable (possibly a path).
        command: String,
        /// argv tail.
        args: Vec<String>,
    },
    /// Connect over Streamable HTTP.
    Http {
        /// The endpoint URL.
        url: String,
    },
}

/// One MCP server installation: the unit of trust, pooling, and scanning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerEntry {
    /// Server name (the user-facing identity; must validate).
    pub name: String,
    /// Transport selector with its identity fields.
    pub transport: TransportSpec,
    /// Extra environment for stdio children (sorted map; hashed).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    /// Declared roots for this server (client-side confinement; hashed —
    /// widening the roots changes identity and re-asks).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roots: Vec<PathBuf>,
    /// Local knobs, deliberately OUT of the fingerprint: they change how we
    /// drive the server, not which server it is.
    #[serde(default = "default_connect_timeout_ms")]
    pub connect_timeout_ms: u64,
    /// Deferred-autostart marker (manager-level policy; trust still binds
    /// to the fingerprint, so flipping this cannot inherit approval).
    #[serde(default)]
    pub trusted_autostart: bool,
    /// Where the entry was scanned from (provenance; excluded from the
    /// fingerprint).
    #[serde(skip)]
    pub source_provider: Option<ProviderKind>,
}

const fn default_connect_timeout_ms() -> u64 {
    DEFAULT_CONNECT_TIMEOUT_MS
}

impl ServerEntry {
    /// Creates a minimal entry (no env, no roots, default timeouts).
    #[must_use]
    pub fn new(name: String, transport: TransportSpec) -> Self {
        Self {
            name,
            transport,
            env: BTreeMap::new(),
            roots: Vec::new(),
            connect_timeout_ms: DEFAULT_CONNECT_TIMEOUT_MS,
            trusted_autostart: false,
            source_provider: None,
        }
    }

    /// The connect timeout as a `Duration`.
    #[must_use]
    pub fn connect_timeout(&self) -> Duration {
        Duration::from_millis(self.connect_timeout_ms)
    }

    /// Typed validation: names reject control characters and path
    /// separators (they later key files and pattern matching), URLs must
    /// parse with http(s) and a host.
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("server name must not be empty".to_owned());
        }
        if self.name.chars().any(|c| c.is_control()) {
            return Err("server name must not contain control characters".to_owned());
        }
        if self.name.contains(['/', '\\']) {
            return Err("server name must not contain path separators".to_owned());
        }
        if let TransportSpec::Http { url } = &self.transport {
            let parsed = url::Url::parse(url)
                .map_err(|error| format!("server url is not a valid URL: {error}"))?;
            if !matches!(parsed.scheme(), "http" | "https") {
                return Err("server url must be http(s)".to_owned());
            }
            if parsed.host_str().is_none() {
                return Err("server url must carry a host".to_owned());
            }
        }
        Ok(())
    }

    /// The blake3 fingerprint over the canonical form of every identity
    /// field (name, transport, env, roots). Canonical bytes are written by
    /// an explicit length-prefixed encoder — not `serde_json` — so the
    /// hashing path is infallible (no serializer failure can silently
    /// degrade to hashing empty input).
    #[must_use]
    pub fn fingerprint(&self) -> Fingerprint {
        let mut canonical = Vec::new();
        push_field(&mut canonical, self.name.as_bytes());
        match &self.transport {
            TransportSpec::Stdio { command, args } => {
                canonical.extend_from_slice(b"stdio\x00");
                push_field(&mut canonical, command.as_bytes());
                canonical.extend_from_slice(
                    &u32::try_from(args.len()).unwrap_or(u32::MAX).to_le_bytes(),
                );
                for arg in args {
                    push_field(&mut canonical, arg.as_bytes());
                }
            }
            TransportSpec::Http { url } => {
                canonical.extend_from_slice(b"http\x00");
                push_field(&mut canonical, url.as_bytes());
            }
        }
        // BTreeMap iterates in sorted key order: env is order-canonical.
        for (key, value) in &self.env {
            push_field(&mut canonical, key.as_bytes());
            push_field(&mut canonical, value.as_bytes());
        }
        for root in &self.roots {
            push_field(&mut canonical, root.to_string_lossy().as_bytes());
        }
        Fingerprint(blake3::hash(&canonical).to_hex().to_string())
    }
}

/// Appends one length-prefixed field (u32 LE) to the canonical buffer.
fn push_field(buffer: &mut Vec<u8>, bytes: &[u8]) {
    buffer.extend_from_slice(&u32::try_from(bytes.len()).unwrap_or(u32::MAX).to_le_bytes());
    buffer.extend_from_slice(bytes);
}
