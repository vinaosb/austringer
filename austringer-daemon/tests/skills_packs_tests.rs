//! Task-6 acceptance (plan todo 6): the skills runtime — startup load
//! through the real SkillLoader, tier-gated /skill invocation,
//! trigger-driven refusals in a second session, and the GET /packs
//! surface. Fixture packs on disk in a temp data dir; zero network,
//! zero keys, zero spawned processes.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use austringer_core::agent_loop::Inference;
use austringer_daemon::config::DaemonConfig;
use austringer_daemon::server::{DaemonState, build_router};

mod common;

use common::CompleteOnlyInference;

fn write_file(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
    std::fs::write(path, content).expect("write file");
}

/// One fixture SKILL.md on disk under `{dir}/.austringer/skills/{name}`.
fn write_skill(dir: &Path, name: &str, trigger: &str) {
    let skill = format!(
        "---\nname: {name}\ndescription: \"Fixture {name} pack.\"\nversion: \"1.0.0\"\ntriggers:\n  - {trigger}\n---\n\nfixture skill body marker {name}-body\n"
    );
    write_file(
        &dir.join(".austringer")
            .join("skills")
            .join(name)
            .join("SKILL.md"),
        &skill,
    );
}

async fn spawn_in(dir: &Path) -> (String, String, Arc<DaemonState>) {
    let config = DaemonConfig {
        data_dir: dir.to_owned(),
        token: Some("e2e-token".into()),
        durable: false,
        ..DaemonConfig::default()
    };
    let factory: Arc<dyn Fn() -> Box<dyn Inference> + Send + Sync> =
        Arc::new(|| Box::new(CompleteOnlyInference));
    let state =
        Arc::new(DaemonState::new(config, factory).unwrap_or_else(|e| panic!("state: {e}")));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let app = build_router(Arc::clone(&state));
    tokio::spawn(async move {
        let serve = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        );
        let _ = serve.await;
    });
    (format!("127.0.0.1:{port}"), state.token.clone(), state)
}

async fn get(base: &str, token: &str, path: &str) -> (u16, serde_json::Value) {
    let response = reqwest::Client::new()
        .get(format!("http://{base}{path}"))
        .bearer_auth(token)
        .send()
        .await
        .expect("http");
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    let json = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
    (status, json)
}

async fn post_submit(base: &str, token: &str, session: &str, text: &str) -> (u16, String) {
    let body = serde_json::json!({
        "type": "command", "command": {"type": "submit", "text": text},
    });
    let response = reqwest::Client::new()
        .post(format!("http://{base}/s/{session}/cmd"))
        .bearer_auth(token)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send()
        .await
        .expect("http");
    let status = response.status().as_u16();
    let text = response.text().await.unwrap_or_default();
    (status, text)
}

/// Fixture data dir: one approved-tier pack, one quarantined (draft,
/// absent from pack-tiers.json — fail-closed default).
fn fixture_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("pack-tiers.json"),
        br#"{"trusted-pack": "approved"}"#,
    )
    .expect("pack tiers");
    write_skill(dir.path(), "trusted-pack", "ship the widget");
    write_skill(dir.path(), "draft-pack", "run the quarantined helper");
    dir
}

#[tokio::test]
async fn packs_endpoint_lists_tiers_and_trust_status() {
    let dir = fixture_dir();
    let (base, token, _state) = spawn_in(dir.path()).await;

    let (status, body) = get(&base, &token, "/packs").await;
    assert_eq!(status, 200, "{body}");
    let packs = body["packs"]
        .as_array()
        .unwrap_or_else(|| panic!("packs array: {body}"));

    let trusted = packs
        .iter()
        .find(|p| p["name"] == "trusted-pack")
        .unwrap_or_else(|| panic!("trusted-pack listed: {body}"));
    assert_eq!(trusted["tier"], "approved");
    assert_eq!(trusted["trusted"], true);

    let draft = packs
        .iter()
        .find(|p| p["name"] == "draft-pack")
        .unwrap_or_else(|| panic!("quarantined pack must be LISTED: {body}"));
    assert_eq!(draft["tier"], "draft");
    assert_eq!(draft["trusted"], false);
}

#[tokio::test]
async fn packs_endpoint_requires_auth() {
    let dir = fixture_dir();
    let (base, _token, _state) = spawn_in(dir.path()).await;

    let response = reqwest::Client::new()
        .get(format!("http://{base}/packs"))
        .send()
        .await
        .expect("http");
    assert_eq!(response.status().as_u16(), 401, "rides the auth layer");
}

#[tokio::test]
async fn quarantined_pack_refuses_skill_invocation_with_tier() {
    let dir = fixture_dir();
    let (base, token, _state) = spawn_in(dir.path()).await;

    let (status, text) = post_submit(&base, &token, "main", "/skill draft-pack").await;
    assert_eq!(status, 400, "typed refusal, not acceptance: {text}");
    assert!(
        text.contains("draft-tier"),
        "refusal carries the quarantine tier: {text}"
    );
    assert!(
        text.contains("draft-pack"),
        "refusal names the pack: {text}"
    );
}

