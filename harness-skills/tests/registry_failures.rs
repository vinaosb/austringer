//! Failure channels (plan todo 32 acceptance): tampered pack => typed
//! hash-mismatch rejection with NOTHING written; ledger drift + diff show
//! tamper; forbidden-plugin glob deny + forbid-'*' lockdown; bad/missing
//! signatures; corrupt pack/index; missing secrets; untrusted-text
//! validation of names/versions/paths.

use harness_skills::registry::{
    Blake3Verifier, Drift, GovernancePolicy, GovernanceStack, InstallOptions, PackFile,
    RegistryError, RegistryIndex, RegistryManager, RegistryStore, add_pack, blake3_hex,
};
use serde_json::json;
use tempfile::TempDir;

mod registry_common;

use registry_common::{key, project, real_server, secrets};

fn install(
    store: &dyn harness_skills::registry::RegistryStore,
    name: &str,
    proj_root: &std::path::Path,
) -> Result<harness_skills::registry::InstallReceipt, RegistryError> {
    let scope = harness_skills::registry::Scope::project(proj_root.to_path_buf());
    RegistryManager::install(
        store,
        name,
        &scope,
        &InstallOptions::default(),
        &secrets(&[]),
    )
}

#[test]
fn tampered_pack_file_is_rejected_by_hash_with_nothing_written() {
    let server = TempDir::new().unwrap();
    let proj = TempDir::new().unwrap();
    let store = real_server(server.path(), None);

    let entry = store.index().unwrap().find("dotnet").unwrap().clone();
    let pack_path = server
        .path()
        .join("packs")
        .join(format!("{}.hpk", entry.hash));
    let original = std::fs::read(&pack_path).unwrap();
    let mut tampered = original.clone();
    tampered.extend_from_slice(b"/* tampered */");
    std::fs::write(&pack_path, tampered).unwrap();

    match install(&store, "dotnet", proj.path()) {
        Err(RegistryError::HashMismatch {
            name,
            expected,
            actual,
        }) => {
            assert_eq!(name, "dotnet");
            assert_eq!(
                expected, entry.hash,
                "expected = index pin (original bytes)"
            );
            let served = store.pack_bytes(&entry.hash).unwrap();
            assert_eq!(
                actual,
                blake3_hex(&served),
                "actual = hash of the served (tampered) bytes"
            );
        }
        other => panic!("expected HashMismatch, got {other:?}"),
    }

    // MISLEADING-SUCCESS guard: nothing was installed
    assert!(
        !proj
            .path()
            .join(".harness")
            .join("skills")
            .join("dotnet")
            .exists()
    );
    assert!(
        RegistryManager::installed(proj.path()).unwrap().is_empty(),
        "no ledger entry for a rejected install"
    );
}

#[test]
fn ledger_diff_shows_on_disk_tamper_after_clean_install() {
    let server = TempDir::new().unwrap();
    let proj = TempDir::new().unwrap();
    let store = real_server(server.path(), None);
    install(&store, "dotnet", proj.path()).expect("clean install");

    let skill_md = proj
        .path()
        .join(".harness")
        .join("skills")
        .join("dotnet")
        .join("SKILL.md");
    let original = std::fs::read_to_string(&skill_md).unwrap();
    std::fs::write(&skill_md, format!("{original}\n/* TAMPERED */")).unwrap();

    // verify(): per-file drift, typed and located
    let drift = RegistryManager::verify(proj.path()).unwrap();
    match drift.first() {
        Some(Drift::Modified { name, key, .. }) => {
            assert_eq!(name, "dotnet");
            assert_eq!(key, "skills/dotnet/SKILL.md");
        }
        other => panic!("expected Modified drift, got {other:?}"),
    }

    // diff_disk(): ledger vs disk diff row with was/now hashes
    let diff = RegistryManager::diff_disk(proj.path()).unwrap();
    assert_eq!(diff.changed.len(), 1, "exactly one changed entry");
    let changed = &diff.changed[0];
    assert_eq!(changed.name, "dotnet");
    assert_eq!(changed.files_changed.len(), 1);
    let fc = &changed.files_changed[0];
    assert_eq!(fc.key, "skills/dotnet/SKILL.md");
    let expected_was = blake3_hex(original.as_bytes());
    assert_eq!(fc.was.as_deref(), Some(expected_was.as_str()));
    assert_ne!(fc.was, fc.now);

    // restoring the file clears the drift
    std::fs::write(&skill_md, &original).unwrap();
    assert!(RegistryManager::verify(proj.path()).unwrap().is_empty());
}

