//! Shared helpers for the registry suites (todo 32). Different test
//! binaries use different subsets — helpers are shared, not dead.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use harness_skills::registry::{
    LocalDirStore, MapSecretSource, Scope, SecretKey, build_registry_from_packs_dir,
};

pub fn repo_packs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("packs")
        .canonicalize()
        .expect("packs/ registry content dir must exist at the workspace root")
}

pub fn write_file(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
    std::fs::write(path, content).expect("write file");
}

/// Builds the reference server from the REAL shipped packs tree (T33
/// content) and returns a local-file store over it.
pub fn real_server(dir: &Path, key: Option<&SecretKey>) -> LocalDirStore {
    build_registry_from_packs_dir(&repo_packs(), dir, key).expect("build reference server");
    LocalDirStore::new(dir)
}

/// Writes one synthetic skill-pack dir with minimal valid frontmatter.
pub fn write_pack(root: &Path, name: &str, trigger: &str) {
    let skill = format!(
        "---\nname: {name}\ndescription: \"Synthetic {name} pack for registry tests.\"\nversion: \"1.0.0\"\ntriggers:\n  - {trigger}\n---\n\n# {name}\n\nSynthetic body.\n"
    );
    write_file(&root.join(name).join("SKILL.md"), &skill);
}

pub fn project(base: &Path) -> Scope {
    Scope::project(base.join("proj"))
}

pub fn secrets(pairs: &[(&str, &str)]) -> MapSecretSource {
    MapSecretSource::new(
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect::<BTreeMap<String, String>>(),
    )
}

pub fn key() -> SecretKey {
    SecretKey::from_bytes([7u8; 32])
}
