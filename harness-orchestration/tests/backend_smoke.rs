//! External-harness backend smoke test (plan acceptance): a mock child
//! speaking the JSONL protocol over a REAL child process, plus the
//! supply-chain surface — opt-in disabled-by-default config, SHA-256
//! release-asset verification, env redaction, per-harness plugin records.

mod common;

use std::collections::BTreeMap;
use std::time::Duration;

use harness_orchestration::backend::{
    BackendConfig, BackendKind,
    install::{DEFAULT_REDACT_PATTERNS, redact_env, verify_sha256},
    plugins::{HarnessPlugins, PluginRecord, version_gt},
    process::{BackendError, ProcessBackend},
    protocol::{PromptRequest, decode_response, encode_request},
};

/// The mock child entry: when the harness re-executes the test binary with
/// HARNESS_ORCHESTRATION_MOCK_CHILD set, THIS test becomes the protocol
/// child (reads one request line, writes one response line, exits 0).
#[test]
fn mock_backend_child() {
    let Ok(script) = std::env::var("HARNESS_ORCHESTRATION_MOCK_CHILD") else {
        return; // normal suite run: nothing to do
    };
    let mut line = String::new();
    let bytes = std::io::Read::read_to_string(&mut std::io::stdin(), &mut line).unwrap_or_default();
    assert!(bytes > 0, "child must receive a request line");
    let req: PromptRequest = serde_json::from_str(line.trim()).unwrap();
    let resp = harness_orchestration::backend::protocol::PromptResponse {
        id: req.id,
        ok: true,
        summary: format!("{script}:{}", req.objective),
        artifacts: Vec::new(),
        input_tokens: 11,
        output_tokens: 7,
    };
    let out = serde_json::to_string(&resp).unwrap();
    let mut stdout = std::io::stdout();
    use std::io::Write;
    write!(stdout, "\n{out}\n").unwrap();
}

/// SMOKE (plan acceptance): the process backend drives a mock child over
/// the official-protocol stdio framing end-to-end.
#[test]
fn backend_smoke_mock_child_protocol() {
    let exe = std::env::current_exe().unwrap();
    let mut config = BackendConfig::opt_in(exe.to_string_lossy().to_string());
    config.args = vec![
        "--exact".into(),
        "mock_backend_child".into(),
        "--nocapture".into(),
        "--test-threads=1".into(),
    ];
    let backend = ProcessBackend::new(BackendKind::ClaudeCode, config);

    let mut env = BTreeMap::new();
    env.insert(
        "HARNESS_ORCHESTRATION_MOCK_CHILD".into(),
        "claude-mock".into(),
    );

    let request = PromptRequest {
        id: 7,
        objective: "implement the launcher".into(),
        model: Some("claude-sonnet-4-6".into()),
    };
    let response = backend
        .run(&request, &env, Duration::from_secs(60))
        .expect("mock child responds over stdio");
    assert_eq!(response.id, 7, "response carries the request id");
    assert!(response.ok);
    assert_eq!(response.summary, "claude-mock:implement the launcher");
    assert_eq!(response.input_tokens, 11);
    assert_eq!(response.output_tokens, 7);
}

/// SUPPLY-CHAIN CARVE-OUT: backends are disabled by default; a disabled
/// config refuses loudly without spawning anything.
#[test]
fn backends_are_opt_in_and_fail_loud_when_disabled() {
    let config = BackendConfig::default();
    assert!(
        !config.enabled,
        "disabled by default (supply-chain carve-out)"
    );
    let backend = ProcessBackend::new(BackendKind::Codex, config);
    let err = backend
        .run(
            &PromptRequest {
                id: 1,
                objective: "x".into(),
                model: None,
            },
            &BTreeMap::new(),
            Duration::from_secs(5),
        )
        .unwrap_err();
    assert!(
        matches!(err, BackendError::Disabled),
        "loud Disabled error: {err:?}"
    );
}

