//! T29 file-based multi-provider MCP manager: scans provider config files
//! (.austringer JSON+TOML, .claude, .cursor, codeium-style), one bad file
//! never kills the scan (per-path diagnostics), `{{secret:…}}`
//! substitution flows from a SecretStore seam (never into config files),
//! and autostart is auth-gated/deferred unless explicitly trusted.

use std::collections::BTreeMap;
use std::path::PathBuf;

use austringer_mcp::config::{ServerEntry, TransportSpec};
use austringer_mcp::manager::{
    AutostartDecision, ProviderKind, ScanReport, TemplateRegistry, scan_directories, scan_file,
};
use austringer_mcp::secret::{MapSecretStore, SecretStore, substitute_secrets};
use tempfile::TempDir;

fn write(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(path, contents).ok();
}

fn dot_harness(dir: &TempDir) -> PathBuf {
    dir.path().join(".austringer")
}

#[test]
fn scans_austringer_json_and_toml() {
    let dir = TempDir::new().expect("temp");
    write(
        &dot_harness(&dir).join("mcp.json"),
        r#"{"mcpServers": {"fs": {"command": "npx", "args": ["-y", "mcp-fs"], "roots": ["C:/repo"]}}}"#,
    );
    write(
        &dot_harness(&dir).join("mcp.toml"),
        r#"[mcpServers.cloud]
url = "https://mcp.example.com/rpc"
"#,
    );
    let report = scan_directories(&[dir.path().to_path_buf()]);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let names: Vec<(&str, ProviderKind)> = report
        .installations
        .iter()
        .map(|install| (install.entry.name.as_str(), install.provider))
        .collect();
    assert!(names.contains(&("fs", ProviderKind::Austringer)));
    assert!(names.contains(&("cloud", ProviderKind::Austringer)));
    let fs = report
        .installations
        .iter()
        .find(|install| install.entry.name == "fs")
        .expect("fs entry");
    assert_eq!(
        fs.entry.transport,
        TransportSpec::Stdio {
            command: "npx".to_owned(),
            args: vec!["-y".to_owned(), "mcp-fs".to_owned()],
        }
    );
}

#[test]
fn scans_claude_cursor_and_codeium_shapes() {
    let dir = TempDir::new().expect("temp");
    write(
        &dir.path().join(".mcp.json"),
        r#"{"mcpServers": {"claude-fs": {"command": "npx", "args": ["claude-server"]}}}"#,
    );
    let cursor = dir.path().join(".cursor");
    write(
        &cursor.join("mcp.json"),
        r#"{"mcpServers": {"cursor-db": {"command": "uvx", "args": ["mcp-db"]}}}"#,
    );
    let codeium = dir
        .path()
        .join("codeium")
        .join("config")
        .join("config.json");
    write(
        &codeium,
        r#"{"experimental": {"mcpServers": {"codeium-x": {"url": "https://x.example/mcp"}}}}"#,
    );
    let report = scan_directories(&[dir.path().to_path_buf()]);
    let by_provider: Vec<(&str, ProviderKind)> = report
        .installations
        .iter()
        .map(|install| (install.entry.name.as_str(), install.provider))
        .collect();
    assert!(by_provider.contains(&("claude-fs", ProviderKind::Claude)));
    assert!(by_provider.contains(&("cursor-db", ProviderKind::Cursor)));
    assert!(by_provider.contains(&("codeium-x", ProviderKind::Codeium)));
}

#[test]
fn one_bad_file_never_kills_the_scan() {
    let dir = TempDir::new().expect("temp");
    write(
        &dot_harness(&dir).join("mcp.json"),
        r#"{"mcpServers": {"good": {"command": "npx"}}}"#,
    );
    let servers_dir = dot_harness(&dir).join("mcp-servers");
    let half_json = servers_dir.join("broken.json");
    write(&half_json, r#"{"mcpServers": {"bad": {"command": "npx""#);
    write(
        &servers_dir.join("wrong.toml"),
        "this is = not [ valid toml",
    );
    write(
        &servers_dir.join("weird.json"),
        r#"{"mcpServers": {"bad\u0007name": {"command": "npx"}}}"#,
    );
    let report = scan_directories(&[dir.path().to_path_buf()]);
    assert!(
        report
            .installations
            .iter()
            .any(|install| install.entry.name == "good"),
        "good entry must survive: {:?}",
        report.installations
    );
    assert_eq!(report.diagnostics.len(), 3, "{:?}", report.diagnostics);
    let paths: Vec<String> = report
        .diagnostics
        .iter()
        .map(|d| d.path.to_string_lossy().into_owned())
        .collect();
    assert!(paths.iter().any(|path| path.contains("broken.json")));
    assert!(paths.iter().any(|path| path.contains("wrong.toml")));
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.message.contains("control")),
        "{:?}",
        report.diagnostics
    );
}

