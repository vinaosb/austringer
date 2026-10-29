//! Lock-mode load pins: recorded pins equal the on-disk content hashes,
//! an unchanged second load never rewrites skills-lock.json, and a
//! mismatched pin still shadows the skill out of routing.

use std::fs;
use std::path::{Path, PathBuf};

use austringer_skills::loader::{LockMode, SkillLoader};
use austringer_skills::lock::SkillsLockFile;

fn write(p: &Path, contents: &str) {
    fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
    fs::write(p, contents).expect("write");
}

fn skill_doc(name: &str, desc: &str) -> String {
    format!("---\nname: {name}\ndescription: {desc}\n---\n\n# {name}\nbody of {name}\n")
}

fn skill_md(base: &Path, name: &str) -> PathBuf {
    base.join(".opencode")
        .join("skills")
        .join(name)
        .join("SKILL.md")
}

#[test]
fn recorded_pins_match_the_on_disk_hashes() {
    let tmp = tempfile::tempdir().expect("tmp");
    let base = tmp.path();
    for name in ["alpha", "beta"] {
        write(&skill_md(base, name), &skill_doc(name, "pinning surface"));
    }
    let index = SkillLoader::new()
        .vendor_root(base)
        .lock_mode(LockMode::Record)
        .load()
        .expect("load");
    assert_eq!(index.report().parsed, 2);
    let lock = SkillsLockFile::read(&base.join(".austringer").join("skills-lock.json"))
        .expect("lock read")
        .expect("lock written");
    for name in ["alpha", "beta"] {
        let bytes = fs::read(skill_md(base, name)).expect("read SKILL.md");
        assert_eq!(
            lock.pins[name].content_hash,
            SkillsLockFile::hash_bytes(&bytes),
            "pin for {name} equals the on-disk content hash"
        );
    }
}

#[test]
fn unchanged_second_load_does_not_rewrite_the_lock() {
    let tmp = tempfile::tempdir().expect("tmp");
    let base = tmp.path();
    write(
        &skill_md(base, "stable"),
        &skill_doc("stable", "stability pin"),
    );
    let lock_path = base.join(".austringer").join("skills-lock.json");
    let _first = SkillLoader::new()
        .vendor_root(base)
        .lock_mode(LockMode::Record)
        .load()
        .expect("first load");
    let before = fs::read(&lock_path).expect("lock after first load");
    let mtime_before = fs::metadata(&lock_path)
        .expect("meta")
        .modified()
        .expect("mtime");
    let _second = SkillLoader::new()
        .vendor_root(base)
        .lock_mode(LockMode::Record)
        .load()
        .expect("second load");
    let after = fs::read(&lock_path).expect("lock after second load");
    let mtime_after = fs::metadata(&lock_path)
        .expect("meta")
        .modified()
        .expect("mtime");
    assert_eq!(
        before, after,
        "lock content is byte-stable across unchanged loads"
    );
    assert_eq!(
        mtime_before, mtime_after,
        "unchanged load performs no lock write (tmp+rename would bump mtime)"
    );
}

#[test]
fn mismatched_pin_still_shadows_the_skill_out_of_routing() {
    let tmp = tempfile::tempdir().expect("tmp");
    let base = tmp.path();
    let dir = skill_md(base, "tampered");
    write(&dir, &skill_doc("tampered", "original routing"));
    let _first = SkillLoader::new()
        .vendor_root(base)
        .lock_mode(LockMode::Record)
        .load()
        .expect("first load");
    write(&dir, &skill_doc("tampered", "tampered routing"));
    let index = SkillLoader::new()
        .vendor_root(base)
        .lock_mode(LockMode::Record)
        .load()
        .expect("second load");
    assert_eq!(index.report().lock_mismatches.len(), 1);
    let decision = index.route_text("tampered routing", None);
    assert!(
        !decision
            .matched
            .iter()
            .any(|m| m.name.as_str() == "tampered"),
        "mismatched skill is shadowed out of routing"
    );
    assert!(
        index.meta("tampered").is_some(),
        "still listed for inspection"
    );
}
