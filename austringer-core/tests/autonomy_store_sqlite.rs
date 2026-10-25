//! SQLite permission-store tests (todo 8): y/a/n semantics with scope and
//! optional duration, restart round-trip, denials for repetition feedback,
//! the bypass-persistence rejection, schema versioning, and fail-safe
//! behavior on a corrupt database. Durations use an injected clock — no
//! wall clock anywhere in this file.

mod common;

use std::sync::Arc;

use austringer_core::autonomy::{Clock, SqlitePermissionStore};
use austringer_core::inspectors::store::{Grant, GrantScope, PermissionStore, StoreError};
use common::TempSession;

use std::sync::atomic::{AtomicU64, Ordering};

/// Deterministic injected clock (no wall clock in this file): tests
/// advance `ticks` explicitly.
fn step_clock(start: u64) -> (Arc<AtomicU64>, Clock) {
    let ticks = Arc::new(AtomicU64::new(start));
    let clock: Clock = {
        let ticks = Arc::clone(&ticks);
        Arc::new(move || ticks.load(Ordering::SeqCst))
    };
    (ticks, clock)
}

fn advance(ticks: &AtomicU64, by: u64) {
    ticks.fetch_add(by, Ordering::SeqCst);
}

fn grant(tool: &str, target: Option<&str>, scope: GrantScope) -> Grant {
    Grant {
        tool: tool.to_owned(),
        target: target.map(str::to_owned),
        scope,
    }
}

#[test]
fn grants_round_trip_across_restart() {
    let dir = TempSession::new("sqlite-roundtrip");
    let db = dir.path().join("permissions.sqlite3");
    {
        let store = SqlitePermissionStore::open(&db).expect("open");
        store
            .record(&grant(
                "execute_command",
                Some("cargo test"),
                GrantScope::Forever,
            ))
            .expect("record forever");
        store
            .record(&grant(
                "edit_lines",
                Some("src/main.rs"),
                GrantScope::Session,
            ))
            .expect("record session");
    }
    // "Restart": a fresh handle over the same file sees both grants.
    let store = SqlitePermissionStore::open(&db).expect("reopen");
    assert_eq!(
        store.lookup("execute_command", Some("cargo test")),
        Some(GrantScope::Forever)
    );
    assert_eq!(
        store.lookup("edit_lines", Some("src\\main.rs")),
        Some(GrantScope::Session),
        "lookup normalizes Windows separators"
    );
    assert_eq!(store.lookup("edit_lines", None), None);
    // Session clear survives restart too: only forever grants remain.
    store.clear_session().expect("clear");
    assert_eq!(store.lookup("edit_lines", Some("src/main.rs")), None);
    assert_eq!(
        store.lookup("execute_command", Some("cargo test")),
        Some(GrantScope::Forever)
    );
}

#[test]
fn duration_grants_expire_and_stop_honoring() {
    let dir = TempSession::new("sqlite-expiry");
    let db = dir.path().join("permissions.sqlite3");
    let (ticks, clock) = step_clock(1_000);
    let store = SqlitePermissionStore::open_with_clock(&db, clock).expect("open");
    store
        .record_timed(
            &grant("execute_command", Some("cargo build"), GrantScope::Session),
            Some(10),
        )
        .expect("record timed");
    // Inside the window: honored.
    advance(&ticks, 9);
    assert_eq!(
        store.lookup("execute_command", Some("cargo build")),
        Some(GrantScope::Session)
    );
    // Past expiry: stale grants no longer honor, and the row is gone.
    advance(&ticks, 2);
    assert_eq!(store.lookup("execute_command", Some("cargo build")), None);
    drop(store);
    let (_ticks, clock) = step_clock(1_011);
    let reopened = SqlitePermissionStore::open_with_clock(&db, clock).expect("reopen");
    assert_eq!(
        reopened.lookup("execute_command", Some("cargo build")),
        None
    );
}

#[test]
fn denials_persist_for_repetition_feedback() {
    let dir = TempSession::new("sqlite-denials");
    let db = dir.path().join("permissions.sqlite3");
    {
        let store = SqlitePermissionStore::open(&db).expect("open");
        for _ in 0..3 {
            store
                .record_denial("execute_command", Some("rm -rf /x"))
                .expect("deny record");
        }
        assert_eq!(store.denial_count("execute_command", Some("rm -rf /x")), 3);
        assert_eq!(
            store.denial_count("execute_command", Some("cargo build")),
            0
        );
    }
    let store = SqlitePermissionStore::open(&db).expect("reopen");
    assert_eq!(store.denial_count("execute_command", Some("rm -rf /x")), 3);
}

#[test]
fn bypass_grants_are_never_persisted() {
    let dir = TempSession::new("sqlite-bypass");
    let db = dir.path().join("permissions.sqlite3");
    let store = SqlitePermissionStore::open(&db).expect("open");
    for bypass in [
        grant("*", None, GrantScope::Forever),
        grant("*", Some("*"), GrantScope::Session),
        grant("execute_command", Some("*"), GrantScope::Forever),
    ] {
        let error = store.record(&bypass).expect_err("bypass must be rejected");
        assert!(
            error.message.contains("bypass"),
            "typed rejection expected, got: {error}"
        );
    }
    // Nothing leaked into the store.
    assert_eq!(store.lookup("*", None), None);
    assert_eq!(store.lookup("execute_command", Some("*")), None);
}

#[test]
fn corrupt_database_fails_typed_never_panics() {
    let dir = TempSession::new("sqlite-corrupt");
    let db = dir.path().join("permissions.sqlite3");
    std::fs::write(&db, b"this is definitely not a sqlite database at all").expect("seed corrupt");
    let result = SqlitePermissionStore::open(&db);
    match result {
        Err(error) => {
            let text = error.message.to_lowercase();
            assert!(
                text.contains("sqlite") || text.contains("database") || text.contains("corrupt"),
                "typed cause expected, got: {error}"
            );
        }
        Ok(store) => {
            // If open() were lazy, the first lookup must still fail typed
            // (fail-safe: no grant can be honored from a corrupt file).
            let lookup = store.lookup("execute_command", Some("cargo test"));
            assert_eq!(lookup, None);
        }
    }
}

#[test]
fn schema_is_versioned_with_a_migration_hook() {
    let dir = TempSession::new("sqlite-schema");
    let db = dir.path().join("permissions.sqlite3");
    let store = SqlitePermissionStore::open(&db).expect("open");
    assert_eq!(store.schema_version().expect("version"), 1);
    drop(store);
    // A database stamped with a FUTURE schema version is a typed error,
    // never a silent reinterpretation.
    {
        let conn = rusqlite::Connection::open(&db).expect("raw open");
        conn.pragma_update(None, "user_version", 9_999)
            .expect("stamp future");
    }
    let result = SqlitePermissionStore::open(&db);
    let Err(error) = result else {
        panic!("future schema must fail, opened anyway");
    };
    assert!(
        error.message.to_lowercase().contains("schema"),
        "typed schema error expected, got: {error}"
    );
}

#[test]
fn store_error_is_the_typed_surface_for_all_failures() {
    let dir = TempSession::new("sqlite-typed");
    let db = dir.path().join("permissions.sqlite3");
    let store = SqlitePermissionStore::open(&db).expect("open");
    // Sanity: the happy path is typed too (no panics anywhere above).
    store
        .record(&grant("view_window", Some("a.txt"), GrantScope::Session))
        .expect("record");
    let _: Result<(), StoreError> = store.record_denial("view_window", Some("a.txt"));
}