#[test]
fn unexpected_files_inside_a_pack_dir_are_drift() {
    let server = TempDir::new().unwrap();
    let proj = TempDir::new().unwrap();
    let store = real_server(server.path(), None);
    install(&store, "devops", proj.path()).expect("clean install");

    let extra = proj
        .path()
        .join(".harness")
        .join("skills")
        .join("devops")
        .join("evil.md");
    std::fs::write(&extra, "added behind the ledger's back").unwrap();

    let drift = RegistryManager::verify(proj.path()).unwrap();
    match drift.iter().find(|d| matches!(d, Drift::Unexpected { .. })) {
        Some(Drift::Unexpected { name, key }) => {
            assert_eq!(name, "devops");
            assert!(key.ends_with("evil.md"));
        }
        other => panic!("expected Unexpected drift, got {other:?}"),
    }
}

#[test]
fn governance_glob_deny_and_star_lockdown_are_typed_rejections() {
    let server = TempDir::new().unwrap();
    let store = real_server(server.path(), None);

    // glob deny: org forbids "dot*" at the Org layer
    let options = InstallOptions {
        governance: GovernanceStack::new(vec![GovernancePolicy::forbidden(
            harness_skills::registry::Authority::Org,
            vec!["dot*".to_string()],
        )]),
        ..InstallOptions::default()
    };
    let proj = TempDir::new().unwrap();
    let scope = project(proj.path());
    match RegistryManager::install(&store, "dotnet", &scope, &options, &secrets(&[])) {
        Err(RegistryError::GovernanceDeny {
            plugin,
            by,
            pattern,
        }) => {
            assert_eq!(plugin, "dotnet");
            assert_eq!(pattern, "dot*");
            assert_eq!(by, harness_skills::registry::Authority::Org);
        }
        other => panic!("expected GovernanceDeny, got {other:?}"),
    }
    assert!(
        !proj
            .path()
            .join("proj")
            .join(".harness")
            .join("skills")
            .join("dotnet")
            .exists()
    );

    // forbid-'*' lockdown: nothing installable at any lower authority
    let lock = InstallOptions {
        governance: GovernanceStack::new(vec![GovernancePolicy::forbidden(
            harness_skills::registry::Authority::Org,
            vec!["*".to_string()],
        )]),
        ..InstallOptions::default()
    };
    let proj2 = TempDir::new().unwrap();
    let scope2 = project(proj2.path());
    match RegistryManager::install(&store, "legal", &scope2, &lock, &secrets(&[])) {
        Err(RegistryError::Lockdown { .. }) => {}
        other => panic!("expected Lockdown, got {other:?}"),
    }

    // higher authority wins: org forbids '*', but org itself requires legal
    let rescue = InstallOptions {
        governance: GovernanceStack::new(vec![
            GovernancePolicy::forbidden(
                harness_skills::registry::Authority::Org,
                vec!["*".to_string()],
            ),
            GovernancePolicy::required(
                harness_skills::registry::Authority::Org,
                vec!["legal".to_string()],
            ),
        ]),
        ..lock.clone()
    };
    // highest matching layer (the require list is evaluated at the SAME
    // authority as the forbid) => forbidden('*') still matches first only
    // if ordered first; required at equal authority is a conflict the
    // stack reports as lockdown-preserving deny.
    let proj3 = TempDir::new().unwrap();
    let scope3 = project(proj3.path());
    match RegistryManager::install(&store, "legal", &scope3, &rescue, &secrets(&[])) {
        Err(RegistryError::GovernanceDeny { .. }) => {}
        other => panic!("same-authority conflict stays denied, got {other:?}"),
    }
}