#[test]
fn secret_substitution_and_never_in_files() {
    let mut secrets: BTreeMap<String, String> = BTreeMap::new();
    secrets.insert("API_TOKEN".to_owned(), "sk-super-secret".to_owned());
    let store = MapSecretStore::new(secrets);
    let dir = TempDir::new().expect("temp");
    let template_path = dot_harness(&dir).join("template.json");
    write(
        &template_path,
        r#"{"mcpServers": {"cloud": {"command": "mcp-cloud", "args": ["--token={{secret:API_TOKEN}}"]}}}"#,
    );
    let report = scan_file(&template_path, ProviderKind::Austringer);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let install = &report.installations[0];
    assert!(
        install.entry.args_contain_placeholder(),
        "raw scan keeps the placeholder"
    );
    let resolved = substitute_secrets(
        &serde_json::to_string(&install.entry.transport).unwrap_or_default(),
        &store,
    )
    .expect("substitution");
    assert!(resolved.contains("sk-super-secret"));
    let on_disk = std::fs::read_to_string(&template_path).unwrap_or_default();
    assert!(on_disk.contains("{{secret:API_TOKEN}}"));
    assert!(!on_disk.contains("sk-super-secret"));
}

#[test]
fn missing_secret_fails_closed_without_leaking_values() {
    let store = MapSecretStore::new(BTreeMap::new());
    let outcome = substitute_secrets("token={{secret:MISSING}}", &store);
    let error = outcome.expect_err("missing secret must fail");
    assert!(error.to_string().contains("MISSING"));
    assert!(!error.to_string().contains("sk-"));
}

#[test]
fn template_registry_instantiates_hash_keyed() {
    let dir = TempDir::new().expect("temp");
    let _ = dir;
    let mut template = ServerEntry::new(
        "cloud".to_owned(),
        TransportSpec::Stdio {
            command: "mcp-cloud".to_owned(),
            args: vec!["--token={{secret:API_TOKEN}}".to_owned()],
        },
    );
    template.roots.push(PathBuf::from("C:/work"));
    let mut registry = TemplateRegistry::new();
    let key = registry.install(template);
    assert_ne!(key.0, "");

    let mut secrets: BTreeMap<String, String> = BTreeMap::new();
    secrets.insert("API_TOKEN".to_owned(), "sk-a".to_owned());
    let store = MapSecretStore::new(secrets);
    let first = registry.instantiate(&key, &store).expect("instantiate");
    let second = registry
        .instantiate(&key, &store)
        .expect("instantiate again");
    assert_eq!(first.fingerprint(), second.fingerprint());
    assert!(
        format!("{:?}", first.transport).contains("sk-a"),
        "resolved entry carries the secret"
    );
    let error = registry
        .instantiate(&key, &MapSecretStore::new(BTreeMap::new()))
        .expect_err("missing secret must not instantiate");
    assert!(error.to_string().contains("API_TOKEN"));
}

#[test]
fn autostart_is_deferred_unless_trusted() {
    let mut entry = ServerEntry::new(
        "fs".to_owned(),
        TransportSpec::Stdio {
            command: "npx".to_owned(),
            args: vec![],
        },
    );
    assert_eq!(
        ScanReport::autostart_for(&entry),
        AutostartDecision::DeferredOnFirstUse
    );
    entry.trusted_autostart = true;
    assert_eq!(
        ScanReport::autostart_for(&entry),
        AutostartDecision::StartNow
    );
}

#[test]
fn unicode_and_env_quirks_parse() {
    let dir = TempDir::new().expect("temp");
    write(
        &dot_harness(&dir).join("mcp.json"),
        r#"{"mcpServers": {"ファイル": {"command": "npx", "args": ["--名前=値"], "env": {"WEIRD": "a=b c", "EMPTY": ""}}}}"#,
    );
    let report = scan_directories(&[dir.path().to_path_buf()]);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let entry = &report.installations[0].entry;
    assert_eq!(entry.name, "ファイル");
    assert_eq!(entry.env.get("WEIRD").map(String::as_str), Some("a=b c"));
    assert_eq!(entry.env.get("EMPTY").map(String::as_str), Some(""));
}

trait ArgsContainPlaceholder {
    fn args_contain_placeholder(&self) -> bool;
}

impl ArgsContainPlaceholder for ServerEntry {
    fn args_contain_placeholder(&self) -> bool {
        match &self.transport {
            TransportSpec::Stdio { args, .. } => args.iter().any(|arg| arg.contains("{{secret:")),
            TransportSpec::Http { .. } => false,
        }
    }
}

pub fn _unused_store_witness(store: &dyn SecretStore) -> Option<String> {
    store.resolve("x")
}
