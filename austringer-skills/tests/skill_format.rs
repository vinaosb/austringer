//! SKILL.md format: agentskills.io-compatible frontmatter + extensions,
//! strict on structure, tolerant on unknown keys (typed warnings).

use std::path::Path;

use austringer_skills::skill::{ParseErrorKind, SkillMeta, WarningKind, parse_skill_document};

fn parse(
    dir: &str,
    doc: &str,
) -> Result<
    (SkillMeta, Vec<austringer_skills::skill::SkillWarning>),
    austringer_skills::skill::SkillParseError,
> {
    parse_skill_document(Path::new(dir), dir, doc.as_bytes())
}

#[test]
fn agentskills_standard_shape_parses() {
    let doc = "---\nname: pdf-tools\ndescription: Convert PDFs to text and images.\nlicense: MIT\n---\n\n# PDF Tools\nBody here.\n";
    let (meta, warnings) = parse("pdf-tools", doc).expect("standard shape parses");
    assert_eq!(meta.name.as_str(), "pdf-tools");
    assert_eq!(meta.description, "Convert PDFs to text and images.");
    assert!(
        warnings.is_empty(),
        "license is a known agentskills key: {warnings:?}"
    );
}

#[test]
fn extensions_parse_typed() {
    let doc = "---\nname: i18n-helper\ndescription: >-\n  Multilingual helper.\ntriggers:\n  - 安全审计\n  - \"セキュリティ監査\"\n  - \"보안 감사\"\n  - \"🔒 security check\"\ndo-not-trigger-on:\n  - \"pure typography questions\"\nlanguages: [\"Python\", \"Rust\"]\nversion: \"1.4.0\"\nallowed-tools: [\"Read\", \"Bash(git:*)\"]\ncontext_fork: true\nmodel-guidance:\n  glm-5: Prefer terse structured output.\n  gpt-5.5: Use full sentences.\n---\nbody\n";
    let (meta, warnings) = parse("i18n-helper", doc).expect("extensions parse");
    assert_eq!(
        meta.triggers.len(),
        5,
        "4 explicit non-English triggers + the derived /name command: {warnings:?}"
    );
    assert!(meta.triggers.contains(&"安全审计".to_string()));
    assert!(meta.triggers.contains(&"🔒 security check".to_string()));
    assert_eq!(
        meta.negative_triggers,
        vec!["pure typography questions".to_string()]
    );
    assert_eq!(
        meta.languages,
        vec!["Python".to_string(), "Rust".to_string()]
    );
    assert_eq!(meta.version.as_deref(), Some("1.4.0"));
    assert_eq!(
        meta.allowed_tools,
        vec!["Read".to_string(), "Bash(git:*)".to_string()]
    );
    assert!(
        meta.context_fork,
        "context_fork flag is a typed declaration"
    );
    assert_eq!(meta.model_guidance.len(), 2);
    assert_eq!(meta.model_guidance[0].model, "glm-5");
    assert!(
        meta.model_guidance
            .iter()
            .any(|g| g.guidance.contains("terse"))
    );
}