#[test]
fn bad_signature_and_required_signature_are_typed() {
    let server = TempDir::new().unwrap();
    let k = key();
    let store = real_server(server.path(), Some(&k));
    let verifier = Blake3Verifier::new(&k);

    // corrupt the signature in the index
    let mut index = store.index().unwrap();
    index
        .entries
        .iter_mut()
        .find(|e| e.name == "dotnet")
        .unwrap()
        .signature = Some("deadbeef".to_string());
    let index_path = server.path().join("index.json");
    std::fs::write(&index_path, RegistryIndex::to_bytes(&index)).unwrap();

    let proj = TempDir::new().unwrap();
    let scope = project(proj.path());
    let options = InstallOptions {
        verifier: Some(&verifier),
        ..InstallOptions::default()
    };
    match RegistryManager::install(&store, "dotnet", &scope, &options, &secrets(&[])) {
        Err(RegistryError::SignatureInvalid { name, .. }) => assert_eq!(name, "dotnet"),
        other => panic!("expected SignatureInvalid, got {other:?}"),
    }

    // require a signature from an UNSIGNED registry
    let plain_server = TempDir::new().unwrap();
    let plain = real_server(plain_server.path(), None);
    let strict = InstallOptions {
        require_signature: true,
        verifier: Some(&verifier),
        ..InstallOptions::default()
    };
    let proj2 = TempDir::new().unwrap();
    let scope2 = project(proj2.path());
    match RegistryManager::install(&plain, "dotnet", &scope2, &strict, &secrets(&[])) {
        Err(RegistryError::SignatureMissing { name, .. }) => assert_eq!(name, "dotnet"),
        other => panic!("expected SignatureMissing, got {other:?}"),
    }
}

#[test]
fn corrupt_pack_json_with_valid_hash_is_typed_pack_corrupt() {
    let server = TempDir::new().unwrap();
    let store = real_server(server.path(), None);
    let mut index = store.index().unwrap();
    let entry = index
        .entries
        .iter_mut()
        .find(|e| e.name == "dotnet")
        .unwrap();
    let garbage = b"{ this is not a pack file".to_vec();
    let new_hash = blake3_hex(&garbage);
    std::fs::write(
        server
            .path()
            .join("packs")
            .join(format!("{}.hpk", new_hash)),
        &garbage,
    )
    .unwrap();
    entry.hash = new_hash.clone();
    entry.size = garbage.len() as u64;
    std::fs::write(
        server.path().join("index.json"),
        RegistryIndex::to_bytes(&index),
    )
    .unwrap();

    let proj = TempDir::new().unwrap();
    match install(&store, "dotnet", proj.path()) {
        Err(RegistryError::PackCorrupt { detail }) => assert!(!detail.is_empty()),
        other => panic!("expected PackCorrupt, got {other:?}"),
    }
}

#[test]
fn pack_size_is_capped_before_parse() {
    match PackFile::check_size(harness_skills::registry::MAX_PACK_BYTES + 1) {
        Err(RegistryError::PackTooLarge { .. }) => {}
        other => panic!("expected PackTooLarge, got {other:?}"),
    }
    assert!(PackFile::check_size(1024).is_ok());
}

