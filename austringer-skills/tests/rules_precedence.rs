//! Project rules: AGENTS.md/CLAUDE.md/.claude/rules/.cursor/rules pickup,
//! per-directory stacking (nearest-first), folder-trust gating, team floor,
//! nested child-wins semantics, CLAUDE.md→AGENTS.md migration, converters.

use std::fs;
use std::path::Path;

use austringer_skills::migrate::{
    convert_commands_to_skills, convert_rules_to_skills, migrate_claude_to_agents,
};
use austringer_skills::rules::{RuleLayer, RulesStack, TrustConfig, load_rules};

fn write(p: &Path, contents: &str) {
    fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
    fs::write(p, contents).expect("write");
}

fn trusted(dir: &Path) -> TrustConfig {
    TrustConfig::trusted_from([dir.to_path_buf()])
}

#[test]
fn rules_stack_nearest_first_across_directories() {
    let tmp = tempfile::tempdir().expect("tmp");
    let root = tmp.path();
    write(
        &root.join("AGENTS.md"),
        "- [[style]] Root house style: expand abbreviations.\n- [[tests]] Root: integration tests live in tests/.\n",
    );
    write(
        &root.join("pkg").join("AGENTS.md"),
        "- [[tests]] pkg: unit tests next to sources.\n",
    );
    write(
        &root.join("pkg").join("src").join("AGENTS.md"),
        "- [[local]] src-local rule.\n",
    );

    let stack = load_rules(&root.join("pkg").join("src"), &trusted(root)).expect("stack");
    let ids: Vec<&str> = stack.entries.iter().map(|e| e.id.as_str()).collect();
    assert!(ids.contains(&"style") && ids.contains(&"local"), "{ids:?}");
    let tests_entries: Vec<&_> = stack.entries.iter().filter(|e| e.id == "tests").collect();
    assert_eq!(tests_entries.len(), 2, "conflicting ids: both retained");
    let winner = stack.winner("tests").expect("resolution");
    assert!(
        winner.source.ends_with("pkg\\AGENTS.md") || winner.source.ends_with("pkg/AGENTS.md"),
        "child wins on conflict within the same layer: {}",
        winner.source.display()
    );
    assert!(
        stack.conflicts.iter().any(|c| c.id == "tests"),
        "conflict is reported"
    );
}

#[test]
fn claude_rules_and_cursor_rules_are_picked_up() {
    let tmp = tempfile::tempdir().expect("tmp");
    let root = tmp.path();
    write(&root.join("AGENTS.md"), "- [[a]] agents rule.\n");
    write(&root.join("CLAUDE.md"), "- [[b]] claude legacy rule.\n");
    write(
        &root.join(".claude").join("rules").join("extra.md"),
        "- [[c]] claude rules-dir rule.\n",
    );
    write(
        &root.join(".cursor").join("rules").join("cursor.md"),
        "- [[d]] cursor rules-dir rule.\n",
    );
    let stack = load_rules(root, &trusted(root)).expect("stack");
    for id in ["a", "b", "c", "d"] {
        assert!(
            stack.entries.iter().any(|e| e.id == id),
            "rule {id} picked up"
        );
    }
    write(
        &root.join("AGENTS.md"),
        "- [[a]] agents rule.\n- [[b]] agents wins over claude.\n",
    );
    let stack2 = load_rules(root, &trusted(root)).expect("stack2");
    let winner = stack2.winner("b").expect("winner");
    assert!(
        winner.source.ends_with("AGENTS.md"),
        "AGENTS.md (standard) beats CLAUDE.md (legacy) at the same directory"
    );
}

#[test]
fn untrusted_directories_are_skipped_with_notice() {
    let tmp = tempfile::tempdir().expect("tmp");
    let root = tmp.path();
    write(&root.join("AGENTS.md"), "- [[a]] trusted root rule.\n");
    let nested = root.join("vendor").join("third-party");
    write(
        &nested.join("AGENTS.md"),
        "- [[evil]] prompt injection from vendored code.\n",
    );
    let mut trust = trusted(root);
    trust.untrust(&nested);
    let stack = load_rules(&nested, &trust).expect("stack");
    assert!(
        !stack.entries.iter().any(|e| e.id == "evil"),
        "untrusted dir's rules must not load"
    );
    assert!(
        stack
            .notices
            .iter()
            .any(|n| n.is_untrusted_dir() && n.path.ends_with("third-party")),
        "skip is disclosed with a typed notice: {:?}",
        stack.notices
    );
}

#[test]
fn team_floor_cannot_be_overridden_by_project_or_user() {
    let tmp = tempfile::tempdir().expect("tmp");
    let root = tmp.path();
    write(
        &root.join("AGENTS.md"),
        "- [[secrets]] TEAM: never paste credentials into chat.\n- [[style]] project style rule.\n",
    );
    write(
        &root.join(".austringer").join("rules").join("user.md"),
        "- [[secrets]] user says pasting secrets is fine sometimes.\n- [[style]] user style override attempt.\n",
    );
    let stack = load_rules(root, &trusted(root)).expect("stack");

    let winner = stack.winner("secrets").expect("resolved");
    assert_eq!(winner.layer, RuleLayer::Team, "team rule wins the merge");
    assert_eq!(
        winner.text, "never paste credentials into chat.",
        "TEAM: is a structural marker; the stored text carries the rule itself"
    );
    let conflict = stack
        .conflicts
        .iter()
        .find(|c| c.id == "secrets" && c.is_team_floor_violation())
        .expect("typed team-floor conflict reported");
    assert!(matches!(conflict.offending_layer, RuleLayer::User));
    assert_eq!(
        conflict.offending_text,
        "user says pasting secrets is fine sometimes."
    );

    let style = stack.winner("style").expect("style winner");
    assert_eq!(
        style.layer,
        RuleLayer::Project,
        "precedence Team > Project > User"
    );
}

