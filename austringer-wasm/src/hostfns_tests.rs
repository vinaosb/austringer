//! Unit tests for the host-function behavior (path containment, mode
//! enforcement, log budget) - the security-critical core of the host API.

use crate::hostfns::HostCtx;
use crate::hosttypes::{DenyReason, HostEvent, ResolvedScope};
use crate::manifest::PreopenMode;

type R = Result<(), Box<dyn std::error::Error>>;

fn ctx_ro(dir: &std::path::Path) -> HostCtx {
    HostCtx::new(vec![ResolvedScope {
        name: "data".into(),
        host_dir: dir.into(),
        mode: PreopenMode::ReadOnly,
    }])
}

fn ctx_rw(dir: &std::path::Path) -> HostCtx {
    HostCtx::new(vec![ResolvedScope {
        name: "data".into(),
        host_dir: dir.into(),
        mode: PreopenMode::ReadWrite,
    }])
}

#[test]
fn resolve_contains_reads_inside_scope() -> R {
    let dir = tempfile::tempdir()?;
    std::fs::write(dir.path().join("a.txt"), b"x")?;
    let ctx = ctx_ro(dir.path());
    match ctx.resolve("data/a.txt") {
        Ok((resolved, scope)) => {
            assert!(resolved.ends_with("a.txt"));
            assert_eq!(scope.name, "data");
        }
        Err(reason) => return Err(format!("expected Ok, got {reason:?}").into()),
    }
    Ok(())
}

#[test]
fn resolve_rejects_escape_via_parent() -> R {
    let dir = tempfile::tempdir()?;
    let scope = dir.path().join("scope");
    std::fs::create_dir(&scope)?;
    std::fs::write(dir.path().join("outside.txt"), b"secret")?;
    let ctx = ctx_ro(&scope);
    match ctx.resolve("data/../outside.txt") {
        Err(DenyReason::Escape) => {}
        other => return Err(format!("expected Escape, got {other:?}").into()),
    }
    Ok(())
}

#[test]
fn resolve_rejects_unknown_scope_and_missing_target() -> R {
    let dir = tempfile::tempdir()?;
    let ctx = ctx_ro(dir.path());
    match ctx.resolve("nope/x.txt") {
        Err(DenyReason::OutsideScopes) => {}
        other => return Err(format!("expected OutsideScopes, got {other:?}").into()),
    }
    match ctx.resolve("data/missing.txt") {
        Err(DenyReason::NotFound) => {}
        other => return Err(format!("expected NotFound, got {other:?}").into()),
    }
    Ok(())
}

#[test]
fn write_to_read_only_scope_denied_before_target_existence() -> R {
    let dir = tempfile::tempdir()?;
    let mut ctx = ctx_ro(dir.path());
    // The target does not exist yet - the mode denial must still win.
    match ctx.write_file("data/new.txt", b"x") {
        Err(DenyReason::ReadOnlyScope) => {}
        other => return Err(format!("expected ReadOnlyScope, got {other:?}").into()),
    }
    Ok(())
}

#[test]
fn write_can_create_new_files_inside_rw_scope() -> R {
    let dir = tempfile::tempdir()?;
    let mut ctx = ctx_rw(dir.path());
    match ctx.write_file("data/new.txt", b"x") {
        Ok(1) => {}
        other => return Err(format!("expected Ok(1), got {other:?}").into()),
    }
    assert!(dir.path().join("new.txt").is_file());
    Ok(())
}

#[test]
fn write_escape_is_denied() -> R {
    let dir = tempfile::tempdir()?;
    let scope = dir.path().join("scope");
    std::fs::create_dir(&scope)?;
    let mut ctx = ctx_rw(&scope);
    match ctx.write_file("data/../evil.txt", b"x") {
        Err(reason) => assert!(
            reason == DenyReason::Escape || reason == DenyReason::NotFound,
            "escape must never write outside, got {reason:?}"
        ),
        Ok(n) => return Err(format!("escape write must not succeed ({n} bytes)").into()),
    }
    assert!(
        !dir.path().join("evil.txt").exists(),
        "no file may appear outside the scope"
    );
    Ok(())
}

#[test]
fn log_budget_is_bounded() -> R {
    let dir = tempfile::tempdir()?;
    let mut ctx = ctx_ro(dir.path());
    for _ in 0..100 {
        ctx.record_log(0, "line");
    }
    let (logs, events) = ctx.into_parts();
    assert_eq!(logs.len(), 64);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, HostEvent::LogDropped { .. }))
    );
    Ok(())
}