#[tokio::test]
async fn trusted_pack_skill_invocation_injects_context() {
    let dir = fixture_dir();
    let (base, token, state) = spawn_in(dir.path()).await;

    let (status, text) = post_submit(&base, &token, "inject", "/skill trusted-pack ship it").await;
    assert_eq!(status, 202, "trusted pack invocable: {text}");

    // The injected turn carries the skill body (progressive-disclosure
    // invoke) plus the caller's args.
    let host = state.session("inject");
    let seen = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let rendered = host
                .snapshot()
                .iter()
                .map(|frame| serde_json::to_string(frame).unwrap_or_default())
                .collect::<Vec<_>>()
                .join("\n");
            if rendered.contains("trusted-pack-body") && rendered.contains("ship it") {
                return rendered;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(seen.is_ok(), "skill context never reached the event stream");
}

#[tokio::test]
async fn quarantined_trigger_in_second_session_refuses_like_skill_path() {
    let dir = fixture_dir();
    let (base, token, _state) = spawn_in(dir.path()).await;

    // The /skill path refusal (session 1)…
    let (skill_status, skill_text) = post_submit(&base, &token, "first", "/skill draft-pack").await;
    assert_eq!(skill_status, 400);

    // …and the TRIGGER path refusal in a SECOND fixture session: the
    // gates govern trigger-driven activation, not just the command.
    let (trigger_status, trigger_text) = post_submit(
        &base,
        &token,
        "child-delegated",
        "please run the quarantined helper now",
    )
    .await;
    assert_eq!(
        trigger_status, 400,
        "trigger match on a quarantined skill must refuse, not submit: {trigger_text}"
    );
    assert!(
        trigger_text.contains("draft-tier") && trigger_text.contains("draft-pack"),
        "same typed refusal carrying tier + name: {trigger_text}"
    );
    assert_eq!(
        skill_text, trigger_text,
        "identical typed refusal on both activation paths"
    );
}

/// Builds a fixture registry dir holding one pack that carries an MCP
/// entry ADVERSARIALLY marked `trusted_autostart: true` — the install
/// path must still land it disabled.
fn fixture_registry_with_mcp_pack(
    registry_dir: &Path,
) -> austringer_skills::registry::LocalDirStore {
    use austringer_skills::registry::{LocalDirStore, PackFile, add_pack};
    use std::collections::BTreeMap;

    let skill_md = "---\nname: mcp-pack\ndescription: \"Fixture MCP pack.\"\nversion: \"1.0.0\"\n---\n\nfixture mcp pack body\n";
    let mut files = BTreeMap::new();
    files.insert("SKILL.md".to_string(), skill_md.to_string());
    let mut mcp = BTreeMap::new();
    mcp.insert(
        "pack-server".to_string(),
        serde_json::json!({
            "command": "definitely-never-run",
            "args": [],
            "trusted_autostart": true,
        }),
    );
    let mut pack = PackFile::empty("mcp-pack", "1.0.0");
    pack.files = files;
    pack.mcp_servers = mcp;
    add_pack(registry_dir, &pack, None).expect("publish fixture pack");
    LocalDirStore::new(registry_dir)
}

#[tokio::test]
async fn mcp_pack_install_lands_disabled_declaration_and_renders_disabled() {
    use austringer_skills::registry::{InstallOptions, RegistryManager, Scope};

    let registry = tempfile::tempdir().unwrap();
    let store = fixture_registry_with_mcp_pack(registry.path());
    let dir = tempfile::tempdir().unwrap();

    let receipt = RegistryManager::install(
        &store,
        "mcp-pack",
        &Scope::project(dir.path().to_path_buf()),
        &InstallOptions::default(),
        &austringer_skills::registry::MapSecretSource::new(Default::default()),
    )
    .expect("offline install");
    assert_eq!(receipt.name, "mcp-pack");

    // The declaration is in the executable manifest — but DISABLED:
    // the pack's `trusted_autostart: true` claim must not survive.
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join(".austringer").join("mcp.json"))
            .expect("mcp.json written"),
    )
    .expect("mcp.json parses");
    let entry = &manifest["mcpServers"]["pack-server"];
    assert!(entry.is_object(), "declaration present: {manifest}");
    assert_eq!(
        entry["trusted_autostart"], false,
        "pack MCP entries land DISABLED: {manifest}"
    );

    // Composition with the MCP manager (todo 7's renderer): the scan
    // finds the declaration and the autostart decision is DEFERRED —
    // no consumer may launch it. Install/invoke spawn NO process.
    let scan = austringer_mcp::manager::scan_directories(&[dir.path().to_path_buf()]);
    assert!(scan.diagnostics.is_empty(), "{:?}", scan.diagnostics);
    let installation = scan
        .installations
        .iter()
        .find(|i| i.entry.name == "pack-server")
        .unwrap_or_else(|| panic!("scanner sees pack-server: {:?}", scan.installations));
    assert_eq!(
        austringer_mcp::manager::ScanReport::autostart_for(&installation.entry),
        austringer_mcp::manager::AutostartDecision::DeferredOnFirstUse,
        "the disabled marker must compose with autostart_for"
    );

    // GET /packs renders the entry with a disabled marker.
    let (base, token, _state) = spawn_in(dir.path()).await;
    let (status, body) = get(&base, &token, "/packs").await;
    assert_eq!(status, 200, "{body}");
    let servers = body["mcp_servers"]
        .as_array()
        .unwrap_or_else(|| panic!("mcp_servers array: {body}"));
    let rendered = servers
        .iter()
        .find(|s| s["server"] == "pack-server")
        .unwrap_or_else(|| panic!("pack-server rendered: {body}"));
    assert_eq!(rendered["disabled"], true, "disabled marker: {body}");

    // The pack's skill surface is listed too (draft-tier: not in
    // pack-tiers.json).
    let packs = body["packs"].as_array().expect("packs");
    assert!(
        packs
            .iter()
            .any(|p| p["name"] == "mcp-pack" && p["tier"] == "draft"),
        "installed pack listed with its tier: {body}"
    );
}
