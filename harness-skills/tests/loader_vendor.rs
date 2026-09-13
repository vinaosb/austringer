//! Multi-vendor loader: 5 vendor paths, cross-vendor dedup-by-name with
//! precedence + shadows, malformed entries never abort the scan, nested
//! monorepo subtree scoping, unicode/long paths, lock pinning, consent.

use std::fs;
use std::path::Path;

use harness_skills::loader::{LockMode, SkillLoader, Vendor};
use harness_skills::routing::ScopeFiltered;

fn write(p: &Path, contents: &str) {
    fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
    fs::write(p, contents).expect("write");
}

fn skill_doc(name: &str, desc: &str) -> String {
    format!("---\nname: {name}\ndescription: {desc}\n---\n\n# {name}\nbody of {name}\n")
}

#[test]
fn vendor_precedence_dedups_by_name_and_records_shadows() {
    let tmp = tempfile::tempdir().expect("tmp");
    let base = tmp.path();
    for (vendor, body_marker) in [
        (Vendor::Harness, "harness-wins"),
        (Vendor::Opencode, "opencode-shadow"),
        (Vendor::Claude, "claude-shadow"),
        (Vendor::Agents, "agents-shadow"),
        (Vendor::Cursor, "cursor-shadow"),
    ] {
        let dir = base.join(vendor.dir_name()).join("skills").join("dupe");
        write(
            &dir.join("SKILL.md"),
            &format!(
                "---\nname: dupe\ndescription: routing description\n---\n\n# dupe\n{body_marker}\n"
            ),
        );
    }
    let index = SkillLoader::new()
        .vendor_root(base)
        .lock_mode(LockMode::Off)
        .load()
        .expect("load");
    assert_eq!(
        index.report().parsed,
        1,
        "one entry after cross-vendor dedup"
    );
    let meta = index.meta("dupe").expect("deduped entry");
    assert_eq!(
        meta.vendor,
        Vendor::Harness,
        ".harness > .opencode > .claude > .agents > .cursor"
    );
    let shadows: Vec<&str> = index
        .report()
        .shadows
        .iter()
        .map(|s| s.vendor.dir_name())
        .collect();
    assert_eq!(
        shadows,
        vec![".cursor", ".agents", ".claude", ".opencode"],
        "lower-priority occurrences recorded in precedence order"
    );
    // Shadow resolution is observable through invoke: body comes from winner.
    let invoked = index.invoke("dupe").expect("invoke");
    assert!(invoked.body.contains("harness-wins"));
}

#[test]
fn malformed_skill_is_recorded_and_scan_continues() {
    let tmp = tempfile::tempdir().expect("tmp");
    let skills = tmp.path().join(".opencode").join("skills");
    write(
        &skills.join("good-a").join("SKILL.md"),
        &skill_doc("good-a", "alpha routing"),
    );
    write(
        &skills.join("broken").join("SKILL.md"),
        "---\nname: broken\ndescription: never closed\n",
    );
    write(
        &skills.join("good-b").join("SKILL.md"),
        &skill_doc("good-b", "beta routing"),
    );
    let index = SkillLoader::new()
        .vendor_root(tmp.path())
        .lock_mode(LockMode::Off)
        .load()
        .expect("scan survives");
    assert_eq!(index.report().parsed, 2);
    assert_eq!(index.report().parse_errors.len(), 1);
    let err = &index.report().parse_errors[0];
    assert!(
        err.path.parent().is_some_and(|p| p.ends_with("broken")),
        "typed error carries path: {}",
        err.path.display()
    );
    assert!(matches!(
        err.kind,
        harness_skills::skill::ParseErrorKind::UnterminatedFrontmatter
    ));
    assert!(
        index.meta("good-b").is_some(),
        "entries after the malformed one still load"
    );
}

