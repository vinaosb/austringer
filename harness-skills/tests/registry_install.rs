//! Happy path (plan todo 32 acceptance): OFFLINE install of the shipped
//! T33 dotnet pack from a local registry dir with pin+hash verification,
//! ledger entry, and manifest sync through the REAL T30 loader; then
//! remove. No network exists anywhere in this path (structural).

use harness_skills::loader::{LockMode, SkillLoader};
use harness_skills::registry::{
    InstallOptions, RegistryError, RegistryManager, RegistryStore, Scope,
};
use tempfile::TempDir;

mod registry_common;

use registry_common::{key, project, real_server};

fn opts() -> InstallOptions<'static> {
    InstallOptions::default()
}

#[test]
fn offline_install_from_local_dir_pins_verifies_and_syncs_manifest() {
    let server = TempDir::new().unwrap();
    let proj = TempDir::new().unwrap();
    let store = real_server(server.path(), None);
    let scope = project(proj.path());

    // OFFLINE: LocalDirStore is files-only; the crate has no network code.
    let receipt = RegistryManager::install(
        &store,
        "dotnet",
        &scope,
        &opts(),
        &registry_common::secrets(&[]),
    )
    .expect("offline install");
    assert_eq!(receipt.name, "dotnet");
    assert_eq!(receipt.version, "1.0.0");
    assert!(
        receipt
            .files
            .contains(&"skills/dotnet/SKILL.md".to_string())
    );
    assert!(
        receipt
            .files
            .iter()
            .any(|k| k.starts_with("skills/_shared/"))
    );
    assert!(receipt.install_warning.contains("INSTALL WARNING"));

    // files really on disk (assert CONTENT, not just counts)
    let skill_md = proj
        .path()
        .join("proj")
        .join(".harness")
        .join("skills")
        .join("dotnet")
        .join("SKILL.md");
    let text = std::fs::read_to_string(&skill_md).expect("SKILL.md installed");
    assert!(text.contains("name: dotnet"));

    // LEDGER: provenance = content hash + source + per-file pins
    let ledger_path = proj
        .path()
        .join("proj")
        .join(".harness")
        .join("registry-lock.json");
    let installed = RegistryManager::installed(proj.path().join("proj").as_path()).unwrap();
    let entry = installed
        .iter()
        .find(|e| e.name == "dotnet")
        .expect("ledger entry");
    assert_eq!(entry.content_hash, receipt.hash);
    assert_eq!(entry.version, "1.0.0");
    assert!(ledger_path.is_file());
    let pinned = entry.files.get("skills/dotnet/SKILL.md").expect("file pin");
    assert_ne!(
        pinned.as_str(),
        entry.content_hash.as_str(),
        "per-file pin is its own hash"
    );

    // PIN+HASH VERIFY: clean verify right after install
    assert!(
        RegistryManager::verify_entry(proj.path().join("proj").as_path(), "dotnet")
            .unwrap()
            .is_empty()
    );

    // MANIFEST SYNC: the real T30 loader discovers + routes + invokes it
    let index = SkillLoader::new()
        .vendor_root(proj.path().join("proj"))
        .lock_mode(LockMode::Off)
        .load()
        .expect("loader scans installed scope");
    let decision = index.route_text("how do I structure an ef core migration", None);
    let names: Vec<&str> = decision.matched.iter().map(|m| m.name.as_str()).collect();
    assert!(
        names.contains(&"dotnet"),
        "installed pack routes, got {names:?}"
    );
    let invoked = index
        .invoke("dotnet")
        .expect("invoke resolves shared partials");
    assert!(
        !invoked.resolved_partials.is_empty(),
        "{{shared:...}} resolved"
    );
}

