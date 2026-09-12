//! SQLite permission persistence (plan todo 8): the durable
//! [`PermissionStore`] behind T7's trait.
//!
//! y/a/n semantics: a `y` answer grants once (never stored); an `a`
//! answer stores a grant for the pattern with a scope (session survives
//! until cleared, forever survives restart) and an optional
//! duration/expiry honored through an injected logical clock; an `n`
//! answer is recorded as a denial counter feeding repetition feedback.
//!
//! The schema is versioned (`PRAGMA user_version`) with a migration hook
//! (the todo-24 discipline, previewed here): each migration is a plain
//! function in [`MIGRATIONS`], applied in order; a database stamped with
//! a FUTURE version is a typed error, never a silent reinterpretation. A
//! corrupt or unreadable file fails opening with a typed [`StoreError`]
//! (fail-safe: the caller falls back to asking, never to allowing).
//!
//! `bypassPermissions`-style grants (`tool` = `*` or `target` = `*`) are
//! REJECTED with a typed error and never persisted.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, OptionalExtension};

use crate::inspectors::store::{Grant, GrantScope, PermissionStore, StoreError};

/// A logical clock: `now` in ticks. Tests inject a counter (deterministic,
/// no wall clock); production uses seconds since the Unix epoch.
pub type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

/// The schema version this build writes.
const SCHEMA_VERSION: u32 = 1;

type Migration = fn(&Connection) -> Result<(), StoreError>;

/// Ordered migrations: `MIGRATIONS[n]` takes a database at `user_version
/// = n` to `n + 1`.
const MIGRATIONS: &[Migration] = &[v1];

fn v1(conn: &Connection) -> Result<(), StoreError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS grants (
             tool       TEXT NOT NULL,
             target     TEXT,
             scope      TEXT NOT NULL,
             expires_at INTEGER,
             PRIMARY KEY (tool, target)
         );
         CREATE TABLE IF NOT EXISTS denials (
             tool  TEXT NOT NULL,
             target TEXT,
             count INTEGER NOT NULL DEFAULT 1,
             PRIMARY KEY (tool, target)
         );",
    )
    .map_err(sql_error)
}

fn sql_error(error: rusqlite::Error) -> StoreError {
    StoreError {
        message: format!("sqlite: {error}"),
    }
}

fn wall_clock() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|span| span.as_secs())
        .unwrap_or(0)
}

fn bypass_rejected() -> StoreError {
    StoreError {
        message: "bypass permissions rejected: wildcard grants are never persisted".to_owned(),
    }
}

/// The SQLite-backed permission store. One connection behind a mutex
/// (the trait takes `&self`); clones share nothing — reopen instead to
/// model a restart.
pub struct SqlitePermissionStore {
    conn: Mutex<Connection>,
    clock: Clock,
}

impl SqlitePermissionStore {
    /// Opens (creating if absent) the store at `path` with the wall clock.
    ///
    /// # Errors
    /// Typed [`StoreError`] when the file is unreadable, corrupt, or
    /// stamped with a future schema version.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        Self::open_with_clock(path, Arc::new(wall_clock))
    }

    /// Opens the store with an injected logical clock (determinism in
    /// tests; expiry math is entirely clock-relative).
    ///
    /// # Errors
    /// Same typed conditions as [`Self::open`].
    pub fn open_with_clock(path: &Path, clock: Clock) -> Result<Self, StoreError> {
        let conn = Connection::open(path).map_err(sql_error)?;
        Self::migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            clock,
        })
    }

    /// The persisted schema version.
    ///
    /// # Errors
    /// Typed [`StoreError`] on storage failure.
    pub fn schema_version(&self) -> Result<u32, StoreError> {
        self.conn
            .lock()
            .map_err(|_| StoreError {
                message: "sqlite store mutex poisoned".to_owned(),
            })?
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .map(|version| u32::try_from(version).unwrap_or(0))
            .map_err(sql_error)
    }

    fn migrate(conn: &Connection) -> Result<(), StoreError> {
        let version = conn
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .map_err(sql_error)?;
        let version = u32::try_from(version).unwrap_or(0);
        if version > SCHEMA_VERSION {
            return Err(StoreError {
                message: format!(
                    "permission store schema {version} is newer than this build ({SCHEMA_VERSION}); upgrade the harness first"
                ),
            });
        }
        for step in MIGRATIONS.iter().take((SCHEMA_VERSION - version) as usize) {
            step(conn)?;
        }
        if version != SCHEMA_VERSION {
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)
                .map_err(sql_error)?;
        }
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, StoreError> {
        self.conn.lock().map_err(|_| StoreError {
            message: "sqlite store mutex poisoned".to_owned(),
        })
    }
}

