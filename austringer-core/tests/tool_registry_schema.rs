//! Tool-registry schema invariance (todo 6): the base schema serializes
//! exactly the six core tools in canonical order, is byte-identical across
//! turns with different active tools, is invariant across differently
//! populated catalogs ("modes"/"active skills" never vary the core set),
//! and renders deterministically (sorted keys, no timestamps).

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use austringer_core::tools::{CORE_TOOLS, IoCap, ToolProvider, ToolRegistry, ToolSchema};
use common::TempSession;
use serde_json::{Value, json};

/// Stub provider with a fixed schema and output, for aux registration.
struct Stub {
    name: String,
    description: String,
    output: Vec<u8>,
}

impl Stub {
    fn new(name: &str, description: &str, output: &[u8]) -> Self {
        Self {
            name: name.to_owned(),
            description: description.to_owned(),
            output: output.to_vec(),
        }
    }
}

#[async_trait]
impl ToolProvider for Stub {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name.clone(),
            description: self.description.clone(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }),
        }
    }

    async fn execute(
        &self,
        _arguments: &Value,
    ) -> Result<Vec<u8>, austringer_core::tools::ProviderError> {
        Ok(self.output.clone())
    }
}

fn open_registry(name: &str) -> ToolRegistry {
    ToolRegistry::open(TempSession::new(name).path(), IoCap::DEFAULT).expect("open registry")
}

#[test]
fn base_schema_lists_exactly_six_core_tools_in_canonical_order() {
    let registry = open_registry("schema-core-set");
    let pinned = registry.pin_current();
    let rendered = registry.render_base_schema(&pinned);
    let parsed: Value = serde_json::from_str(&rendered).expect("base schema is valid JSON");

    let names: Vec<String> = parsed["core_tools"]
        .as_array()
        .expect("core_tools array")
        .iter()
        .map(|schema| schema["name"].as_str().expect("name").to_owned())
        .collect();
    assert_eq!(
        names,
        CORE_TOOLS.to_vec(),
        "core set must be the six canonical tools in canonical order"
    );
    for schema in parsed["core_tools"].as_array().expect("array") {
        assert!(
            !schema["description"]
                .as_str()
                .expect("description")
                .is_empty(),
            "every core schema carries a description"
        );
        assert!(
            schema["parameters"]
                .as_object()
                .is_some_and(|parameters| !parameters.is_empty()),
            "every core schema carries parameters"
        );
    }
    assert!(
        rendered.contains("aux_tools_table"),
        "base schema embeds the aux table section"
    );
}

#[tokio::test]
async fn base_schema_byte_identical_across_turns_with_different_active_tools() {
    // One session catalog: core tools plus an extra tool registered up front.
    let session = TempSession::new("schema-invariance");
    let mut registry = ToolRegistry::open(session.path(), IoCap::DEFAULT).expect("open registry");
    registry
        .register(Arc::new(Stub::new(
            "grep_search",
            "Search project files with a regex pattern.",
            b"match at src/main.rs:12",
        )))
        .expect("register aux tool");
    let pinned = registry.pin_current();

    // Turn 1: only core tools are active.
    let turn1 = registry.render_base_schema(&pinned);

    // Turn 2: the extra tool is hydrated via tool_search AND used.
    let hydrated = registry
        .dispatch(&pinned, "tool_search", &json!({ "query": "grep_search" }))
        .await
        .expect("hydrate");
    assert!(
        hydrated.observation.contains("grep_search"),
        "hydration reached the observation"
    );
    registry
        .dispatch(&pinned, "grep_search", &json!({ "path": "src/main.rs" }))
        .await
        .expect("use hydrated tool");
    let turn2 = registry.render_base_schema(&pinned);

    // Turn 3: back to core-only activity.
    let turn3 = registry.render_base_schema(&pinned);

    // Byte identity of the raw serialized bytes, not just equality-of-parse.
    assert_eq!(turn1.as_bytes(), turn2.as_bytes());
    assert_eq!(turn2.as_bytes(), turn3.as_bytes());
    assert_eq!(turn1.as_bytes(), turn3.as_bytes());
    // Cross-check: they are also equal as parsed documents (the byte assert
    // above is the binding one; this rules out a shared rendering bug).
    let parse_turn1: Value = serde_json::from_str(&turn1).expect("parse turn1");
    let parse_turn3: Value = serde_json::from_str(&turn3).expect("parse turn3");
    assert_eq!(parse_turn1, parse_turn3);
}

#[test]
fn core_schemas_invariant_across_modes_and_skills() {
    // "Mode"/"active skills" map to differently populated catalogs; the
    // registry output for the six core tools must never vary.
    let core_only = open_registry("schema-mode-core");
    let pinned_core = core_only.pin_current();
    let core_render = core_only.render_core_schemas(&pinned_core);

    let mut loaded = open_registry("schema-mode-loaded");
    for index in 0..5 {
        loaded
            .register(Arc::new(Stub::new(
                &format!("skill_tool_{index}"),
                "A skill-provided auxiliary tool.",
                b"",
            )))
            .expect("register skill tool");
    }
    let pinned_loaded = loaded.pin_current();
    let loaded_render = loaded.render_core_schemas(&pinned_loaded);

    assert_eq!(
        core_render.as_bytes(),
        loaded_render.as_bytes(),
        "core tool schemas must be invariant regardless of active skills"
    );
    // The full base schema differs only in the aux table section.
    let base_core = core_only.render_base_schema(&pinned_core);
    let base_loaded = loaded.render_base_schema(&pinned_loaded);
    assert_ne!(base_core, base_loaded);
    assert!(base_loaded.contains("skill_tool_0"));
}

#[test]
fn schema_rendering_is_deterministic_under_scrambled_key_insertion() {
    let mut rng = common::XorShift64::new(0x600D_F00D);
    for iteration in 0..25 {
        let session = TempSession::new(&format!("schema-det-{iteration}"));
        let mut registry = ToolRegistry::open(session.path(), IoCap::DEFAULT).expect("open");
        // Insertion order is scrambled by the generator; rendering must emit
        // sorted keys (serde_json default) regardless.
        let scrambled = rng.json_value(3);
        let probe = ToolSchema {
            name: "probe_tool".to_owned(),
            description: "determinism probe".to_owned(),
            parameters: scrambled,
        };
        registry
            .register(Arc::new(Stub::new("probe_tool", "determinism probe", b"")))
            .expect("register probe");
        registry
            .publish_version("probe_tool", probe.clone())
            .expect("publish scrambled schema");
        let pinned = registry.pin_current();

        let first = registry.render_base_schema(&pinned);
        let second = registry.render_base_schema(&pinned);
        assert_eq!(first.as_bytes(), second.as_bytes(), "render is pure");

        // A fresh registry with the same logical content renders identically:
        // no ambient data (timestamps, paths, iteration counters) leaks in.
        let twin_session = TempSession::new(&format!("schema-det-twin-{iteration}"));
        let mut twin = ToolRegistry::open(twin_session.path(), IoCap::DEFAULT).expect("open twin");
        twin.register(Arc::new(Stub::new("probe_tool", "determinism probe", b"")))
            .expect("register twin probe");
        twin.publish_version("probe_tool", probe)
            .expect("publish twin schema");
        let pinned_twin = twin.pin_current();
        let twin_render = twin.render_base_schema(&pinned_twin);
        assert_eq!(
            first.as_bytes(),
            twin_render.as_bytes(),
            "identical logical state renders byte-identically"
        );
    }
}