#[test]
fn signed_registry_installs_with_verifier_and_fails_closed_without_one() {
    let server = TempDir::new().unwrap();
    let proj = TempDir::new().unwrap();
    let k = key();
    let store = real_server(server.path(), Some(&k));
    let scope = project(proj.path());
    let verifier = harness_skills::registry::Blake3Verifier::new(&k);

    let mut options = opts();
    options.verifier = Some(&verifier);
    let receipt = RegistryManager::install(
        &store,
        "legal",
        &scope,
        &options,
        &registry_common::secrets(&[]),
    )
    .expect("signed install verifies");
    assert_eq!(receipt.name, "legal");

    // signed entry + NO verifier supplied => fail closed (never silent skip)
    let proj2 = TempDir::new().unwrap();
    let scope2 = project(proj2.path());
    match RegistryManager::install(
        &store,
        "legal",
        &scope2,
        &opts(),
        &registry_common::secrets(&[]),
    ) {
        Err(RegistryError::SignatureVerifierMissing { .. }) => {}
        other => panic!("expected SignatureVerifierMissing, got {other:?}"),
    }
}

#[test]
fn review_ticket_binds_to_hash_and_required_review_gates_install() {
    let server = TempDir::new().unwrap();
    let proj = TempDir::new().unwrap();
    let store = real_server(server.path(), None);
    let scope = project(proj.path());

    let ticket = RegistryManager::review_ticket(&store, "maths").unwrap();
    assert_eq!(ticket.name, "maths");
    let idx = store.index().unwrap();
    assert_eq!(ticket.hash, idx.find("maths").unwrap().hash);

    let mut options = opts();
    options.require_review = true;
    match RegistryManager::install(
        &store,
        "maths",
        &scope,
        &options,
        &registry_common::secrets(&[]),
    ) {
        Err(RegistryError::Unreviewed { .. }) => {}
        other => panic!("expected Unreviewed without acceptance, got {other:?}"),
    }

    let acceptance = ticket.accept("reviewer@example");
    options.acceptance = Some(&acceptance);
    let receipt = RegistryManager::install(
        &store,
        "maths",
        &scope,
        &options,
        &registry_common::secrets(&[]),
    )
    .expect("reviewed install passes");
    assert_eq!(receipt.name, "maths");
    let installed = RegistryManager::installed(proj.path().join("proj").as_path()).unwrap();
    let entry = installed.iter().find(|e| e.name == "maths").unwrap();
    assert_eq!(entry.review.as_ref().unwrap().reviewer, "reviewer@example");
}

#[test]
fn remove_uninstalls_files_ledger_and_loader_visibility() {
    let server = TempDir::new().unwrap();
    let proj = TempDir::new().unwrap();
    let store = real_server(server.path(), None);
    let scope = project(proj.path());
    RegistryManager::install(
        &store,
        "frontend",
        &scope,
        &opts(),
        &registry_common::secrets(&[]),
    )
    .unwrap();

    let removed =
        RegistryManager::remove(proj.path().join("proj").as_path(), "frontend").expect("remove");
    assert!(removed.removed_files.iter().any(|f| f.contains("SKILL.md")));
    assert!(
        !proj
            .path()
            .join("proj")
            .join(".harness")
            .join("skills")
            .join("frontend")
            .exists(),
        "skill dir pruned"
    );
    let installed = RegistryManager::installed(proj.path().join("proj").as_path()).unwrap();
    assert!(installed.iter().all(|e| e.name != "frontend"));
    let index = SkillLoader::new()
        .vendor_root(proj.path().join("proj"))
        .lock_mode(LockMode::Off)
        .load()
        .unwrap();
    assert!(index.meta("frontend").is_none(), "loader no longer sees it");
}

