//! Sandbox-enforcement tests: fuel metering, the 32 MiB memory limiter,
//! and the bounded ABI regions. The memory-hog and infinite-loop fixtures
//! are the plan's required "test kills the offender" proofs; the preopen
//! file-surface probes live in `plugin_fs.rs`.

#![cfg(feature = "wasm-plugins")]

mod common;

use austringer_wasm::error::WasmError;
use austringer_wasm::limits::PluginLimits;
use austringer_wasm::manifest::PreopenMode;
use common::{empty_grant, grant_with_data, load};

type R = Result<(), Box<dyn std::error::Error>>;

/// Unwrap the report outcome with a message instead of `expect`.
fn take_ok(
    output: Result<Vec<u8>, WasmError>,
    ctx: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    output.map_err(|e| format!("{ctx}: {e}").into())
}

#[test]
fn echo_tool_round_trips_bytes_and_burns_fuel() -> R {
    let dir = tempfile::tempdir()?;
    let handle = load(
        "good.wat",
        &grant_with_data(dir.path(), PreopenMode::ReadOnly),
    )?;
    let payload = br#"{"hello":"world"}"#;
    let report = handle.call_raw("echo", payload);
    let out = take_ok(report.output, "echo must succeed")?;
    assert_eq!(out, payload.to_vec());
    assert!(report.fuel_consumed > 0, "the call must burn fuel");
    assert!(
        report.logs.iter().any(|l| l.text.contains("hello")),
        "the fixture logs its input: {:?}",
        report.logs
    );
    Ok(())
}

#[test]
fn infinite_loop_is_killed_by_fuel() -> R {
    let handle = load("fuel_hog.wat", &empty_grant())?;
    let report = handle.call_raw("spin", b"{}");
    match report.output {
        Err(WasmError::FuelExhausted {
            budget, consumed, ..
        }) => {
            assert_eq!(budget, PluginLimits::default().fuel_per_call);
            assert!(consumed > 0);
        }
        other => return Err(format!("expected FuelExhausted, got {other:?}").into()),
    }
    Ok(())
}

#[test]
fn memory_hog_is_terminated_cleanly() -> R {
    let handle = load("mem_hog.wat", &empty_grant())?;
    let report = handle.call_raw("hog", b"{}");
    match report.output {
        Err(WasmError::MemoryCapExceeded { limit_bytes, .. }) => {
            assert_eq!(limit_bytes, 32 * 1024 * 1024);
        }
        other => return Err(format!("expected MemoryCapExceeded, got {other:?}").into()),
    }
    Ok(())
}

#[test]
fn configurable_fuel_budget_applies_per_call() -> R {
    let small = PluginLimits {
        fuel_per_call: 1_000,
        ..PluginLimits::default()
    };
    let handle = austringer_wasm::plugin::PluginHandle::load(
        &common::fixture("fuel_hog.wat"),
        &empty_grant(),
        small,
    )?;
    let report = handle.call_raw("spin", b"{}");
    match report.output {
        Err(WasmError::FuelExhausted { budget, .. }) => assert_eq!(budget, 1_000),
        other => return Err(format!("expected FuelExhausted, got {other:?}").into()),
    }
    Ok(())
}

#[test]
fn oversized_input_is_rejected_before_the_call() -> R {
    let dir = tempfile::tempdir()?;
    let handle = load(
        "good.wat",
        &grant_with_data(dir.path(), PreopenMode::ReadOnly),
    )?;
    let big = vec![b'x'; 4097];
    let report = handle.call_raw("echo", &big);
    assert!(
        matches!(
            report.output,
            Err(WasmError::InputTooLarge {
                len: 4097,
                capacity: 4096
            })
        ),
        "unexpected outcome: {:?}",
        report.output
    );
    Ok(())
}

#[test]
fn lying_output_length_is_rejected() -> R {
    let handle = load("lie_result.wat", &empty_grant())?;
    let report = handle.call_raw("liar", b"{}");
    assert!(
        matches!(
            report.output,
            Err(WasmError::OutputTooLarge {
                len: 9999,
                capacity: 4096,
                ..
            })
        ),
        "unexpected outcome: {:?}",
        report.output
    );
    Ok(())
}

#[test]
fn unicode_and_control_chars_round_trip() -> R {
    let dir = tempfile::tempdir()?;
    let handle = load(
        "good.wat",
        &grant_with_data(dir.path(), PreopenMode::ReadOnly),
    )?;
    let payload = "{\"k\":\"héllo世界\u{7}\"}".as_bytes();
    let report = handle.call_raw("echo", payload);
    let out = take_ok(report.output, "unicode echo must succeed")?;
    assert_eq!(out, payload.to_vec());
    Ok(())
}
