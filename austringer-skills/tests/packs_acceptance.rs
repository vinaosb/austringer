//! Acceptance (plan todo 33): shipped domain packs are registry CONTENT
//! (`packs/` at the workspace root, never compiled into the binary) that
//! must load through the REAL T30 loader — parse clean, carry the full
//! routing surface (triggers incl. non-English + negative scope +
//! languages + allowed-tools + worked example + eval scenario), route
//! representative prompts to the right pack, and keep cross-domain
//! prompts OUT (negative scope wins). Assertions target routing
//! DECISIONS and loaded metadata — never file counts alone.

use std::path::{Path, PathBuf};

use austringer_skills::loader::{LockMode, SkillIndex, SkillLoader, Vendor};
use austringer_skills::routing::RoutingDecision;

fn packs_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("packs")
        .canonicalize()
        .expect("packs/ registry content dir must exist at the workspace root")
}

fn load_packs() -> SkillIndex {
    SkillLoader::new()
        .skills_dir(packs_root(), Vendor::Austringer)
        .lock_mode(LockMode::Off)
        .load()
        .expect("packs/ must load through the real T30 loader")
}

/// The shipped set (name, version); task-33 evidence records the count.
const SHIPPED: [(&str, &str); 11] = [
    ("accounting", "1.0.0"),
    ("ai-deployment", "0.1.0"),
    ("audit-exemplar", "0.1.0"),
    ("backend", "1.0.0"),
    ("datacenter-aec", "0.1.0"),
    ("devops", "1.0.0"),
    ("dotnet", "1.0.0"),
    ("finance-investors", "1.0.0"),
    ("frontend", "1.0.0"),
    ("legal", "1.0.0"),
    ("maths", "1.0.0"),
];

fn matched(decision: &RoutingDecision) -> Vec<&str> {
    let mut names: Vec<&str> = decision.matched.iter().map(|m| m.name.as_str()).collect();
    names.sort_unstable();
    names
}

fn assert_routes(index: &SkillIndex, prompt: &str, expected: &str) {
    let decision = index.route_text(prompt, None);
    assert_eq!(
        matched(&decision),
        [expected],
        "prompt {prompt:?} must route to exactly one pack"
    );
    assert!(
        !decision.matched[0].matched_triggers.is_empty(),
        "routing decision must record which trigger fired for {prompt:?}"
    );
}

fn blocked<'a>(
    decision: &'a RoutingDecision,
    name: &str,
) -> &'a austringer_skills::routing::BlockedMatch {
    decision
        .blocked
        .iter()
        .find(|b| b.name.as_str() == name)
        .unwrap_or_else(|| panic!("{name} must be negative-scope BLOCKED, not merely absent"))
}

#[test]
fn all_shipped_packs_parse_clean_and_manifest_stays_in_sync() {
    let index = load_packs();
    assert!(
        index.report().parse_errors.is_empty(),
        "authored packs parse with zero errors: {:?}",
        index.report().parse_errors
    );
    assert!(
        index.report().warnings.is_empty(),
        "authored packs carry zero warnings (no derived fields, no unknown keys): {:?}",
        index.report().warnings
    );
    assert_eq!(index.report().parsed, SHIPPED.len());
    for (name, version) in SHIPPED {
        let meta = index
            .meta(name)
            .unwrap_or_else(|| panic!("pack {name} missing"));
        assert_eq!(meta.version.as_deref(), Some(version), "{name} version");
    }
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(packs_root().join("manifest.json"))
            .expect("packs/manifest.json must be readable"),
    )
    .expect("manifest.json must be valid JSON");
    let entries = manifest["packs"].as_array().expect("packs array");
    assert_eq!(entries.len(), SHIPPED.len(), "manifest lists every pack");
    for entry in entries {
        let name = entry["name"].as_str().expect("name").to_string();
        let meta = index
            .meta(&name)
            .unwrap_or_else(|| panic!("{name} in manifest but not loaded"));
        assert_eq!(
            meta.version.as_deref(),
            Some(entry["version"].as_str().expect("version")),
            "{name} manifest version matches SKILL.md"
        );
        assert!(entry["category"].is_string() && entry["summary"].is_string());
    }
}

#[test]
fn every_pack_carries_the_full_routing_surface_and_worked_example() {
    let index = load_packs();
    for meta in index.metas() {
        let name = meta.name.as_str();
        assert!(!meta.triggers.is_empty(), "{name}: triggers required");
        assert!(
            meta.triggers.iter().any(|t| t == &format!("/{name}")),
            "{name}: /command trigger derived"
        );
        assert!(
            !meta.negative_triggers.is_empty(),
            "{name}: negative scope required"
        );
        assert!(!meta.languages.is_empty(), "{name}: languages required");
        assert!(
            !meta.allowed_tools.is_empty(),
            "{name}: allowed-tools hints required"
        );
        let invoked = index
            .invoke(name)
            .unwrap_or_else(|e| panic!("{name}: invoke must resolve shared partials: {e}"));
        assert!(
            invoked.body.contains("## Worked example"),
            "{name}: worked example required"
        );
        assert!(
            invoked.body.contains("## Eval scenario"),
            "{name}: eval scenario required"
        );
        assert!(
            invoked.body.contains("Pack conventions"),
            "{name}: shared _shared/ partial interpolated into body"
        );
    }
}

