//! Acceptance: load the user's REAL skills dirs (fixture corpus copy),
//! all entries parse, route, negative-scope; progressive disclosure is
//! structural (metadata surface never carries bodies).

use std::path::Path;

use austringer_skills::loader::{LockMode, SkillLoader, Vendor};

fn corpus(sub: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/corpus")
        .join(sub)
}

fn load_corpus() -> austringer_skills::loader::SkillIndex {
    // Fixture copies hold the *contents* of each vendor skills dir.
    SkillLoader::new()
        .skills_dir(corpus("opencode"), Vendor::Opencode)
        .skills_dir(corpus("agents"), Vendor::Agents)
        .lock_mode(LockMode::Off)
        .load()
        .expect("corpus load must not fail")
}

#[test]
fn all_real_corpus_entries_parse_and_count_is_recorded() {
    // Corpus inventory (documented in task-30 evidence): the user's real
    // C:\Users\vsbb1\.config\opencode\skills (49 skill dirs + `_shared`
    // partials) + C:\Users\vsbb1\.agents\skills (7 dirs), copied byte-exact.
    // `microsoft-foundry` is a nested tree carrying 6 SKILL.md files, so
    // the corpus holds 61 loadable skill entries total. Every one must
    // parse with ZERO hard errors; known-shape entries carry typed
    // warnings instead (7 bare-markdown skills, 1 lossy-UTF-8 file).
    let index = load_corpus();
    assert_eq!(
        index.report().parsed,
        61,
        "all corpus SKILL.md entries parse"
    );
    assert!(
        index.report().parse_errors.is_empty(),
        "no hard parse errors in the real corpus: {:?}",
        index.report().parse_errors
    );
    let bare = index
        .report()
        .warnings
        .iter()
        .filter(|w| w.is_missing_frontmatter())
        .count();
    assert_eq!(
        bare, 7,
        "the 7 bare-markdown engineering skills parse via dir-name derivation"
    );
    let lossy = index
        .report()
        .warnings
        .iter()
        .filter(|w| w.is_invalid_utf8())
        .count();
    assert_eq!(
        lossy, 1,
        "lgpd-data-privacy's invalid UTF-8 byte is replaced lossily with a warning"
    );
    let truncated = index
        .report()
        .warnings
        .iter()
        .filter(|w| {
            matches!(
                w.kind,
                austringer_skills::skill::WarningKind::DescriptionTruncated { .. }
            )
        })
        .count();
    assert_eq!(
        truncated, 1,
        "ui-ux-accessibility's over-cap description truncates with a warning"
    );
    // (The corpus declares no unknown top-level keys; that channel is
    // proven with synthetic documents in skill_format.rs.)
}

#[test]
fn progressive_disclosure_metadata_surface_has_no_body() {
    let index = SkillLoader::new()
        .skills_dir(corpus("opencode"), Vendor::Opencode)
        .lock_mode(LockMode::Off)
        .load()
        .expect("load");
    let meta = index.meta("audit-security").expect("present");
    // The metadata surface exposes ONLY routing fields.
    assert_eq!(meta.name.as_str(), "audit-security");
    assert!(meta.description.starts_with("Security auditing"));
    assert_eq!(meta.version.as_deref(), Some("2.0.0"));
    assert!(meta.languages.contains(&".NET/C#".to_string()));
    let ser = serde_json::to_string(&index.metas_json()).expect("serializable");
    assert!(
        !ser.contains("# Security Audit Squad"),
        "body must not leak into the metadata JSON surface"
    );
    // Body arrives only through the explicit invoke path.
    let invoked = index.invoke("audit-security").expect("invoke loads body");
    assert!(
        invoked.body.contains("# Security Audit Squad"),
        "invoke returns the full body"
    );
    assert!(
        invoked.body.contains("USE FOR"),
        "body content beyond the frontmatter is present on invoke"
    );
}

#[test]
fn corpus_trigger_routing_matrix() {
    let index = load_corpus();
    let matrix = [
        (
            "audit security of my Flask app: injection and secrets",
            Some("audit-security"),
        ),
        ("help me set up Keycloak OIDC login", Some("keycloak-oidc")),
        (
            "nightly cleanup jobs and recurring jobs",
            Some("quartz-scheduling"),
        ),
        ("please run /caveman", Some("caveman")),
        ("converting documents to markdown", Some("markitdown")),
        ("what database should I use for analytics", None),
    ];
    for (prompt, expected) in matrix {
        let decision = index.route_text(prompt, None);
        let names: Vec<&str> = decision
            .matched
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>();
        if let Some(want) = expected {
            assert!(
                names.contains(&want),
                "prompt {prompt:?} should route to {want}; got {names:?}"
            );
        } else {
            assert!(
                names.is_empty(),
                "prompt {prompt:?} must route nowhere; got {names:?}"
            );
        }
    }
}

#[test]
fn corpus_negative_scope_blocks_a_would_match_trigger() {
    let index = SkillLoader::new()
        .skills_dir(corpus("opencode"), Vendor::Opencode)
        .lock_mode(LockMode::Off)
        .load()
        .expect("load");
    // lgpd-data-privacy's DO NOT USE FOR: "frontend UI work" (real corpus
    // text). The prompt positively matches ("consent management" is a USE
    // FOR trigger) yet the negative scope must block the route.
    let prompt = "review consent management for our frontend UI work";
    let decision = index.route_text(prompt, None);
    let blocked = decision
        .blocked
        .iter()
        .find(|b| b.name.as_str() == "lgpd-data-privacy")
        .expect("lgpd must appear as negatively-scoped");
    assert!(
        blocked
            .blocked_by
            .iter()
            .any(|t| t.contains("frontend UI work")),
        "blocked by the real DO-NOT-USE phrase: {:?}",
        blocked.blocked_by
    );
    assert!(
        !decision
            .matched
            .iter()
            .any(|m| m.name.as_str() == "lgpd-data-privacy"),
        "negatively-scoped skill must not appear in matched"
    );
    // Control: same domain, no negative phrase → routes.
    let control = index.route_text("LGPD compliance review of consent management", None);
    assert!(
        control
            .matched
            .iter()
            .any(|m| m.name.as_str() == "lgpd-data-privacy"),
        "control prompt without the negative phrase routes"
    );
}

#[test]
fn shared_partials_dir_loads_as_pack_not_skill() {
    let index = SkillLoader::new()
        .skills_dir(corpus("opencode"), Vendor::Opencode)
        .lock_mode(LockMode::Off)
        .load()
        .expect("load");
    assert!(
        index.meta("_shared").is_none(),
        "underscore dirs are pack partials, not skills"
    );
    assert!(
        index
            .shared_partials()
            .any(|p| p.file_stem().is_some_and(|s| s == "audit-template")),
        "audit-template partial discovered from the real _shared dir"
    );
}
