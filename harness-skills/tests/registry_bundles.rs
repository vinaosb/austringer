//! Plugin bundles with managed manifests (Devin §22): skills+rules+MCP+
//! subagents ship as ONE unit; requiredPlugins/optionalPlugins/
//! forbiddenPlugins with glob deny + SHA pinning + secret refs; governance
//! (higher authority wins) applies to every bundled plugin.

use harness_skills::registry::{
    Authority, GovernancePolicy, GovernanceStack, InstallOptions, ItemKind, RegistryError,
    RegistryManager, RegistryStore, Scope, blake3_hex, build_registry_from_packs_dir,
};
use tempfile::TempDir;

mod registry_common;

use registry_common::{secrets, write_file, write_pack};

const BUNDLE_JSON: &str = r#"{
  "plugins": [
    {"name": "core-lib", "pin": "__PIN__", "requirement": "required"}
  ],
  "optional": ["extra-tool"],
  "forbidden": ["telemetry-*"],
  "secret_refs": ["TEAM_TOKEN"]
}"#;

/// Same bundle, but forbidding the plugin the target project already has
/// installed (glob deny against the INSTALLED set, not the bundle's own).
const INSTALLED_CONFLICT_JSON: &str = r#"{
  "plugins": [
    {"name": "core-lib", "pin": "__PIN__", "requirement": "required"}
  ],
  "optional": [],
  "forbidden": ["extra-*"],
  "secret_refs": ["TEAM_TOKEN"]
}"#;

/// Builds a synth registry whose `team-bundle` pins the ACTUAL core-lib
/// pack hash (two-phase deterministic build).
fn bundle_server(
    base: &std::path::Path,
    bundle_json: &str,
) -> harness_skills::registry::LocalDirStore {
    let src = base.join("src");
    let manifest = r#"{
  "version": 1,
  "packs": [
    {"name": "core-lib", "category": "programming", "version": "1.0.0", "summary": "Synthetic core pack (CJK: \u6d4b\u8bd5 \ud83d\ude80)."},
    {"name": "extra-tool", "category": "programming", "version": "1.0.0", "summary": "Optional plugin."},
    {"name": "team-bundle", "category": "bundle", "version": "1.0.0", "summary": "Team unit bundle."}
  ]
}"#;
    write_file(&src.join("manifest.json"), manifest);
    write_pack(&src, "core-lib", "core lib");
    write_pack(&src, "extra-tool", "extra tool");
    write_file(
        &src.join("_shared").join("conventions.md"),
        "# Conventions\n",
    );
    write_file(
        &src.join("team-bundle").join("README.md"),
        "team bundle readme\n",
    );

    // phase 1: learn core-lib's deterministic pack hash
    let probe = base.join("probe");
    let first = build_registry_from_packs_dir(&src, &probe, None).expect("probe build");
    let pin = first.find("core-lib").expect("core-lib entry").hash.clone();

    // phase 2: write the bundle manifest with that pin, rebuild
    write_file(
        &src.join("team-bundle").join("bundle.json"),
        &bundle_json.replace("__PIN__", &pin),
    );
    let out = base.join("registry");
    build_registry_from_packs_dir(&src, &out, None).expect("final build");
    assert!(
        out.join("packs").join(format!("{pin}.hpk")).exists(),
        "core-lib hash unchanged by the bundle addition"
    );
    harness_skills::registry::LocalDirStore::new(&out)
}

#[test]
fn bundle_installs_as_one_unit_with_required_plugins_pinned() {
    let base = TempDir::new().unwrap();
    let store = bundle_server(base.path(), BUNDLE_JSON);
    let index = store.index().unwrap();
    assert_eq!(index.find("team-bundle").unwrap().hash.len(), 64);

    let proj = base.path().join("proj-work");
    let scope = Scope::project(proj.clone());
    let receipt = RegistryManager::install(
        &store,
        "team-bundle",
        &scope,
        &InstallOptions::default(),
        &secrets(&[("TEAM_TOKEN", "team-secret-1")]),
    )
    .expect("bundle install");
    assert_eq!(receipt.name, "team-bundle");
    assert_eq!(receipt.plugins_installed, vec!["core-lib".to_string()]);
    assert_eq!(receipt.optional_available, vec!["extra-tool".to_string()]);

    // the unit's own files AND the required plugin's files are on disk
    assert!(
        proj.join(".harness")
            .join("skills")
            .join("team-bundle")
            .join("README.md")
            .is_file()
    );
    assert!(
        proj.join(".harness")
            .join("skills")
            .join("core-lib")
            .join("SKILL.md")
            .is_file()
    );

    // ledger: one Bundle entry recording provenance for the whole unit
    let ledger = RegistryManager::installed(&proj).unwrap();
    assert_eq!(ledger.len(), 1, "bundle + plugins = ONE ledger entry");
    let entry = ledger
        .iter()
        .find(|e| e.name == "team-bundle")
        .expect("bundle entry");
    assert_eq!(entry.kind, ItemKind::Bundle);
    assert_eq!(entry.plugins, vec!["core-lib".to_string()]);
    assert!(
        entry
            .files
            .keys()
            .any(|k| k.starts_with("skills/core-lib/"))
    );

    // verify covers the whole unit
    assert!(RegistryManager::verify(&proj).unwrap().is_empty());
}