#[test]
fn representative_prompts_route_to_the_right_pack() {
    let index = load_packs();
    let cases = [
        (
            "Help me set up a .NET clean architecture project with EF Core and CQRS",
            "dotnet",
        ),
        (
            "Add health checks and Serilog structured logging to my ASP.NET Core service",
            "dotnet",
        ),
        ("run /dotnet to scaffold the webapi", "dotnet"),
        (
            "Make this React component accessible and fix the layout",
            "frontend",
        ),
        (
            "Design REST API error responses with RFC 7807 problem details",
            "backend",
        ),
        (
            "Set up Docker Compose deployment with nginx reverse proxy and TLS",
            "devops",
        ),
        ("legal review of this lease", "legal"),
        ("Prove this integral converges", "maths"),
        (
            "Reconcile the trial balance and prepare journal entries",
            "accounting",
        ),
        (
            "Build a financial model with revenue projections for investors",
            "finance-investors",
        ),
        (
            "audit the codebase for security issues before release",
            "audit-exemplar",
        ),
        (
            "What TPM quota do I need for gpt-5.5 in west-europe?",
            "ai-deployment",
        ),
    ];
    for (prompt, expected) in cases {
        assert_routes(&index, prompt, expected);
    }
}

#[test]
fn non_english_prompts_route_to_the_right_pack() {
    let index = load_packs();
    let cases = [
        (
            "auditar dependências desatualizadas no projeto",
            "audit-exemplar",
        ),
        ("审计代码的安全性，重点看依赖漏洞", "audit-exemplar"),
        ("revisão do contrato de arrendamento do escritório", "legal"),
        ("vérifier la clause abusive dans ce contrat", "legal"),
        (
            "Buchungssätze für die Abschlusserstellung prüfen",
            "accounting",
        ),
        ("montar o balancete de verificação", "accounting"),
        ("résoudre cette équation différentielle", "maths"),
        ("préparer la levée de fonds série A", "finance-investors"),
        ("ativar o modo escuro no site", "frontend"),
        ("déploiement docker compose avec nginx", "devops"),
        ("implantar modelo de ia no foundry", "ai-deployment"),
        (
            "dimensionamento de cabos para o novo datacenter",
            "datacenter-aec",
        ),
        ("审计这栋建筑的配电柜", "datacenter-aec"),
    ];
    for (prompt, expected) in cases {
        assert_routes(&index, prompt, expected);
    }
}

#[test]
fn cross_domain_prompts_do_not_mis_trigger() {
    let index = load_packs();

    // 'audit this electrical panel' — engineering work, not a code audit.
    let d = index.route_text("audit this electrical panel", None);
    assert_eq!(matched(&d), ["datacenter-aec"]);
    let audit = blocked(&d, "audit-exemplar");
    assert!(
        audit.blocked_by.iter().any(|t| t == "electrical panel"),
        "negative scope carries the blocking phrase"
    );

    // Positive code-audit trigger + engineering phrase: negative scope wins.
    let d = index.route_text("security audit of the electrical panel wiring", None);
    assert_eq!(matched(&d), ["datacenter-aec"]);
    let audit = blocked(&d, "audit-exemplar");
    assert!(audit.matched_triggers.iter().any(|t| t == "security audit"));
    assert!(audit.blocked_by.iter().any(|t| t == "electrical panel"));

    // 'audit our sprint process' — process review; no shipped pack owns it.
    let d = index.route_text("audit our sprint process", None);
    assert!(d.matched.is_empty(), "matched: {:?}", matched(&d));

    // 'legal review of this lease' — legal domain only, no audit pack.
    let d = index.route_text("legal review of this lease", None);
    assert_eq!(matched(&d), ["legal"]);

    // Finance trigger inside accounting work: negative scope hands it over.
    let d = index.route_text("due diligence on the journal entries ledger", None);
    assert_eq!(matched(&d), ["accounting"]);
    let fin = blocked(&d, "finance-investors");
    assert!(fin.matched_triggers.iter().any(|t| t == "due diligence"));
    assert!(fin.blocked_by.iter().any(|t| t == "journal entries"));

    // Model-serving words inside plain container deployment stay devops.
    let d = index.route_text("deploy an ai model with docker compose", None);
    assert_eq!(matched(&d), ["devops"]);
    let ai = blocked(&d, "ai-deployment");
    assert!(ai.blocked_by.iter().any(|t| t == "docker compose"));
}