#[test]
fn nested_monorepo_skills_scope_to_their_subtree() {
    let tmp = tempfile::tempdir().expect("tmp");
    let base = tmp.path();
    // Global skill: unscoped.
    write(
        &base
            .join(".opencode")
            .join("skills")
            .join("global-thing")
            .join("SKILL.md"),
        &skill_doc("global-thing", "global helper. USE FOR: global routing"),
    );
    // Nested package skill under packages/foo/.harness/skills.
    let nested = base
        .join("packages")
        .join("foo")
        .join(".harness")
        .join("skills")
        .join("foo-only");
    write(
        &nested.join("SKILL.md"),
        &skill_doc("foo-only", "foo helper. USE FOR: foo scoped routing"),
    );
    let index = SkillLoader::new()
        .vendor_root(base)
        .lock_mode(LockMode::Off)
        .load()
        .expect("load");
    assert_eq!(index.report().parsed, 2);

    // Outside the subtree: foo-only is scope-filtered, global routes.
    let outside = index.route_text(
        "global routing plus foo scoped routing",
        Some(&base.join("packages").join("bar")),
    );
    assert!(
        outside
            .matched
            .iter()
            .any(|m| m.name.as_str() == "global-thing")
    );
    let filtered: Vec<&ScopeFiltered> = outside
        .scope_filtered
        .iter()
        .filter(|s| s.name.as_str() == "foo-only")
        .collect();
    assert_eq!(
        filtered.len(),
        1,
        "nested skill does not route outside its subtree"
    );
    assert!(
        filtered[0].scope.ends_with("packages\\foo") || filtered[0].scope.ends_with("packages/foo"),
        "scope recorded with the owning subtree: {}",
        filtered[0].scope.display()
    );

    // Inside the subtree: foo-only routes.
    let inside = index.route_text(
        "foo scoped routing",
        Some(&base.join("packages").join("foo").join("src")),
    );
    assert!(
        inside.matched.iter().any(|m| m.name.as_str() == "foo-only"),
        "nested skill routes inside its subtree: {:?}",
        inside.scope_filtered
    );
}

#[test]
fn nested_monorepo_skill_routes_inside_subtree() {
    let tmp = tempfile::tempdir().expect("tmp");
    let base = tmp.path();
    let nested = base
        .join("packages")
        .join("föo")
        .join(".harness")
        .join("skills")
        .join("foo-only");
    write(
        &nested.join("SKILL.md"),
        &skill_doc("foo-only", "foo helper. USE FOR: foo scoped routing"),
    );
    let index = SkillLoader::new()
        .vendor_root(base)
        .lock_mode(LockMode::Off)
        .load()
        .expect("load");
    let inside = index.route_text(
        "foo scoped routing",
        Some(
            &base
                .join("packages")
                .join("föo")
                .join("src")
                .join("deep.rs"),
        ),
    );
    assert!(
        inside.matched.iter().any(|m| m.name.as_str() == "foo-only"),
        "unicode subtree paths route inside: {:?}",
        inside.scope_filtered
    );
}

#[test]
fn long_paths_over_260_chars_load() {
    let tmp = tempfile::tempdir().expect("tmp");
    // canonicalize() yields a \\?\-prefixed path on Windows, which std::fs
    // accepts past the 260-char boundary without registry long-path opt-in.
    let mut deep = tmp.path().canonicalize().expect("canonicalize");
    for i in 0..8 {
        deep = deep.join(format!(
            "very-long-segment-{i:02}-abcdefghijklmnopqrstuvwxyz0123456"
        ));
    }
    let full = deep.join("SKILL.md");
    assert!(
        full.as_os_str().len() > 260,
        "fixture path exceeds MAX_PATH: {}",
        full.as_os_str().len()
    );
    write(
        &full,
        &skill_doc("deep-skill", "deep helper. USE FOR: deep path routing"),
    );
    let index = SkillLoader::new()
        .skills_dir(&deep, Vendor::Opencode)
        .lock_mode(LockMode::Off)
        .load()
        .expect("long paths load");
    assert!(index.meta("deep-skill").is_some(), "long path resolved");
}