#[test]
fn missing_secret_fails_closed_and_values_never_enter_config_files() {
    let server = TempDir::new().unwrap();
    let mut pack = PackFile::empty("acme-mcp", "1.0.0");
    pack.summary = "Declarative MCP marketplace item.".to_string();
    pack.secret_params = vec![harness_skills::registry::SecretParam {
        name: "ACME_TOKEN".to_string(),
        prompt: "Paste your Acme API token".to_string(),
    }];
    pack.mcp_servers.insert(
        "acme".to_string(),
        json!({"command": "npx", "args": ["-y", "acme-mcp"], "env": {"TOKEN": "{{secret:ACME_TOKEN}}"}}),
    );
    let entry = add_pack(server.path(), &pack, None).unwrap();
    let store = harness_skills::registry::LocalDirStore::new(server.path());
    assert_eq!(entry.name, "acme-mcp");

    // missing secret => typed failure naming the SECRET (never a value)
    let proj = TempDir::new().unwrap();
    let scope = project(proj.path());
    match RegistryManager::install(
        &store,
        "acme-mcp",
        &scope,
        &InstallOptions::default(),
        &secrets(&[]),
    ) {
        Err(RegistryError::MissingSecret { name }) => assert_eq!(name, "ACME_TOKEN"),
        other => panic!("expected MissingSecret, got {other:?}"),
    }
    assert!(
        !proj
            .path()
            .join("proj")
            .join(".harness")
            .join("mcp.json")
            .exists(),
        "fail-closed install writes nothing"
    );

    // resolving store: placeholder preserved in config, value NEVER in file
    let proj2 = TempDir::new().unwrap();
    let scope2 = project(proj2.path());
    let receipt = RegistryManager::install(
        &store,
        "acme-mcp",
        &scope2,
        &InstallOptions::default(),
        &secrets(&[("ACME_TOKEN", "s3cr3t-value")]),
    )
    .expect("install with resolved secret");
    assert!(receipt.files.contains(&"mcp/acme".to_string()));
    let mcp_json =
        std::fs::read_to_string(proj2.path().join("proj").join(".harness").join("mcp.json"))
            .unwrap();
    assert!(
        mcp_json.contains("{{secret:ACME_TOKEN}}"),
        "placeholder stays"
    );
    assert!(
        !mcp_json.contains("s3cr3t-value"),
        "secret value never lands in config"
    );

    // mcp entry tamper is drift too
    let mcp_path = proj2.path().join("proj").join(".harness").join("mcp.json");
    std::fs::write(
        &mcp_path,
        r#"{"mcpServers": {"acme": {"command": "evil"}}}"#,
    )
    .unwrap();
    let drift = RegistryManager::verify(proj2.path().join("proj").as_path()).unwrap();
    assert!(
        drift
            .iter()
            .any(|d| matches!(d, Drift::Modified { key, .. } if key == "mcp/acme")),
        "mcp entry drift detected, got {drift:?}"
    );
}

#[test]
fn untrusted_index_text_is_validated_not_evaluated() {
    // path traversal / spaces / control chars in names are typed rejects
    for bad in ["../evil", "dot net", "a\u{7}b", "", "UPPER", "dot/net"] {
        assert!(
            matches!(
                harness_skills::registry::validate_item_name(bad),
                Err(RegistryError::InvalidName { .. })
            ),
            "name {bad:?} must be rejected"
        );
    }
    assert!(harness_skills::registry::validate_item_name("dotnet-clean-2").is_ok());

    for bad in ["1.0.0/../../x", "v\u{1}\u{2}", "has space"] {
        assert!(
            matches!(
                harness_skills::registry::validate_version(bad),
                Err(RegistryError::InvalidVersion { .. })
            ),
            "version {bad:?} must be rejected"
        );
    }
    assert!(harness_skills::registry::validate_version("1.0.0-beta.1+mips").is_ok());

    // pack file keys must be safe relative paths
    for bad in ["../escape.md", "/abs.md", "a\\b.md", "deep/\u{0}/x"] {
        assert!(
            !PackFile::validate_relpath(bad),
            "path {bad:?} must be rejected"
        );
    }
    assert!(PackFile::validate_relpath("SKILL.md"));
    assert!(PackFile::validate_relpath("sub/dir/notes.md"));

    // an index entry with a bad name is a typed parse error, not a panic
    let mut index = RegistryIndex::default();
    index.entries.push(harness_skills::registry::RegistryEntry {
        name: "../evil".to_string(),
        version: "1.0.0".to_string(),
        category: "x".to_string(),
        summary: String::new(),
        hash: blake3_hex(b"x"),
        size: 1,
        signature: None,
    });
    match RegistryIndex::parse(&RegistryIndex::to_bytes(&index)) {
        Err(RegistryError::InvalidName { .. }) => {}
        other => panic!("expected InvalidName from index parse, got {other:?}"),
    }
}

#[test]
fn corrupt_ledger_is_typed_and_missing_ledger_lists_empty() {
    let proj = TempDir::new().unwrap();
    assert!(RegistryManager::installed(proj.path()).unwrap().is_empty());
    let ledger = proj.path().join(".harness").join("registry-lock.json");
    std::fs::create_dir_all(ledger.parent().unwrap()).unwrap();
    std::fs::write(&ledger, "{ nope").unwrap();
    match RegistryManager::installed(proj.path()) {
        Err(RegistryError::LedgerCorrupt { .. }) => {}
        other => panic!("expected LedgerCorrupt, got {other:?}"),
    }
}
