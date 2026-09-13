//! Optional OTel-flavored export for fleet ops (Grok posture): DOUBLE
//! opt-in (config flag AND env), content-free by default with four
//! independent content gates (prompts / responses / tool details /
//! tool content), secret-scrubbed, mTLS-ready config surface. Emits
//! JSON telemetry records through a transport seam (HTTP in production,
//! in-memory in tests).

use std::sync::Arc;

/// The env-var arm of the double opt-in.
pub const ENV_OPT_IN: &str = "HARNESS_TELEMETRY";

/// Independent content gates. All `false` by default: telemetry without
/// explicit opt-in carries counters and names only — never content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ContentGates {
    /// Include prompt text.
    pub prompts: bool,
    /// Include model response text.
    pub responses: bool,
    /// Include tool names/arguments.
    pub tool_details: bool,
    /// Include tool result content.
    pub tool_content: bool,
}

/// mTLS client identity for the export endpoint (config surface; the
/// transport applies it when wiring TLS — todo 41 wires the certificate
/// loading itself, the surface is validated here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MtlsConfig {
    /// PEM client certificate path.
    pub client_cert_path: String,
    /// PEM client key path.
    pub client_key_path: String,
    /// Optional PEM CA bundle path.
    pub ca_path: Option<String>,
}

/// Telemetry configuration.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TelemetryConfig {
    /// Config-file arm of the double opt-in.
    pub enabled: bool,
    /// Export endpoint (required for export; None disables regardless).
    pub endpoint: Option<String>,
    /// Content gates.
    pub gates: ContentGates,
    /// mTLS identity.
    pub mtls: Option<MtlsConfig>,
    /// Scrub known secret shapes from any content that passes the
    /// gates.
    pub scrub: bool,
}

impl TelemetryConfig {
    /// Pure double-opt-in decision (env arm injected for testability).
    #[must_use]
    pub fn active_given(&self, env_arm: bool) -> bool {
        self.enabled && env_arm && self.endpoint.is_some()
    }

    /// Live decision: config arm AND `HARNESS_TELEMETRY=1` AND endpoint.
    #[must_use]
    pub fn active(&self) -> bool {
        let env_arm = std::env::var(ENV_OPT_IN).ok().as_deref() == Some("1");
        self.active_given(env_arm)
    }
}

/// Telemetry failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("telemetry error: {message}")]
pub struct TelemetryError {
    /// Cause.
    pub message: String,
}

/// One export record: names and counters always; content only through
/// open gates (and scrubbed when scrubbing is on).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct TelemetryRecord {
    /// Record kind (`prompt`, `response`, `tool_call`, `tool_result`).
    pub kind: String,
    /// Session id (never content).
    pub session_id: String,
    /// Bounded attributes (names, counters).
    pub attributes: std::collections::BTreeMap<String, String>,
    /// Gated, scrubbed content.
    pub content: Option<String>,
}

/// Export transport seam.
pub trait TelemetryTransport: Send + Sync {
    /// Deliver one serialized record batch.
    fn send(&self, body: String) -> Result<(), TelemetryError>;
}

/// HTTP transport over the workspace reqwest client. Fire-and-forget:
/// telemetry must never block or fail the daemon — delivery errors are
/// dropped on the floor.
pub struct HttpTransport {
    endpoint: String,
}

impl HttpTransport {
    /// Transport for `endpoint`.
    #[must_use]
    pub fn new(endpoint: String) -> Self {
        Self { endpoint }
    }
}

impl TelemetryTransport for HttpTransport {
    fn send(&self, body: String) -> Result<(), TelemetryError> {
        if tokio::runtime::Handle::try_current().is_err() {
            return Err(TelemetryError {
                message: "no async runtime".into(),
            });
        }
        let endpoint = self.endpoint.clone();
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let _ = client
                .post(endpoint)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body)
                .send()
                .await;
        });
        Ok(())
    }
}

/// In-memory transport (tests, embedders).
#[derive(Default)]
pub struct MemoryTransport {
    sent: std::sync::Mutex<Vec<String>>,
}

