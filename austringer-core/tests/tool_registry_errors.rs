//! Typed-error coverage for the tool registry (todo 6): every failure mode
//! is a typed, panic-free error — unknown tools carry closest-match
//! suggestions plus the aux table, the reserved completion tool is never
//! registry-executed, schema-only contracts reject dispatch, and registry
//! mutations validate their targets.

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use austringer_core::tools::{
    IoCap, Lifecycle, ToolDispatchError, ToolProvider, ToolRegistry, ToolSchema, ToolVersion,
};
use common::TempSession;
use serde_json::{Value, json};

/// Minimal stub executor.
struct EchoExecutor {
    output: &'static str,
}

/// Minimal stub aux tool.
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
    ) -> Result<Vec<u8>, austringer_core::tools::ProviderError> {
        Ok(self.output.as_bytes().to_vec())
    }
}

#[async_trait]
impl ToolProvider for AuxTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name.to_owned(),
            description: "auxiliary tool".to_owned(),
            parameters: json!({ "type": "object" }),
        }
    }

    async fn execute(
        &self,
        _arguments: &Value,
    ) -> Result<Vec<u8>, austringer_core::tools::ProviderError> {
        Ok(Vec::new())
    }
}

fn open_registry(name: &str) -> ToolRegistry {
    ToolRegistry::open(TempSession::new(name).path(), IoCap::DEFAULT).expect("open registry")
}

fn any_schema() -> ToolSchema {
    ToolSchema {
        name: "edit_lines".to_owned(),
        description: "any".to_owned(),
        parameters: json!({ "type": "object" }),
    }
}

#[tokio::test]
async fn dispatch_unknown_tool_returns_typed_suggestions() {
    let mut registry = open_registry("errors-unknown");
    registry
        .bind_executor("edit_lines", Arc::new(EchoExecutor { output: "ok" }))
        .expect("bind");
    let pinned = registry.pin_current();

    let error = registry
        .dispatch(&pinned, "view_wind", &json!({}))
        .await
        .expect_err("unknown tool");
    match error {
        ToolDispatchError::UnknownTool {
            ref name,
            ref suggestions,
            ref aux_table,
        } => {
            assert_eq!(name, "view_wind");
            assert!(
                suggestions.contains(&"view_window".to_owned()),
                "closest match listed: {suggestions:?}"
            );
            assert!(!aux_table.is_empty(), "aux table accompanies the error");
        }
        other => panic!("expected UnknownTool, got {other:?}"),
    }
}

#[tokio::test]
async fn reserved_task_completed_rejected_by_registry_dispatch() {
    let registry = open_registry("errors-reserved");
    let pinned = registry.pin_current();
    let error = registry
        .dispatch(&pinned, "task_completed", &json!({ "summary": "done" }))
        .await
        .expect_err("task_completed is loop-enforced, never registry-executed");
    assert!(matches!(error, ToolDispatchError::ReservedTool { .. }));
}

#[tokio::test]
async fn dispatch_without_executor_is_typed_error() {
    let registry = open_registry("errors-no-executor");
    let pinned = registry.pin_current();
    let error = registry
        .dispatch(&pinned, "view_window", &json!({ "path": "a.rs" }))
        .await
        .expect_err("core contract has no executor until todos 16-19");
    assert!(matches!(error, ToolDispatchError::NoExecutor { .. }));
}

#[test]
fn register_rejects_duplicate_tool_name() {
    let mut registry = open_registry("errors-duplicate");
    registry
        .register(Arc::new(AuxTool { name: "web_fetch" }))
        .expect("first registration");
    let error = registry
        .register(Arc::new(AuxTool { name: "web_fetch" }))
        .expect_err("duplicate names rejected");
    assert!(format!("{error}").contains("web_fetch"));

    let error = registry
        .register(Arc::new(AuxTool { name: "edit_lines" }))
        .expect_err("core names are reserved");
    assert!(format!("{error}").contains("edit_lines"));
}

#[test]
fn mutating_unknown_tool_is_typed_error() {
    let mut registry = open_registry("errors-mutate-unknown");
    let error = registry
        .publish_version("nonexistent", any_schema())
        .expect_err("cannot version a tool without a contract");
    assert!(format!("{error}").contains("nonexistent"));
    let error = registry
        .set_lifecycle("nonexistent", ToolVersion(1), Lifecycle::Deprecated, None)
        .expect_err("cannot deprecate a tool without a contract");
    assert!(format!("{error}").contains("nonexistent"));
    let error = registry
        .bind_executor("nonexistent", Arc::new(EchoExecutor { output: "x" }))
        .expect_err("cannot bind an executor to nothing");
    assert!(format!("{error}").contains("nonexistent"));
}

#[test]
fn set_lifecycle_unknown_version_is_typed_error() {
    let mut registry = open_registry("errors-unknown-version");
    let error = registry
        .set_lifecycle("edit_lines", ToolVersion(99), Lifecycle::Deprecated, None)
        .expect_err("version must exist before it can transition");
    assert!(matches!(
        error,
        austringer_core::tools::RegistryError::UnknownVersion { .. }
    ));
}
