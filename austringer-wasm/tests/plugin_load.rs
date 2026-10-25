//! Plugin LOAD-path tests: manifest extraction, capability gating, the
//! interface-version ladder, and malformed-input handling - all typed.

#![cfg(feature = "wasm-plugins")]

mod common;

use austringer_wasm::error::WasmError;
use austringer_wasm::hostfns::PluginGrant;
use austringer_wasm::manifest::{Capability, PreopenMode};
use common::{empty_grant, grant_with_data, load};

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
                declared: 3,
                max: 2,
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
fn interface1_fixture_plugin_loads_unchanged_on_the_interface2_host() -> R {
    // Given: an interface-1 fixture plugin and the host whose ladder now
    // tops out at interface 2 (M6 seam).
    let dir = tempfile::tempdir()?;
    let handle = load(
        "good.wat",
        &grant_with_data(dir.path(), PreopenMode::ReadOnly),
    )?;
    // Then: it still loads, still declares interface 1 - the ladder
    // never breaks old plugins (Zed 10-WIT-versions lesson).
    assert_eq!(handle.manifest().interface, 1);
    assert_eq!(handle.manifest().name, "echo-fixture");
    Ok(())
}

#[test]
fn media_adapter_interface2_plugin_loads_when_granted() -> R {
    // Given: an interface-2 plugin declaring the media-adapter capability
    // (manifest validation seam; v0.2 has NO host-fn impls - the plugin
    // imports nothing beyond the interface-1 ABI table).
    let grant = PluginGrant {
        capabilities: vec![Capability::MediaAdapter],
        scopes: vec![],
    };
    let handle = load("media_adapter.wat", &grant)?;
    // Then: it negotiates interface 2 and the capability round-trips.
    assert_eq!(handle.manifest().interface, 2);
    assert_eq!(
        handle.manifest().capabilities,
        vec![Capability::MediaAdapter]
    );
    Ok(())
}

#[test]
fn media_adapter_capability_ungranted_fails_load_typed() -> R {
    // Given: the same plugin with an empty grant.
    let err = load("media_adapter.wat", &empty_grant())
        .err()
        .ok_or("expected failure")?;
    // Then: declared-but-not-granted fails the load - never a silent
    // escalation.
    assert!(
        matches!(
            err,
            WasmError::CapabilityNotGranted { ref plugin, ref capability }
                if plugin == "media-adapter-fixture" && capability == "media-adapter"
        ),
        "unexpected error: {err}"
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