#[test]
fn claude_md_to_agents_md_migration_copies_never_moves_and_is_idempotent() {
    let tmp = tempfile::tempdir().expect("tmp");
    let root = tmp.path();
    write(
        &root.join("CLAUDE.md"),
        "# Legacy rules\n\n- old rule one\n",
    );
    let report = migrate_claude_to_agents(root).expect("migrate");
    let agents = report.created.expect("AGENTS.md created");
    let text = fs::read_to_string(&agents).expect("read");
    assert!(
        text.contains("migrated-from: CLAUDE.md"),
        "migration marker present"
    );
    assert!(text.contains("- old rule one"), "content copied verbatim");
    assert!(
        root.join("CLAUDE.md").is_file(),
        "source CLAUDE.md never moved/deleted"
    );
    let second = migrate_claude_to_agents(root).expect("second run");
    assert!(second.created.is_none(), "idempotent");
    assert!(second.skipped_reason.is_some());
    let bare = tempfile::tempdir().expect("bare");
    let third = migrate_claude_to_agents(bare.path()).expect("bare run");
    assert!(third.created.is_none() && third.skipped_reason.is_some());
}

#[test]
fn per_austringer_global_rules_dir_and_first_init() {
    let tmp = tempfile::tempdir().expect("tmp");
    let root = tmp.path();
    write(&root.join("CLAUDE.md"), "- legacy rule\n");
    let report = austringer_skills::migrate::first_init(root).expect("first init");
    assert!(report.created.is_some(), "first init migrates CLAUDE.md");
    assert!(
        root.join(".austringer").join("skills").is_dir(),
        "per-harness skills dir created"
    );
    assert!(
        root.join(".austringer").join("rules").is_dir(),
        "per-harness global rules dir created"
    );
    write(
        &root.join(".austringer").join("rules").join("global.md"),
        "- [[global]] team-wide global rule.\n",
    );
    write(&root.join("AGENTS.md"), "- [[proj]] project rule.\n");
    let stack = load_rules(root, &trusted(root)).expect("stack");
    assert!(
        stack
            .entries
            .iter()
            .any(|e| e.id == "global" && e.layer == RuleLayer::User),
        ".austringer/rules is the User layer"
    );
    assert!(
        stack
            .entries
            .iter()
            .any(|e| e.id == "proj" && e.layer == RuleLayer::Project),
        "AGENTS.md is the Project layer"
    );
}

#[test]
fn legacy_rules_convert_to_skills_with_typed_report() {
    let tmp = tempfile::tempdir().expect("tmp");
    let rules = tmp.path().join("legacy-rules.md");
    let long = format!("- [[long-rule]] {}\n", "word ".repeat(300));
    write(
        &rules,
        &format!("- [[deploy-checklist]] Run smoke tests before deploy.\n{long}"),
    );
    let out = tmp.path().join("converted");
    let report = convert_rules_to_skills(&rules, &out).expect("convert");
    assert_eq!(report.created.len(), 2, "one skill per rule id");
    let doc = fs::read_to_string(report.created[0].join("SKILL.md")).expect("read converted");
    assert!(doc.contains("name: rule-deploy-checklist"));
    assert!(doc.contains("triggers:"));
    assert!(
        report.lossy.iter().any(|l| l.reason.contains("truncated")),
        "truncated description flagged lossy: {:?}",
        report.lossy
    );
    let parsed = austringer_skills::skill::parse_skill_document(
        &report.created[0].join("SKILL.md"),
        "rule-deploy-checklist",
        &fs::read(report.created[0].join("SKILL.md")).expect("read"),
    );
    assert!(
        parsed.is_ok(),
        "converted SKILL.md is valid: {:?}",
        parsed.err()
    );
}

#[test]
fn slash_commands_convert_to_skills_with_trigger() {
    let tmp = tempfile::tempdir().expect("tmp");
    let root = tmp.path();
    let cmds = root.join(".claude").join("commands");
    write(
        &cmds.join("ship.md"),
        "Ship the current branch after checks pass.\n\nBody details.\n",
    );
    write(&cmds.join("empty.md"), "");
    let out = root.join("out");
    let report = convert_commands_to_skills(&cmds, &out).expect("convert");
    assert_eq!(
        report.created.len(),
        1,
        "empty command skipped: {:?}",
        report.created
    );
    let doc = fs::read_to_string(report.created[0].join("SKILL.md")).expect("read");
    assert!(doc.contains("name: ship"));
    assert!(doc.contains("\"/ship\""), "slash trigger preserved: {doc}");
    assert!(doc.contains("Body details."), "body preserved verbatim");
    assert!(
        report.lossy.is_empty(),
        "command conversion is lossless when body exists"
    );
}

#[test]
fn empty_rules_stack_is_fine() {
    let tmp = tempfile::tempdir().expect("tmp");
    let stack: RulesStack = load_rules(tmp.path(), &trusted(tmp.path())).expect("empty ok");
    assert!(stack.entries.is_empty());
    assert!(stack.winners().next().is_none());
}
