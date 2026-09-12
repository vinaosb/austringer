//! Tool behavior version registry (todo 6, Grok Build pattern): versioned
//! tool contracts with an Active/Deprecated/RemovalCandidate lifecycle;
//! sessions pin the version set they started with, so upgrading the
//! registry mid-session cannot churn schemas for existing sessions;
//! deprecation surfaces metadata (replaced-by) without changing the
//! serialized shape.

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use common::TempSession;
use harness_core::tools::{IoCap, Lifecycle, ToolProvider, ToolRegistry, ToolSchema, ToolVersion};
use serde_json::{Value, json};

/// Minimal stub executor for a core tool.
struct EchoExecutor {
    output: &'static str,
}

/// Minimal stub aux tool registered whole (contract + executor).
struct AuxTool {
    name: &'static str,
}

#[async_trait]
impl ToolProvider for EchoExecutor {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "edit_lines".to_owned(),
            description: "echo executor".to_owned(),
            parameters: json!({ "type": "object" }),
        }
    }

    async fn execute(
        &self,
        _arguments: &Value,
    ) -> Result<Vec<u8>, harness_core::tools::ProviderError> {
        Ok(self.output.as_bytes().to_vec())
    }
}

#[async_trait]
impl ToolProvider for AuxTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name.to_owned(),
            description: "late-mounted auxiliary tool".to_owned(),
            parameters: json!({ "type": "object" }),
        }
    }

    async fn execute(
        &self,
        _arguments: &Value,
    ) -> Result<Vec<u8>, harness_core::tools::ProviderError> {
        Ok(b"late".to_vec())
    }
}

fn open_registry(name: &str) -> ToolRegistry {
    ToolRegistry::open(TempSession::new(name).path(), IoCap::DEFAULT).expect("open registry")
}

fn v2_schema() -> ToolSchema {
    ToolSchema {
        name: "edit_lines".to_owned(),
        description: "v2: apply line edits with AST-validated anchors".to_owned(),
        parameters: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "edits": { "type": "array" }
            },
            "required": ["path", "edits"]
        }),
    }
}

#[test]
fn session_pins_versions_when_registry_upgrades() {
    let mut registry = open_registry("version-pin");
    let pinned_v1 = registry.pin_current();
    let before = registry.render_base_schema(&pinned_v1);

    // The registry is upgraded mid-session: edit_lines v2 published.
    let v2 = registry
        .publish_version("edit_lines", v2_schema())
        .expect("publish v2");
    assert_eq!(v2, ToolVersion(2), "publishing appends the next version");

    // The pinned session is unaffected: same pinned version, same bytes.
    let after_for_pinned = registry.render_base_schema(&pinned_v1);
    assert_eq!(
        before.as_bytes(),
        after_for_pinned.as_bytes(),
        "a mid-session upgrade must not churn the pinned session's schema"
    );
    assert_eq!(pinned_v1.version_of("edit_lines"), Some(ToolVersion(1)));

    // A NEW session pins the upgraded version and sees the v2 schema.
    let pinned_v2 = registry.pin_current();
    assert_eq!(pinned_v2.version_of("edit_lines"), Some(v2));
    let upgraded = registry.render_core_schemas(&pinned_v2);
    let parsed: Value = serde_json::from_str(&upgraded).expect("render");
    let edit = parsed
        .as_array()
        .expect("array")
        .iter()
        .find(|schema| schema["name"] == json!("edit_lines"))
        .expect("edit_lines");
    assert_eq!(
        edit["description"],
        json!("v2: apply line edits with AST-validated anchors"),
        "new sessions see the v2 contract"
    );
}