/// AUTO-INSTALL GATE: release assets verify against a pinned SHA-256.
#[test]
fn sha256_verification() {
    // sha256("harness") known digest.
    let digest = {
        use sha2::Digest;
        let mut hasher = sha2::Sha256::new();
        hasher.update(b"harness");
        let out = hasher.finalize();
        out.iter().map(|b| format!("{b:02x}")).collect::<String>()
    };
    assert!(verify_sha256(b"harness", &digest));
    assert!(
        verify_sha256(b"harness", &digest.to_uppercase()),
        "hex is case-insensitive"
    );
    assert!(!verify_sha256(b"harnesS", &digest), "wrong bytes fail");
    assert!(
        !verify_sha256(b"harness", "zz"),
        "malformed digest fails closed"
    );
}

/// ENV REDACTION: secrets never reach child processes.
#[test]
fn env_redaction_strips_secrets() {
    let mut env = BTreeMap::new();
    env.insert("ANTHROPIC_API_KEY".into(), "sk-secret".into());
    env.insert("OPENAI_API_KEY".into(), "sk-other".into());
    env.insert("PATH".into(), "C:\\bin".into());
    let redacted = redact_env(&env, DEFAULT_REDACT_PATTERNS);
    assert_eq!(redacted.len(), 1, "only PATH survives");
    assert_eq!(redacted.get("PATH").map(String::as_str), Some("C:\\bin"));
}

/// PER-HARNESS PLUGIN MANAGERS: plugin records are version-checked per
/// external CLI agent and pinned by content hash.
#[test]
fn per_harness_plugin_records() {
    let mut plugins = HarnessPlugins::new(BackendKind::ClaudeCode);
    plugins
        .register(PluginRecord {
            name: "review-bot".into(),
            version: "1.2.0".into(),
            sha256: "a1b2".into(),
        })
        .unwrap();
    assert!(
        plugins.needs_update("review-bot", "1.3.0"),
        "marketplace version outruns the installed record"
    );
    assert!(!plugins.needs_update("review-bot", "1.2.0"));
    assert!(
        !plugins.needs_update("missing", "9.9.9"),
        "absent plugins do not demand updates"
    );

    assert!(version_gt("1.3.0", "1.2.9"));
    assert!(version_gt("2.0", "1.9.9"));
    assert!(!version_gt("1.2.0", "1.2.0"));

    assert!(!HarnessPlugins::verify_plugin(b"plugin-bytes", "a1b2"));

    let encoded = encode_request(&PromptRequest {
        id: 3,
        objective: "roundtrip".into(),
        model: None,
    });
    assert_eq!(encoded.matches('\n').count(), 0, "single-line framing");
    assert!(
        encoded.contains("\"id\":3"),
        "stable sorted-key JSON: {encoded}"
    );
    let response_line =
        serde_json::to_string(&harness_orchestration::backend::protocol::PromptResponse {
            id: 3,
            ok: true,
            summary: "s".into(),
            artifacts: Vec::new(),
            input_tokens: 1,
            output_tokens: 1,
        })
        .unwrap();
    let decoded = decode_response(&response_line).unwrap();
    assert_eq!(decoded.id, 3);
    assert!(
        decode_response("not json").is_err(),
        "garbage lines are typed errors"
    );
}

/// A dead child surfaces a typed spawn/exit error, never a hang.
#[test]
fn backend_dead_command_is_typed_error() {
    let mut config = BackendConfig::opt_in("definitely-not-a-real-command-xyz".into());
    config.args = vec![];
    let backend = ProcessBackend::new(BackendKind::Codex, config);
    let err = backend
        .run(
            &PromptRequest {
                id: 1,
                objective: "x".into(),
                model: None,
            },
            &BTreeMap::new(),
            Duration::from_secs(10),
        )
        .unwrap_err();
    assert!(
        matches!(err, BackendError::Spawn { .. }),
        "typed spawn failure: {err:?}"
    );
}
