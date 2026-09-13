//! Lock pinning, consent gating, non-English triggers, unicode paths,
//! invoke surfaces, and the no-body-in-metadata guarantee.

use std::fs;
use std::path::Path;

use harness_skills::consent::ConsentGate;
use harness_skills::loader::{LockMode, SkillLoader};
use harness_skills::routing::RouteQuery;

fn write(p: &Path, contents: &str) {
    fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
    fs::write(p, contents).expect("write");
}

fn skill_doc(name: &str, desc: &str) -> String {
    format!("---\nname: {name}\ndescription: {desc}\n---\n\n# {name}\nbody of {name}\n")
}
#[test]
fn lock_pinning_first_load_records_and_mismatch_shadows() {
    let tmp = tempfile::tempdir().expect("tmp");
    let base = tmp.path();
    let dir = base.join(".opencode").join("skills").join("pinned-skill");
    write(
        &dir.join("SKILL.md"),
        &skill_doc("pinned-skill", "pinnable routing"),
    );

    // First load with recording: skills-lock.json is written.
    let index = SkillLoader::new()
        .vendor_root(base)
        .lock_mode(LockMode::Record)
        .load()
        .expect("load");
    let lock_path = base.join(".harness").join("skills-lock.json");
    assert!(lock_path.is_file(), "install records written on first load");
    assert_eq!(index.report().parsed, 1);
    assert!(index.report().lock_mismatches.is_empty());
    let _ = index;

    // Tamper with the pinned skill.
    write(
        &dir.join("SKILL.md"),
        &skill_doc("pinned-skill", "tampered routing"),
    );

    // Second load: content-hash mismatch → typed warning + shadow (no routing).
    let index = SkillLoader::new()
        .vendor_root(base)
        .lock_mode(LockMode::Record)
        .load()
        .expect("load");
    assert_eq!(index.report().lock_mismatches.len(), 1, "mismatch detected");
    let m = &index.report().lock_mismatches[0];
    assert_eq!(m.name.as_str(), "pinned-skill");
    assert_ne!(m.pinned_hash, m.actual_hash);
    let decision = index.route_text("tampered routing", None);
    assert!(
        !decision
            .matched
            .iter()
            .any(|x| x.name.as_str() == "pinned-skill"),
        "mismatched skill is shadowed out of routing"
    );
    assert!(
        index.meta("pinned-skill").is_some(),
        "still listed for inspection"
    );
}

#[test]
fn consent_gating_keeps_skills_inert_until_recorded() {
    let tmp = tempfile::tempdir().expect("tmp");
    let base = tmp.path();
    let dir = base.join(".opencode").join("skills").join("consented-tool");
    write(
        &dir.join("SKILL.md"),
        "---\nname: consented-tool\ndescription: runs things. USE FOR: deploy everything\nallowed-tools: [\"Bash\", \"Write\"]\n---\nbody\n",
    );
    let index = SkillLoader::new()
        .vendor_root(base)
        .lock_mode(LockMode::Off)
        .load()
        .expect("load");

    let mut gate = ConsentGate::new();
    let warning = ConsentGate::install_warning(index.meta("consented-tool").expect("meta"));
    assert!(
        warning.text.contains("Bash") && warning.text.contains("Write"),
        "INSTALL_WARNING generated from declared tools: {}",
        warning.text
    );
    assert!(
        warning.text.to_lowercase().contains("deploy everything"),
        "warning describes what it can do from triggers"
    );

    // Inert before consent.
    let pre = index.route_with(&query("deploy everything", None), Some(&gate));
    assert!(
        !pre.matched
            .iter()
            .any(|m| m.name.as_str() == "consented-tool"),
        "skill is inert until consent is recorded"
    );
    assert!(
        pre.pending_consent
            .iter()
            .any(|n| n.as_str() == "consented-tool"),
        "surfaced as pending consent"
    );

    // Grant, then it routes.
    gate.grant(index.meta("consented-tool").expect("meta"), warning);
    let post = index.route_with(&query("deploy everything", None), Some(&gate));
    assert!(
        post.matched
            .iter()
            .any(|m| m.name.as_str() == "consented-tool"),
        "routes once consent recorded"
    );
}

fn query<'a>(text: &'a str, work: Option<&'a Path>) -> RouteQuery<'a> {
    RouteQuery {
        text,
        work_path: work,
    }
}

