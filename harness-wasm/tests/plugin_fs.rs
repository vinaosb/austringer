//! Preopen-scoped host-fn tests: virtual-path reads and writes inside a
//! granted scope, read-only runtime enforcement, traversal denial, and
//! not-found typing.

#![cfg(feature = "wasm-plugins")]

mod common;

use common::{grant_with_data, load};
use harness_wasm::error::WasmError;
use harness_wasm::hostfns::{DenyReason, HostEvent};
use harness_wasm::manifest::PreopenMode;

type R = Result<(), Box<dyn std::error::Error>>;

fn take_ok(
    output: Result<Vec<u8>, WasmError>,
    ctx: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    output.map_err(|e| format!("{ctx}: {e}").into())
}

#[test]
fn read_within_scope_works() -> R {
    let dir = tempfile::tempdir()?;
    std::fs::write(dir.path().join("note.txt"), b"{\"note\":\"hi\"}")?;
    let handle = load(
        "fs.wat",
        &grant_with_data(dir.path(), PreopenMode::ReadWrite),
    )?;
    let report = handle.call_raw("read_note", b"{}");
    let out = take_ok(report.output, "read_note must succeed")?;
    assert_eq!(out, b"{\"note\":\"hi\"}".to_vec());
    assert!(
        report
            .events
            .iter()
            .any(|e| matches!(e, HostEvent::ReadOk { path, bytes: 13 } if path == "data/note.txt")),
        "events: {:?}",
        report.events
    );
    Ok(())
}

#[test]
fn write_within_rw_scope_persists() -> R {
    let dir = tempfile::tempdir()?;
    let handle = load(
        "fs.wat",
        &grant_with_data(dir.path(), PreopenMode::ReadWrite),
    )?;
    let report = handle.call_raw("write_note", b"hello-note");
    let out = take_ok(report.output, "write_note must succeed")?;
    assert_eq!(out, b"ok".to_vec());
    assert_eq!(std::fs::read(dir.path().join("out.txt"))?, b"hello-note");
    assert!(
        report
            .events
            .iter()
            .any(|e| matches!(e, HostEvent::WriteOk { path, .. } if path == "data/out.txt")),
        "events: {:?}",
        report.events
    );
    Ok(())
}

#[test]
fn write_to_read_only_scope_denied_at_runtime() -> R {
    let dir = tempfile::tempdir()?;
    let handle = load(
        "fs_ro_break.wat",
        &grant_with_data(dir.path(), PreopenMode::ReadOnly),
    )?;
    let report = handle.call_raw("write_note", b"nope");
    match report.output {
        Err(WasmError::GuestError { code, .. }) => assert_eq!(code, -3),
        other => return Err(format!("expected GuestError, got {other:?}").into()),
    }
    assert!(
        report.events.iter().any(|e| matches!(
            e,
            HostEvent::WriteDenied {
                reason: DenyReason::ReadOnlyScope,
                ..
            }
        )),
        "events: {:?}",
        report.events
    );
    assert!(
        !dir.path().join("out.txt").exists(),
        "the write must not reach disk"
    );
    Ok(())
}

#[test]
fn path_traversal_out_of_scope_denied() -> R {
    let dir = tempfile::tempdir()?;
    // Plant the target one level ABOVE the scope root: a correct host never
    // reads it; a vulnerable host would return its bytes.
    std::fs::write(dir.path().join("outside.txt"), b"secret")?;
    let scope_dir = dir.path().join("scope");
    std::fs::create_dir(&scope_dir)?;
    let handle = load(
        "traversal.wat",
        &grant_with_data(&scope_dir, PreopenMode::ReadOnly),
    )?;
    let report = handle.call_raw("escape", b"{}");
    match report.output {
        Err(WasmError::GuestError { code, .. }) => assert_eq!(code, -2),
        other => return Err(format!("expected GuestError, got {other:?}").into()),
    }
    assert!(
        report.events.iter().any(
            |e| matches!(e, HostEvent::ReadDenied { path, reason: DenyReason::Escape }
                if path == "data/../outside.txt")
        ),
        "events: {:?}",
        report.events
    );
    Ok(())
}

#[test]
fn missing_preopen_file_denied_not_found() -> R {
    let dir = tempfile::tempdir()?;
    let handle = load(
        "fs.wat",
        &grant_with_data(dir.path(), PreopenMode::ReadWrite),
    )?;
    let report = handle.call_raw("read_note", b"{}");
    match report.output {
        Err(WasmError::GuestError { code, .. }) => assert_eq!(code, -4),
        other => return Err(format!("expected GuestError, got {other:?}").into()),
    }
    assert!(
        report.events.iter().any(|e| matches!(
            e,
            HostEvent::ReadDenied {
                reason: DenyReason::NotFound,
                ..
            }
        )),
        "events: {:?}",
        report.events
    );
    Ok(())
}
