//! Task-6 acceptance (austringer-skills seam): pack-sourced MCP entries
//! land in `.austringer/mcp.json` as DISABLED declarations — whatever the
//! pack claims — while user-authored entries keep their own fields.
//! The manifest shape itself stays exactly what `merge_mcp` defines.

use std::collections::BTreeMap;
use std::path::Path;

use austringer_skills::registry::{
    InstallOptions, LocalDirStore, PackFile, RegistryManager, Scope, add_pack,
};
use tempfile::TempDir;

fn install_pack_with_mcp(dir: &Path, server_value: serde_json::Value) -> TempDir {
    let registry = TempDir::new().unwrap();
    let skill_md =
        "---\nname: probe-pack\ndescription: \"Probe.\"\nversion: \"1.0.0\"\n---\n\nprobe body\n";
    let mut files = BTreeMap::new();
    files.insert("SKILL.md".to_string(), skill_md.to_string());
    let mut mcp = BTreeMap::new();
    mcp.insert("probe-server".to_string(), server_value);
    let mut pack = PackFile::empty("probe-pack", "1.0.0");
    pack.files = files;
    pack.mcp_servers = mcp;
    add_pack(registry.path(), &pack, None).expect("publish");
    let store = LocalDirStore::new(registry.path());
    RegistryManager::install(
        &store,
        "probe-pack",
        &Scope::project(dir.join("proj")),
        &InstallOptions::default(),
        &austringer_skills::registry::MapSecretSource::new(Default::default()),
    )
    .expect("install");
    registry
}

fn mcp_json(proj_root: &Path) -> serde_json::Value {
    serde_json::from_str(
        &std::fs::read_to_string(proj_root.join(".austringer").join("mcp.json"))
            .expect("mcp.json written"),
    )
    .expect("mcp.json parses")
}

#[test]
fn pack_claiming_trusted_autostart_lands_disabled() {
    let proj = TempDir::new().unwrap();
    let _registry = install_pack_with_mcp(
        proj.path(),
        serde_json::json!({
            "command": "never-starts",
            "args": [],
            "trusted_autostart": true,
        }),
    );

    let manifest = mcp_json(&proj.path().join("proj"));
    let entry = &manifest["mcpServers"]["probe-server"];
    assert!(
        entry["trusted_autostart"].is_boolean(),
        "marker materialized: {manifest}"
    );
    assert_eq!(
        entry["trusted_autostart"], false,
        "a pack may never self-enable an MCP server: {manifest}"
    );
}

#[test]
fn pack_entry_without_marker_lands_explicitly_disabled() {
    let proj = TempDir::new().unwrap();
    let _registry = install_pack_with_mcp(
        proj.path(),
        serde_json::json!({"command": "never-starts", "args": []}),
    );

    let manifest = mcp_json(&proj.path().join("proj"));
    assert_eq!(
        manifest["mcpServers"]["probe-server"]["trusted_autostart"], false,
        "absent marker becomes an explicit disabled declaration: {manifest}"
    );
}

#[test]
fn user_authored_entries_keep_their_own_autostart_fields() {
    let proj = TempDir::new().unwrap();
    let root = proj.path().join("proj");
    std::fs::create_dir_all(root.join(".austringer")).unwrap();
    std::fs::write(
        root.join(".austringer").join("mcp.json"),
        r#"{"mcpServers": {"user-server": {"command": "user-tool", "args": [], "trusted_autostart": true}}}"#,
    )
    .unwrap();

    let _registry = install_pack_with_mcp(
        proj.path(),
        serde_json::json!({"command": "never-starts", "args": []}),
    );

    let manifest = mcp_json(&root);
    assert_eq!(
        manifest["mcpServers"]["user-server"]["trusted_autostart"], true,
        "merge preserves non-pack entries untouched: {manifest}"
    );
    assert_eq!(
        manifest["mcpServers"]["probe-server"]["trusted_autostart"], false,
        "pack entry still disabled: {manifest}"
    );
}
