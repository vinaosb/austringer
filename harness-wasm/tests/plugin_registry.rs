//! T6-registry integration: plugin tools mount like any other tool, the
//! versioned-contract registry governs them, results flow through the IoCap
//! path, and untrusted plugin output is bounded.

#![cfg(feature = "wasm-plugins")]

mod common;

use common::grant_with_data;
use harness_core::tools::{IoCap, Lifecycle, RegistryError, ToolRegistry, ToolSchema, ToolVersion};
use harness_wasm::error::WasmError;
use harness_wasm::manifest::PreopenMode;
use harness_wasm::plugin::PluginHandle;
use harness_wasm::provider::mount_plugin;
use serde_json::json;

type R = Result<(), Box<dyn std::error::Error>>;

fn loaded_echo(dir: &std::path::Path) -> Result<PluginHandle, WasmError> {
    PluginHandle::load(
        &common::fixture("good.wat"),
        &grant_with_data(dir, PreopenMode::ReadOnly),
        Default::default(),
    )
}

#[tokio::test]
async fn plugin_tool_mounts_and_round_trips_through_the_registry() -> R {
    let session = tempfile::tempdir()?;
    let mut registry = ToolRegistry::open(session.path(), IoCap::DEFAULT)?;
    let handle = loaded_echo(session.path())?;
    mount_plugin(&mut registry, &handle)?;

    let pinned = registry.pin_current();
    let result = registry
        .dispatch(&pinned, "echo", &json!({"hello": "plugin"}))
        .await?;
    // Tool RESULTS asserted (not just success): the observation must carry
    // the round-tripped JSON the fixture produced.
    assert!(
        result.observation.contains("\"hello\":\"plugin\""),
        "observation: {}",
        result.observation
    );
    assert!(result.spill.is_none(), "small result stays inline");
    assert_eq!(pinned.version_of("echo"), Some(ToolVersion(1)));
    Ok(())
}

#[tokio::test]
async fn version_registry_governs_plugin_tools() -> R {
    let session = tempfile::tempdir()?;
    let mut registry = ToolRegistry::open(session.path(), IoCap::DEFAULT)?;
    let handle = loaded_echo(session.path())?;
    mount_plugin(&mut registry, &handle)?;

    // Pin the live contract, then publish v2 and deprecate v1 underneath it:
    // an older session pinned to v1 must keep working WITH a notice.
    let pinned = registry.pin_current();
    let v2_schema = ToolSchema {
        name: "echo".into(),
        description: "echoes the tool input verbatim (v2 contract)".into(),
        parameters: json!({"type": "object"}),
    };
    let v2 = registry.publish_version("echo", v2_schema)?;
    registry.set_lifecycle("echo", ToolVersion(1), Lifecycle::Deprecated, Some(v2))?;

    let notices = registry.deprecations(&pinned);
    assert_eq!(notices.len(), 1, "pinned v1 must be flagged");
    let result = registry
        .dispatch(&pinned, "echo", &json!({"pinned": true}))
        .await?;
    assert!(
        result.observation.contains("\"pinned\":true"),
        "observation: {}",
        result.observation
    );
    let notice = result.deprecation.ok_or("deprecation notice missing")?;
    assert_eq!(notice.version, ToolVersion(1));
    assert_eq!(notice.lifecycle, Lifecycle::Deprecated);
    Ok(())
}

#[tokio::test]
async fn duplicate_plugin_tool_name_fails_typed() -> R {
    let session = tempfile::tempdir()?;
    let mut registry = ToolRegistry::open(session.path(), IoCap::DEFAULT)?;
    let first = loaded_echo(session.path())?;
    let second = loaded_echo(session.path())?;
    mount_plugin(&mut registry, &first)?;
    let err = mount_plugin(&mut registry, &second)
        .err()
        .ok_or("expected duplicate failure")?;
    assert!(
        matches!(err, RegistryError::DuplicateTool { .. }),
        "unexpected error: {err}"
    );
    Ok(())
}

#[tokio::test]
async fn weird_arguments_survive_the_plugin_boundary() -> R {
    let session = tempfile::tempdir()?;
    let mut registry = ToolRegistry::open(session.path(), IoCap::DEFAULT)?;
    let handle = loaded_echo(session.path())?;
    mount_plugin(&mut registry, &handle)?;
    let pinned = registry.pin_current();

    for args in [
        json!(null),
        json!({"nested": {"deep": [1, 2, {"x": "y"}]}}),
        json!({"cjk": "中文キリル编制"}),
        json!([true, false]),
    ] {
        let result = registry.dispatch(&pinned, "echo", &args).await?;
        let expected = serde_json::to_string(&args)?;
        assert!(
            result.observation.contains(&expected),
            "args {args} must round-trip; observation: {}",
            result.observation
        );
    }
    Ok(())
}