#[tokio::test]
async fn deprecation_surfaces_metadata_without_serialized_churn() {
    let mut registry = open_registry("version-deprecate");
    registry
        .bind_executor("edit_lines", Arc::new(EchoExecutor { output: "ok" }))
        .expect("bind executor");
    let pinned = registry.pin_current();
    let before = registry.render_base_schema(&pinned);
    assert!(registry.deprecations(&pinned).is_empty());

    // Upgrade: v2 published, v1 deprecated with a replaced-by pointer.
    let v2 = registry
        .publish_version("edit_lines", v2_schema())
        .expect("v2");
    registry
        .set_lifecycle(
            "edit_lines",
            ToolVersion(1),
            Lifecycle::Deprecated,
            Some(v2),
        )
        .expect("deprecate v1");

    // The serialized shape for the pinned session is unchanged.
    let after = registry.render_base_schema(&pinned);
    assert_eq!(
        before.as_bytes(),
        after.as_bytes(),
        "deprecation must not change the serialized schema"
    );

    // But the metadata surfaces through the typed notice API...
    let notices = registry.deprecations(&pinned);
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].tool, "edit_lines");
    assert_eq!(notices[0].version, ToolVersion(1));
    assert_eq!(notices[0].lifecycle, Lifecycle::Deprecated);
    assert_eq!(notices[0].replaced_by, Some(v2));

    // ...and on the dispatch path without breaking execution.
    let result = registry
        .dispatch(&pinned, "edit_lines", &json!({ "path": "a.rs" }))
        .await
        .expect("deprecated tool still executes for the pinned session");
    assert_eq!(result.observation, "ok");
    assert!(result.deprecation.is_some());
    assert_eq!(
        result.deprecation.as_ref().map(|n| n.replaced_by),
        Some(Some(v2))
    );
}

#[test]
fn removal_candidate_lifecycle_surfaced() {
    let mut registry = open_registry("version-removal");
    let pinned = registry.pin_current();
    registry
        .set_lifecycle(
            "lsp_query",
            ToolVersion(1),
            Lifecycle::RemovalCandidate,
            None,
        )
        .expect("mark removal candidate");

    let notices = registry.deprecations(&pinned);
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].tool, "lsp_query");
    assert_eq!(notices[0].lifecycle, Lifecycle::RemovalCandidate);
    assert_eq!(notices[0].replaced_by, None);

    // The serialized schema is still unchanged.
    let fresh = registry.pin_current();
    let pristine = open_registry("version-removal-pristine");
    let pristine_pin = pristine.pin_current();
    assert_eq!(
        registry.render_base_schema(&fresh).as_bytes(),
        pristine.render_base_schema(&pristine_pin).as_bytes()
    );
}

#[tokio::test]
async fn late_registered_tool_invisible_to_pinned_session() {
    // Stale-state defense: a session pins the catalog it started with.
    let session = TempSession::new("version-late");
    let mut registry = ToolRegistry::open(session.path(), IoCap::DEFAULT).expect("open");
    let pinned = registry.pin_current();

    // A new tool is mounted after the session started (e.g. an MCP server).
    registry
        .register(Arc::new(AuxTool { name: "web_fetch" }))
        .expect("late mount");
    let error = registry
        .dispatch(&pinned, "web_fetch", &json!({}))
        .await
        .expect_err("invisible to the pinned session");
    assert!(matches!(
        error,
        harness_core::tools::ToolDispatchError::UnknownTool { .. }
    ));
    let search_error = registry
        .search(&pinned, "web_fetch")
        .expect_err("deferred loading cannot surface it either");
    assert!(search_error.suggestions.is_empty());

    // The pinned session's serialized schema is unchanged by the mount.
    let pristine = open_registry("version-late-pristine");
    let pristine_pin = pristine.pin_current();
    assert_eq!(
        registry.render_base_schema(&pinned).as_bytes(),
        pristine.render_base_schema(&pristine_pin).as_bytes()
    );

    // A fresh session DOES see the late-mounted tool.
    let fresh = registry.pin_current();
    assert!(
        registry
            .search(&fresh, "web_fetch")
            .expect("visible")
            .iter()
            .any(|schema| schema.name == "web_fetch")
    );
}