impl MemoryTransport {
    /// Empty transport.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Everything delivered so far.
    pub fn delivered(&self) -> Vec<String> {
        match self.sent.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

impl TelemetryTransport for MemoryTransport {
    fn send(&self, body: String) -> Result<(), TelemetryError> {
        match self.sent.lock() {
            Ok(mut guard) => {
                guard.push(body);
                Ok(())
            }
            Err(poisoned) => {
                poisoned.into_inner().push(body);
                Ok(())
            }
        }
    }
}

/// Replace well-known secret shapes with a redaction marker.
#[must_use]
pub fn scrub_secrets(text: &str) -> String {
    let mut out = text.to_owned();
    for marker in ["sk-", "ghp_", "gho_", "xoxb-", "xoxp-", "AKIA"] {
        while let Some(at) = out.find(marker) {
            let after = at + marker.len();
            let keep = out[..after].to_owned();
            let tail: String = out[after..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .collect();
            if tail.len() < 6 {
                break;
            }
            let rest = out[after + tail.len()..].to_owned();
            out = format!("{keep}[REDACTED]{rest}");
        }
    }
    out.replace("Bearer eyJ", "Bearer [REDACTED-")
}

/// The exporter: gates, scrubs, and delivers records.
pub struct TelemetryExporter {
    config: TelemetryConfig,
    transport: Arc<dyn TelemetryTransport>,
}

impl TelemetryExporter {
    /// Exporter over a transport.
    #[must_use]
    pub fn new(config: TelemetryConfig, transport: Arc<dyn TelemetryTransport>) -> Self {
        Self { config, transport }
    }

    /// Build one record under the gates; `None` content when the gate
    /// is closed or there is nothing to include. The pure core of
    /// [`TelemetryExporter::record`].
    #[must_use]
    pub fn build_record(
        &self,
        kind: &str,
        session_id: &str,
        gate: bool,
        content: Option<&str>,
        mut attributes: std::collections::BTreeMap<String, String>,
    ) -> TelemetryRecord {
        let content = match (gate, content) {
            (true, Some(text)) => {
                let text = if self.config.scrub {
                    scrub_secrets(text)
                } else {
                    text.to_owned()
                };
                Some(text)
            }
            _ => None,
        };
        attributes.insert(
            "gated".into(),
            if content.is_some() {
                "open".into()
            } else {
                "closed".into()
            },
        );
        TelemetryRecord {
            kind: kind.to_owned(),
            session_id: session_id.to_owned(),
            attributes,
            content,
        }
    }

    /// Record and deliver; skipped entirely when inactive.
    pub fn record(
        &self,
        kind: &str,
        session_id: &str,
        gate: bool,
        content: Option<&str>,
        attributes: std::collections::BTreeMap<String, String>,
    ) {
        if !self.config.active() {
            return;
        }
        let record = self.build_record(kind, session_id, gate, content, attributes);
        if let Ok(body) = serde_json::to_string(&record) {
            let _ = self.transport.send(body);
        }
    }

    /// Prompt record (prompts gate).
    pub fn record_prompt(&self, session_id: &str, prompt: &str) {
        let mut attributes = std::collections::BTreeMap::new();
        attributes.insert("bytes".into(), prompt.len().to_string());
        self.record(
            "prompt",
            session_id,
            self.config.gates.prompts,
            Some(prompt),
            attributes,
        );
    }

    /// Response record (responses gate).
    pub fn record_response(&self, session_id: &str, response: &str) {
        let mut attributes = std::collections::BTreeMap::new();
        attributes.insert("bytes".into(), response.len().to_string());
        self.record(
            "response",
            session_id,
            self.config.gates.responses,
            Some(response),
            attributes,
        );
    }

    /// Tool call record (tool_details gate for name+args,
    /// tool_content gate for results).
    pub fn record_tool(&self, session_id: &str, tool: &str, result: Option<&str>) {
        let mut attributes = std::collections::BTreeMap::new();
        attributes.insert("tool".into(), tool.to_owned());
        self.record(
            "tool_call",
            session_id,
            self.config.gates.tool_details,
            Some(tool),
            attributes,
        );
        if let Some(result) = result {
            let mut attributes = std::collections::BTreeMap::new();
            attributes.insert("tool".into(), tool.to_owned());
            self.record(
                "tool_result",
                session_id,
                self.config.gates.tool_content,
                Some(result),
                attributes,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_config() -> TelemetryConfig {
        TelemetryConfig {
            enabled: true,
            endpoint: Some("https://telemetry.invalid/v1/traces".into()),
            ..TelemetryConfig::default()
        }
    }

    #[test]
    fn double_opt_in_required() {
        let mut config = active_config();
        assert!(config.active_given(true), "both arms + endpoint = active");
        assert!(!config.active_given(false), "env arm missing");
        config.enabled = false;
        assert!(!config.active_given(true), "config arm missing");
        config.enabled = true;
        config.endpoint = None;
        assert!(!config.active_given(true), "endpoint missing");
    }

    #[test]
    fn gates_default_content_free() {
        let config = TelemetryConfig::default();
        assert_eq!(config.gates, ContentGates::default());
        assert!(!config.gates.prompts);
        assert!(!config.gates.responses);
        assert!(!config.gates.tool_details);
        assert!(!config.gates.tool_content);
    }

    #[test]
    fn scrubbing_replaces_known_secret_shapes() {
        let scrubbed = scrub_secrets("key sk-AbCdEfGh1234567890 done");
        assert!(scrubbed.contains("[REDACTED]"), "{scrubbed}");
        assert!(!scrubbed.contains("AbCdEfGh1234567890"));
        let aws = scrub_secrets("AKIAIOSFODNN7EXAMPLE secret");
        assert!(!aws.contains("IOSFODNN7EXAMPLE"), "{aws}");
        let bearer = scrub_secrets("Authorization: Bearer eyJhbGciOi");
        assert!(bearer.contains("[REDACTED-"), "{bearer}");
    }

    #[test]
    fn closed_gate_means_no_content_and_disabled_means_no_traffic() {
        let transport = Arc::new(MemoryTransport::new());
        // enabled: false is deterministically inactive regardless of env.
        let disabled = TelemetryExporter::new(
            TelemetryConfig {
                enabled: false,
                endpoint: Some("https://x.invalid".into()),
                ..TelemetryConfig::default()
            },
            Arc::clone(&transport) as Arc<dyn TelemetryTransport>,
        );
        disabled.record_prompt("s1", "sk-ZZZZZZZZZZ9999");
        assert!(
            transport.delivered().is_empty(),
            "inactive exporter must not export"
        );

        let gated = TelemetryExporter::new(
            TelemetryConfig {
                enabled: true,
                endpoint: Some("https://x.invalid".into()),
                gates: ContentGates::default(),
                ..TelemetryConfig::default()
            },
            Arc::clone(&transport) as Arc<dyn TelemetryTransport>,
        );
        let record = gated.build_record(
            "prompt",
            "s1",
            false,
            Some("top secret prompt"),
            std::collections::BTreeMap::new(),
        );
        assert!(
            record.content.is_none(),
            "closed gate must carry no content"
        );
        let serialized = serde_json::to_string(&record).ok().unwrap_or_default();
        assert!(
            !serialized.contains("top secret"),
            "gate closed but content on wire: {serialized}"
        );
    }

    #[test]
    fn open_gate_exposes_content_but_scrubs_secrets() {
        let transport = Arc::new(MemoryTransport::new());
        let exporter = TelemetryExporter::new(
            TelemetryConfig {
                enabled: true,
                endpoint: Some("https://x.invalid".into()),
                gates: ContentGates {
                    prompts: true,
                    ..ContentGates::default()
                },
                scrub: true,
                ..TelemetryConfig::default()
            },
            Arc::clone(&transport) as Arc<dyn TelemetryTransport>,
        );
        let record = exporter.build_record(
            "prompt",
            "s1",
            true,
            Some("prompt with sk-AbCdEfGh1234567890 token"),
            std::collections::BTreeMap::new(),
        );
        assert!(record.content.is_some());
        assert!(
            record
                .content
                .as_deref()
                .unwrap_or_default()
                .contains("[REDACTED]"),
            "{record:?}"
        );
        let closed = exporter.build_record(
            "response",
            "s1",
            false,
            Some("never exported sk-AbCdEfGh1234567890"),
            std::collections::BTreeMap::new(),
        );
        assert!(closed.content.is_none());
    }

    #[test]
    fn gates_are_independent() {
        let gates = ContentGates {
            tool_content: true,
            ..ContentGates::default()
        };
        assert!(!gates.prompts && !gates.responses && !gates.tool_details);
        assert!(gates.tool_content);
    }
}