#[test]
fn cjk_and_emoji_triggers_route_by_containment() {
    let tmp = tempfile::tempdir().expect("tmp");
    let dir = tmp.path().join(".opencode").join("skills").join("i18n");
    write(
        &dir.join("SKILL.md"),
        "---\nname: i18n\ndescription: multilingual audit helper\ntriggers: [\"安全审计\", \"セキュリティ監査\", \"보안 감사\", \"🔒 security\"]\n---\nbody\n",
    );
    let index = SkillLoader::new()
        .vendor_root(tmp.path())
        .lock_mode(LockMode::Off)
        .load()
        .expect("load");
    let cases = [
        ("请对我的项目做一次安全审计", true),
        ("プロジェクトのセキュリティ監査をお願いします", true),
        ("보안 감사 좀 해주세요", true),
        ("run the 🔒 security pass", true),
        ("给项目加个漂亮的页脚", false),
    ];
    for (prompt, should_match) in cases {
        let d = index.route_text(prompt, None);
        let hit = d.matched.iter().any(|m| m.name.as_str() == "i18n");
        assert_eq!(hit, should_match, "prompt {prompt:?} match={should_match}");
    }
}

#[test]
fn unicode_directory_names_load() {
    let tmp = tempfile::tempdir().expect("tmp");
    let dir = tmp
        .path()
        .join(".opencode")
        .join("skills")
        .join("навык-セキュリティ");
    write(
        &dir.join("SKILL.md"),
        &skill_doc("unicode-dir-skill", "unicode dir routing"),
    );
    let index = SkillLoader::new()
        .vendor_root(tmp.path())
        .lock_mode(LockMode::Off)
        .load()
        .expect("load");
    assert!(index.meta("unicode-dir-skill").is_some());
}

#[test]
fn broken_nested_dirs_do_not_crash_the_scan() {
    let tmp = tempfile::tempdir().expect("tmp");
    let skills = tmp.path().join(".cursor").join("skills");
    fs::create_dir_all(skills.join("empty-dir-no-skill-md")).expect("mkdir");
    // A FILE where a skill dir name would be (not a dir).
    fs::write(skills.join("stray-file.md"), "not a skill").expect("write");
    write(
        &skills.join("works").join("SKILL.md"),
        &skill_doc("works", "cursor routing"),
    );
    let index = SkillLoader::new()
        .vendor_root(tmp.path())
        .lock_mode(LockMode::Off)
        .load()
        .expect("scan survives junk");
    assert!(index.meta("works").is_some());
}

#[test]
fn invoke_surfaces_allowed_tools_and_model_guidance() {
    let tmp = tempfile::tempdir().expect("tmp");
    let dir = tmp.path().join(".opencode").join("skills").join("forky");
    write(
        &dir.join("SKILL.md"),
        "---\nname: forky\ndescription: d\nallowed-tools: [\"Read\", \"Bash(git:*)\"]\ncontext_fork: true\nmodel-guidance:\n  glm-5: Be terse.\n---\n\n# Forky\nfork body\n",
    );
    let index = SkillLoader::new()
        .vendor_root(tmp.path())
        .lock_mode(LockMode::Off)
        .load()
        .expect("load");
    let meta = index.meta("forky").expect("meta");
    assert!(meta.context_fork, "context_fork declaration parses");
    let invoked = index.invoke("forky").expect("invoke");
    assert!(invoked.body.contains("fork body"));
    assert_eq!(
        invoked.allowed_tools,
        vec!["Read".to_string(), "Bash(git:*)".to_string()]
    );
    assert_eq!(invoked.model_guidance.len(), 1);
    assert_eq!(invoked.model_guidance[0].model, "glm-5");
    // Wildcard tool matching used by the orchestration layer later.
    assert!(harness_skills::skill::tool_pattern_matches(
        "Bash(git:*)",
        "Bash(git:status)"
    ));
    assert!(!harness_skills::skill::tool_pattern_matches(
        "Bash(git:*)",
        "Bash(rm:-rf)"
    ));
}

#[test]
fn metas_json_is_the_only_routing_surface() {
    let tmp = tempfile::tempdir().expect("tmp");
    let dir = tmp
        .path()
        .join(".opencode")
        .join("skills")
        .join("secret-body");
    write(
        &dir.join("SKILL.md"),
        "---\nname: secret-body\ndescription: d\n---\n\nTOPSECRETMARKER internal instructions\n",
    );
    let index = SkillLoader::new()
        .vendor_root(tmp.path())
        .lock_mode(LockMode::Off)
        .load()
        .expect("load");
    let surface = serde_json::to_string(&index.metas_json()).expect("ser");
    assert!(
        !surface.contains("TOPSECRETMARKER"),
        "bodies must not be injectable by default"
    );
}