#[test]
fn project_and_global_scopes_write_to_their_own_roots() {
    let server = TempDir::new().unwrap();
    let proj = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let store = real_server(server.path(), None);

    RegistryManager::install(
        &store,
        "legal",
        &Scope::project(proj.path().to_path_buf()),
        &opts(),
        &registry_common::secrets(&[]),
    )
    .unwrap();
    RegistryManager::install(
        &store,
        "legal",
        &Scope::global(home.path().to_path_buf()),
        &opts(),
        &registry_common::secrets(&[]),
    )
    .unwrap();

    assert!(
        !proj
            .path()
            .join(".harness")
            .join("skills")
            .join("legal")
            .is_file()
    );
    assert!(
        proj.path()
            .join(".harness")
            .join("skills")
            .join("legal")
            .join("SKILL.md")
            .is_file()
    );
    assert!(
        home.path()
            .join(".harness")
            .join("skills")
            .join("legal")
            .join("SKILL.md")
            .is_file()
    );
    let proj_ledger = RegistryManager::installed(proj.path()).unwrap();
    let home_ledger = RegistryManager::installed(home.path()).unwrap();
    assert_eq!(proj_ledger.len(), 1);
    assert_eq!(home_ledger.len(), 1);
    assert_eq!(proj_ledger[0].content_hash, home_ledger[0].content_hash);
}

#[test]
fn upgrade_replaces_provenance_and_the_ledger_diff_shows_what_moved() {
    let server = TempDir::new().unwrap();
    let proj = TempDir::new().unwrap();
    let store = real_server(server.path(), None);
    let root = proj.path().join("proj");
    let scope = project(proj.path());

    RegistryManager::install(
        &store,
        "maths",
        &scope,
        &opts(),
        &registry_common::secrets(&[]),
    )
    .expect("v1 install");
    let before = harness_skills::registry::RegistryLedger::load(&root).unwrap();

    // publish an upgraded maths pack: only SKILL.md changes; the shared
    // partials keep byte-identical content (pins must NOT move)
    let index = store.index().unwrap();
    let old_entry = index.find("maths").unwrap().clone();
    let old_pack =
        harness_skills::registry::PackFile::parse(&store.pack_bytes(&old_entry.hash).unwrap())
            .unwrap();
    let mut upgraded = old_pack.clone();
    let bumped = old_pack.files["SKILL.md"].clone() + "\n<!-- v2 -->\n";
    upgraded.files.insert("SKILL.md".to_string(), bumped);
    upgraded.version = "1.1.0".to_string();
    harness_skills::registry::add_pack(server.path(), &upgraded, None).unwrap();

    let receipt = RegistryManager::install(
        &store,
        "maths",
        &scope,
        &opts(),
        &registry_common::secrets(&[]),
    )
    .expect("v2 install (upgrade)");
    assert_eq!(receipt.version, "1.1.0");

    // exactly ONE ledger entry, new provenance
    let after = harness_skills::registry::RegistryLedger::load(&root).unwrap();
    assert_eq!(after.entries.len(), 1);
    assert_eq!(after.entries[0].version, "1.1.0");
    assert_ne!(
        after.entries[0].content_hash,
        before.entries[0].content_hash
    );

    // the ledger DIFF names the moved pin (was = v1 pin, now = v2 pin)
    let diff = before.diff(&after);
    assert_eq!(diff.changed.len(), 1, "one item changed");
    let changed = &diff.changed[0];
    assert_eq!(changed.name, "maths");
    assert_eq!(changed.files_changed.len(), 1, "shared partials unchanged");
    let fc = &changed.files_changed[0];
    assert_eq!(fc.key, "skills/maths/SKILL.md");
    assert_eq!(
        fc.was.as_deref(),
        Some(before.entries[0].files["skills/maths/SKILL.md"].as_str())
    );
    assert_eq!(
        fc.now.as_deref(),
        Some(after.entries[0].files["skills/maths/SKILL.md"].as_str())
    );
    assert!(
        RegistryManager::verify(&root).unwrap().is_empty(),
        "post-upgrade disk matches ledger"
    );
}

#[test]
fn manager_search_is_the_frontend_surface_over_the_store() {
    let server = TempDir::new().unwrap();
    let store = real_server(server.path(), None);

    let hits = RegistryManager::search(&store, "LEGAL").unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].name, "legal");
    let hits = RegistryManager::search(&store, "investors").unwrap();
    assert!(
        hits.iter().any(|e| e.name == "finance-investors"),
        "summary matched"
    );
    assert!(
        RegistryManager::search(&store, "klingon-nothing")
            .unwrap()
            .is_empty()
    );
}