#[test]
fn sha_pin_mismatch_against_index_is_typed() {
    let base = TempDir::new().unwrap();
    let store = bundle_server(
        base.path(),
        &BUNDLE_JSON.replace("__PIN__", &blake3_hex(b"stale-bytes")),
    );
    let proj = base.path().join("proj-pin");
    let scope = Scope::project(proj.clone());
    match RegistryManager::install(
        &store,
        "team-bundle",
        &scope,
        &InstallOptions::default(),
        &secrets(&[("TEAM_TOKEN", "x")]),
    ) {
        Err(RegistryError::PinMismatch { plugin, .. }) => assert_eq!(plugin, "core-lib"),
        other => panic!("expected PinMismatch, got {other:?}"),
    }
    assert!(
        !proj
            .join(".harness")
            .join("skills")
            .join("core-lib")
            .exists(),
        "pinned-mismatch plugin never written"
    );
}

#[test]
fn missing_required_plugin_and_missing_secret_ref_are_typed() {
    let base = TempDir::new().unwrap();
    let ghost = r#"{
      "plugins": [{"name": "ghost-pack", "pin": "__PIN__", "requirement": "required"}],
      "optional": [], "forbidden": [], "secret_refs": []
    }"#;
    let store = bundle_server(
        base.path(),
        &ghost.replace("__PIN__", &blake3_hex(b"whatever")),
    );
    let proj = base.path().join("proj-ghost");
    let scope = Scope::project(proj.clone());
    match RegistryManager::install(
        &store,
        "team-bundle",
        &scope,
        &InstallOptions::default(),
        &secrets(&[]),
    ) {
        Err(RegistryError::MissingRequiredPlugin { plugin }) => assert_eq!(plugin, "ghost-pack"),
        other => panic!("expected MissingRequiredPlugin, got {other:?}"),
    }

    // missing secret ref (TEAM_TOKEN declared but not resolvable)
    let base2 = TempDir::new().unwrap();
    let store2 = bundle_server(base2.path(), BUNDLE_JSON);
    let proj2 = base2.path().join("proj-sec");
    let scope2 = Scope::project(proj2.clone());
    match RegistryManager::install(
        &store2,
        "team-bundle",
        &scope2,
        &InstallOptions::default(),
        &secrets(&[]),
    ) {
        Err(RegistryError::MissingSecret { name }) => assert_eq!(name, "TEAM_TOKEN"),
        other => panic!("expected MissingSecret for bundle ref, got {other:?}"),
    }
}

#[test]
fn forbidden_glob_deny_blocks_bundle_when_target_already_installed() {
    let base = TempDir::new().unwrap();
    let store = bundle_server(base.path(), INSTALLED_CONFLICT_JSON);
    let proj = base.path().join("proj-fb");
    let scope = Scope::project(proj.clone());

    // pre-install the plugin the bundle forbids ("extra-*" vs extra-tool)
    RegistryManager::install(
        &store,
        "extra-tool",
        &scope,
        &InstallOptions::default(),
        &secrets(&[("TEAM_TOKEN", "x")]),
    )
    .expect("pre-install extra-tool");

    match RegistryManager::install(
        &store,
        "team-bundle",
        &scope,
        &InstallOptions::default(),
        &secrets(&[("TEAM_TOKEN", "x")]),
    ) {
        Err(RegistryError::ForbiddenPluginConflict { plugin, pattern }) => {
            assert_eq!(plugin, "extra-tool");
            assert_eq!(pattern, "extra-*");
        }
        other => panic!("expected ForbiddenPluginConflict, got {other:?}"),
    }
}

#[test]
fn governance_applies_to_bundled_required_plugin() {
    let base = TempDir::new().unwrap();
    let store = bundle_server(base.path(), BUNDLE_JSON);
    let proj = base.path().join("proj-gov");
    let scope = Scope::project(proj.clone());
    let options = InstallOptions {
        governance: GovernanceStack::new(vec![GovernancePolicy::forbidden(
            Authority::Team,
            vec!["core-*".to_string()],
        )]),
        ..InstallOptions::default()
    };
    match RegistryManager::install(
        &store,
        "team-bundle",
        &scope,
        &options,
        &secrets(&[("TEAM_TOKEN", "x")]),
    ) {
        Err(RegistryError::GovernanceDeny { plugin, .. }) => assert_eq!(plugin, "core-lib"),
        other => panic!("expected GovernanceDeny for bundled plugin, got {other:?}"),
    }
}

#[test]
fn bundle_manifest_validation_rejects_bad_names_and_pins() {
    // unit-level shape checks
    assert!(matches!(
        harness_skills::registry::validate_pin("nothex"),
        Err(RegistryError::InvalidPin { .. })
    ));
    assert!(harness_skills::registry::validate_pin(&blake3_hex(b"x")).is_ok());

    // a bundle manifest carrying a traversal name fails pack parse
    let base = TempDir::new().unwrap();
    let store = bundle_server(base.path(), BUNDLE_JSON);
    let index = store.index().unwrap();
    let bytes = store
        .pack_bytes(&index.find("team-bundle").unwrap().hash)
        .unwrap();
    let text = String::from_utf8_lossy(&bytes).to_string();
    let evil = text.replace("\"core-lib\"", "\"../evil\"");
    match harness_skills::registry::PackFile::parse(evil.as_bytes()) {
        Err(RegistryError::InvalidName { .. }) => {}
        other => panic!("expected InvalidName from bundle manifest, got {other:?}"),
    }
}