fn scope_text(scope: GrantScope) -> &'static str {
    match scope {
        GrantScope::Session => "session",
        GrantScope::Forever => "forever",
    }
}

fn scope_parse(text: &str) -> Option<GrantScope> {
    match text {
        "session" => Some(GrantScope::Session),
        "forever" => Some(GrantScope::Forever),
        _ => None,
    }
}

fn ensure_not_bypass(grant: &Grant) -> Result<(), StoreError> {
    if grant.tool == "*" || grant.target.as_deref() == Some("*") {
        return Err(bypass_rejected());
    }
    Ok(())
}

impl PermissionStore for SqlitePermissionStore {
    fn record(&self, grant: &Grant) -> Result<(), StoreError> {
        ensure_not_bypass(grant)?;
        self.record_timed(grant, None)
    }

    fn record_timed(&self, grant: &Grant, duration: Option<u64>) -> Result<(), StoreError> {
        ensure_not_bypass(grant)?;
        let expires_at = duration
            .map(|ticks| (self.clock)() + ticks)
            .and_then(|expiry| i64::try_from(expiry).ok());
        self.lock()?
            .execute(
                "INSERT INTO grants (tool, target, scope, expires_at) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (tool, target) DO UPDATE SET scope = ?3, expires_at = ?4",
                rusqlite::params![
                    grant.tool,
                    grant
                        .target
                        .as_ref()
                        .map(|target| SqlValue::Text(target.replace('\\', "/")))
                        .unwrap_or(SqlValue::Null),
                    scope_text(grant.scope),
                    expires_at,
                ],
            )
            .map(|_| ())
            .map_err(sql_error)
    }

    fn lookup(&self, tool: &str, target: Option<&str>) -> Option<GrantScope> {
        let now = (self.clock)();
        let row = self.lock().ok().and_then(|conn| {
            conn.query_row(
                "SELECT scope, expires_at FROM grants WHERE tool = ?1 AND target IS ?2",
                rusqlite::params![
                    tool,
                    target
                        .map(|target| SqlValue::Text(target.replace('\\', "/")))
                        .unwrap_or(SqlValue::Null),
                ],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?)),
            )
            .optional()
            .ok()
            .flatten()
        });
        let (scope_text, expires_at) = row?;
        if let Some(expiry) = expires_at
            && u64::try_from(expiry).unwrap_or(0) <= now
        {
            // Stale grant: stop honoring it and lazily drop the row.
            if let Ok(conn) = self.lock() {
                let _ = conn.execute(
                    "DELETE FROM grants WHERE tool = ?1 AND target IS ?2",
                    rusqlite::params![
                        tool,
                        target
                            .map(|target| SqlValue::Text(target.replace('\\', "/")))
                            .unwrap_or(SqlValue::Null),
                    ],
                );
            }
            return None;
        }
        scope_parse(&scope_text)
    }

    fn clear_session(&self) -> Result<(), StoreError> {
        self.lock()?
            .execute("DELETE FROM grants WHERE scope = 'session'", [])
            .map(|_| ())
            .map_err(sql_error)
    }

    fn record_denial(&self, tool: &str, target: Option<&str>) -> Result<(), StoreError> {
        self.lock()?
            .execute(
                "INSERT INTO denials (tool, target, count) VALUES (?1, ?2, 1)
                 ON CONFLICT (tool, target) DO UPDATE SET count = count + 1",
                rusqlite::params![
                    tool,
                    target
                        .map(|target| SqlValue::Text(target.replace('\\', "/")))
                        .unwrap_or(SqlValue::Null),
                ],
            )
            .map(|_| ())
            .map_err(sql_error)
    }

    fn denial_count(&self, tool: &str, target: Option<&str>) -> u32 {
        self.lock()
            .ok()
            .and_then(|conn| {
                conn.query_row(
                    "SELECT count FROM denials WHERE tool = ?1 AND target IS ?2",
                    rusqlite::params![
                        tool,
                        target
                            .map(|target| SqlValue::Text(target.replace('\\', "/")))
                            .unwrap_or(SqlValue::Null),
                    ],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .ok()
                .flatten()
            })
            .and_then(|count| u32::try_from(count).ok())
            .unwrap_or(0)
    }
}
