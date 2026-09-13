//! Plugin LOAD-path tests: manifest extraction, capability gating, the
//! interface-version ladder, and malformed-input handling - all typed.

#![cfg(feature = "wasm-plugins")]

mod common;

use common::{empty_grant, grant_with_data, load};
use harness_wasm::error::WasmError;
use harness_wasm::hostfns::PluginGrant;
use harness_wasm::manifest::{Capability, PreopenMode};

type R = Result<(), Box<dyn std::error::Error>>;

#[test]
fn well_formed_plugin_loads() -> R {
    let dir = tempfile::tempdir()?;
    let handle = load(
        "good.wat",
        &grant_with_data(dir.path(), PreopenMode::ReadOnly),
    )?;
    assert_eq!(handle.manifest().name, "echo-fixture");
    assert_eq!(handle.manifest().tools.len(), 1);
    Ok(())
}

#[test]
fn garbage_wasm_fails_typed() -> R {
    let err = load("garbage.wat", &empty_grant())
        .err()
        .ok_or("expected failure")?;
    assert!(
        matches!(err, WasmError::CompileFailed { ref plugin, .. } if plugin == "garbage"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[test]
fn malformed_manifest_fails_typed() -> R {
    let err = load("bad_manifest.wat", &empty_grant())
        .err()
        .ok_or("expected failure")?;
    assert!(
        matches!(err, WasmError::Manifest { .. }),
        "unexpected error: {err}"
    );
    Ok(())
}

#[test]
fn missing_manifest_fails_typed() -> R {
    let err = load("no_manifest.wat", &empty_grant())
        .err()
        .ok_or("expected failure")?;
    assert!(
        matches!(err, WasmError::ManifestMissing { ref plugin, .. } if plugin == "no_manifest"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[test]
fn declared_but_ungranted_capability_fails_load() -> R {
    let err = load("cap_hog.wat", &empty_grant())
        .err()
        .ok_or("expected failure")?;
    assert!(
        matches!(err, WasmError::CapabilityNotGranted { ref plugin, ref capability }
            if plugin == "cap-hog-fixture" && capability == "process-exec"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[test]
fn granted_capability_loads() -> R {
    let grant = PluginGrant {
        capabilities: vec![Capability::ProcessExec],
        scopes: vec![],
    };
    let handle = load("cap_hog.wat", &grant)?;
    assert_eq!(
        handle.manifest().capabilities,
        vec![Capability::ProcessExec]
    );
    Ok(())
}

#[test]
fn interface_too_old_fails_with_guidance() -> R {
    let err = load("old_iface.wat", &empty_grant())
        .err()
        .ok_or("expected failure")?;
    assert!(
        matches!(
            err,
            WasmError::InterfaceTooOld {
                declared: 0,
                min: 1,
                ..
            }
        ),
        "unexpected error: {err}"
    );
    assert!(
        err.to_string().contains("rebuild"),
        "guidance must tell the user to rebuild the plugin: {err}"
    );
    Ok(())
}

#[test]
fn interface_too_new_fails_with_guidance() -> R {
    let err = load("new_iface.wat", &empty_grant())
        .err()
        .ok_or("expected failure")?;
    assert!(
        matches!(
            err,
            WasmError::InterfaceTooNew {
                declared: 2,
                max: 1,
                ..
            }
        ),
        "unexpected error: {err}"
    );
    assert!(
        err.to_string().contains("upgrade"),
        "guidance must tell the user to upgrade the host: {err}"
    );
    Ok(())
}

#[test]
fn unsatisfied_preopen_fails_load() -> R {
    let err = load("good.wat", &empty_grant())
        .err()
        .ok_or("expected failure")?;
    assert!(
        matches!(err, WasmError::PreopenUnsatisfied { ref scope, .. } if scope == "data"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[test]
fn read_write_preopen_on_read_only_grant_fails_load() -> R {
    let dir = tempfile::tempdir()?;
    let err = load(
        "fs.wat",
        &grant_with_data(dir.path(), PreopenMode::ReadOnly),
    )
    .err()
    .ok_or("expected failure")?;
    assert!(
        matches!(err, WasmError::PreopenModeDenied { ref scope, .. } if scope == "data"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[test]
fn unknown_tool_name_is_typed() -> R {
    let dir = tempfile::tempdir()?;
    let handle = load(
        "good.wat",
        &grant_with_data(dir.path(), PreopenMode::ReadOnly),
    )?;
    let report = handle.call_raw("nope", b"{}");
    assert!(
        matches!(report.output, Err(WasmError::ToolNotDeclared { ref tool, .. }) if tool == "nope"),
        "unexpected outcome: {:?}",
        report.output
    );
    Ok(())
}