#[test]
fn unknown_keys_are_typed_warnings_not_errors() {
    let doc = "---\nname: x-skill\ndescription: d\nfuture-field: 42\nanother-unknown: [1, 2]\n---\nbody\n";
    let (meta, warnings) = parse("x-skill", doc).expect("forward-compatible parse");
    let unknown: Vec<&str> = warnings
        .iter()
        .filter_map(|w| match &w.kind {
            WarningKind::UnknownKey { key } => Some(key.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        unknown,
        vec!["another-unknown", "future-field"],
        "sorted, typed, non-fatal: {warnings:?}"
    );
    assert_eq!(meta.name.as_str(), "x-skill");
}

#[test]
fn bare_markdown_derives_name_from_directory_with_warning() {
    let doc = "# BIM Authoring Skill\n\n## USE FOR\n- IFC models\n";
    let (meta, warnings) = parse("bim-authoring", doc).expect("opencode-style bare markdown loads");
    assert_eq!(meta.name.as_str(), "bim-authoring");
    assert!(
        warnings
            .iter()
            .any(|w| matches!(w.kind, WarningKind::MissingFrontmatter))
    );
    assert_eq!(meta.description, "BIM Authoring Skill");
}

#[test]
fn malformed_frontmatter_is_a_typed_error_with_path_and_line() {
    // Unterminated frontmatter: no closing fence.
    let err = parse("broken", "---\nname: broken\ndescription: never closed\n")
        .expect_err("unterminated");
    assert!(matches!(err.kind, ParseErrorKind::UnterminatedFrontmatter));
    assert_eq!(err.line, 4, "error carries a line number");
    assert!(err.path.ends_with("broken"), "error carries the path");

    // Garbage: a bare scalar where a mapping is required.
    let err = parse("garbage", "---\njust some words\n---\nbody\n").expect_err("mapping required");
    assert!(
        matches!(err.kind, ParseErrorKind::NotAMapping { .. }),
        "{:?}",
        err.kind
    );

    // Duplicate key is structurally ambiguous.
    let err = parse("dupe", "---\nname: a\nname: b\ndescription: d\n---\nx\n")
        .expect_err("duplicate key");
    assert!(matches!(err.kind, ParseErrorKind::DuplicateKey(ref key) if key == "name"));
    assert_eq!(err.line, 3);

    // Invalid name shape.
    let err = parse("good-dir", "---\nname: Bad_Name!\ndescription: d\n---\nx\n")
        .expect_err("kebab required");
    assert!(matches!(err.kind, ParseErrorKind::InvalidName { .. }));
}

#[test]
fn spec_limits_are_enforced_with_typed_errors() {
    // Over-cap description: tolerated with a typed truncation warning
    // (compat-swallow — the real corpus contains such skills).
    let long_desc = "d".repeat(1025);
    let doc = format!("---\nname: y\ndescription: {long_desc}\n---\nbody\n");
    let (meta, warnings) = parse("y", &doc).expect("over-cap description still loads");
    assert_eq!(meta.description.len(), 1024);
    assert!(
        warnings
            .iter()
            .any(|w| matches!(w.kind, WarningKind::DescriptionTruncated { original_len } if original_len == 1025)),
        "{warnings:?}"
    );

    // Over-cap name: hard error (names are identifiers).
    let long_name = "a".repeat(65);
    let doc = format!("---\nname: {long_name}\ndescription: d\n---\nbody\n");
    let err = parse("z", &doc).expect_err("name cap is 64");
    assert!(matches!(err.kind, ParseErrorKind::FieldTooLong { field, .. } if field == "name"));
}

#[test]
fn real_world_shapes_roundtrip() {
    // Trailing spaces after scalars (real lgpd-data-privacy shape).
    let doc = "---\nname: lgpd-data-privacy  \ndescription: \"LGPD patterns.\"\n---\nbody\n";
    let (meta, _) = parse("lgpd-data-privacy", doc).expect("trailing spaces tolerated");
    assert_eq!(meta.name.as_str(), "lgpd-data-privacy");

    // Quoted description with inner quotes + USE FOR/DO NOT USE FOR lists.
    let doc = "---\nname: keycloak-oidc\ndescription: \"Keycloak OIDC auth. USE FOR: Keycloak integration, SSO, realm role claims. DO NOT USE FOR: IdentityServer, Auth0, Azure AD, non-Keycloak IdP, frontend-only auth.\"\n---\nbody\n";
    let (meta, _) = parse("keycloak-oidc", doc).expect("quoted description");
    assert!(
        meta.triggers.iter().any(|t| t == "Keycloak integration"),
        "derived triggers: {:?}",
        meta.triggers
    );
    assert!(
        meta.negative_triggers.iter().any(|t| t == "Auth0"),
        "derived negative triggers: {:?}",
        meta.negative_triggers
    );

    // Folded multiline description (real caveman/audit shape).
    let doc = "---\nname: folded\ndescription: >-\n  Line one\n  line two.\n---\nbody\n";
    let (meta, _) = parse("folded", doc).expect("folded scalar");
    assert_eq!(meta.description, "Line one line two.");

    // Nested metadata map with flow sequence values (real aspnetcore-health shape).
    let doc = "---\nname: nested\ndescription: d\nmetadata:\n  author: community\n  version: \"1.0.0\"\n  frameworks: [\".NET 8+\", \"Redis\"]\n---\nbody\n";
    let (_, warnings) = parse("nested", doc).expect("nested metadata is structurally valid");
    assert!(
        !warnings
            .iter()
            .any(|w| matches!(w.kind, WarningKind::UnknownKey { .. })),
        "metadata sub-keys are not flagged as unknown top-level keys: {warnings:?}"
    );
}

#[test]
fn invalid_utf8_is_replaced_lossily_with_a_typed_warning() {
    // Simulates the real lgpd-data-privacy file (invalid byte in 'Proteção').
    let mut doc = b"---\nname: lgpd-x\ndescription: LGPD (Lei Geral de Prote".to_vec();
    doc.extend_from_slice(&[0xE7, 0xE3, 0x6F]); // ç + stray byte (invalid sequence)
    doc.extend_from_slice(b" de Dados) patterns.\n---\nbody\n");
    let (meta, warnings) =
        parse_skill_document(Path::new("lgpd-x"), "lgpd-x", &doc).expect("lossy parse");
    assert_eq!(meta.name.as_str(), "lgpd-x");
    assert!(
        warnings
            .iter()
            .any(|w| matches!(w.kind, WarningKind::InvalidUtf8 { .. })),
        "lossy replacement disclosed: {warnings:?}"
    );
}
