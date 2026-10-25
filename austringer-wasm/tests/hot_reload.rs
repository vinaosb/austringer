//! Hot-reload semantics: swapped plugin files apply to NEW calls only,
//! in-flight snapshots finish on the old instance, failed reloads keep the
//! old instance serving, and the polling watcher seam emits events.

#![cfg(feature = "wasm-plugins")]

mod common;

use std::time::Duration;

use austringer_wasm::error::WasmError;
use austringer_wasm::manifest::PreopenMode;
use austringer_wasm::plugin::{PluginHandle, PluginWatcher, ReloadEvent, ReloadOutcome};
use common::grant_with_data;

type R = Result<(), Box<dyn std::error::Error>>;

/// Copy a fixture into a scratch dir so the pristine tree stays untouched,
/// returning the plugin path.
fn staged(name: &str, dir: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    let path = dir.join("echo.wat");
    std::fs::copy(common::fixture(name), &path)?;
    Ok(path)
}

fn load_staged(path: &std::path::Path, scope: &std::path::Path) -> Result<PluginHandle, WasmError> {
    PluginHandle::load(
        path,
        &grant_with_data(scope, PreopenMode::ReadOnly),
        Default::default(),
    )
}

fn echo(handle: &PluginHandle) -> Result<Vec<u8>, WasmError> {
    handle.call_raw("echo", b"{\"v\":1}").output
}

#[test]
fn poll_swap_applies_to_new_calls() -> R {
    let dir = tempfile::tempdir()?;
    let scope = tempfile::tempdir()?;
    let path = staged("good.wat", dir.path())?;
    let handle = load_staged(&path, scope.path())?;

    assert!(matches!(handle.poll_once(), ReloadOutcome::Unchanged));
    let before = echo(&handle)?;
    assert_eq!(before, br#"{"v":1}"#.to_vec());

    std::fs::write(&path, common::fixture_bytes("good2.wat")?)?;
    assert!(matches!(handle.poll_once(), ReloadOutcome::Swapped));
    let after = echo(&handle)?;
    assert_eq!(after, br#"{"v":1}-v2"#.to_vec());
    Ok(())
}

#[test]
fn in_flight_snapshot_finishes_on_the_old_instance() -> R {
    let dir = tempfile::tempdir()?;
    let scope = tempfile::tempdir()?;
    let path = staged("good.wat", dir.path())?;
    let handle = load_staged(&path, scope.path())?;

    // "In flight": the snapshot pins the instance a call would be using
    // while the file is swapped underneath it.
    let in_flight = handle.snapshot();
    std::fs::write(&path, common::fixture_bytes("good2.wat")?)?;
    handle.reload()?;

    let old = in_flight.call_raw("echo", br#"{"v":1}"#).output;
    assert_eq!(
        old?,
        br#"{"v":1}"#.to_vec(),
        "old instance keeps old behavior"
    );

    let new = echo(&handle)?;
    assert_eq!(new, br#"{"v":1}-v2"#.to_vec(), "new calls see the swap");
    Ok(())
}

#[test]
fn failed_reload_keeps_serving_the_old_instance() -> R {
    let dir = tempfile::tempdir()?;
    let scope = tempfile::tempdir()?;
    let path = staged("good.wat", dir.path())?;
    let handle = load_staged(&path, scope.path())?;

    std::fs::write(&path, b"this is not a module")?;
    match handle.poll_once() {
        ReloadOutcome::Failed(WasmError::CompileFailed { .. }) => {}
        other => return Err(format!("expected Failed(CompileFailed), got {other:?}").into()),
    }
    // Typed swap semantics: the botched swap leaves the old plugin serving.
    assert_eq!(echo(&handle)?, br#"{"v":1}"#.to_vec());
    Ok(())
}

#[test]
fn watcher_thread_emits_reload_events() -> R {
    let dir = tempfile::tempdir()?;
    let scope = tempfile::tempdir()?;
    let path = staged("good.wat", dir.path())?;
    let handle = load_staged(&path, scope.path())?;

    let guard = PluginWatcher::spawn(handle, Duration::from_millis(10))?;
    std::fs::write(&path, common::fixture_bytes("good2.wat")?)?;

    match guard.events().recv_timeout(Duration::from_secs(3)) {
        Ok(ReloadEvent::Reloaded { plugin }) => assert_eq!(plugin, "echo-fixture"),
        Ok(other) => return Err(format!("expected Reloaded, got {other:?}").into()),
        Err(_) => return Err("watcher emitted no event within 3s".into()),
    }
    Ok(())
}
