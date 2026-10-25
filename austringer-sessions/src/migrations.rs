//! Schema-versioned migrations with mandatory down-steps.
//!
//! Registry discipline (the dsh lesson): every generation is a pair of
//! `up`/`down` functions; a generation without a working downgrade is
//! never added. `user_version` is the stamp. A database stamped with a
//! future version is a typed [`SessionError::Schema`], never a silent
//! reinterpretation. Round-trip tests in this module walk every
//! generation in both directions and compare schema fingerprints.

use rusqlite::Connection;

use crate::error::SessionError;

const V1_SCHEMA: &str = "CREATE TABLE sessions (
    id            TEXT PRIMARY KEY,
    title         TEXT NOT NULL DEFAULT '',
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    last_seq      INTEGER NOT NULL DEFAULT 0,
    event_count   INTEGER NOT NULL DEFAULT 0,
    turn_count    INTEGER NOT NULL DEFAULT 0,
    payload_bytes INTEGER NOT NULL DEFAULT 0,
    encrypted     INTEGER NOT NULL DEFAULT 0,
    kdf_salt      BLOB,
    kdf_params    TEXT,
    wrapped_key   BLOB,
    forked_from   TEXT,
    pinned        INTEGER NOT NULL DEFAULT 0,
    archived_at   INTEGER
);
CREATE TABLE events (
    session_id TEXT NOT NULL,
    seq        INTEGER NOT NULL,
    kind       TEXT NOT NULL,
    payload    BLOB NOT NULL,
    PRIMARY KEY (session_id, seq)
) WITHOUT ROWID;
CREATE TABLE turn_index (
    session_id TEXT NOT NULL,
    turn       INTEGER NOT NULL,
    start_seq  INTEGER NOT NULL,
    end_seq    INTEGER NOT NULL,
    PRIMARY KEY (session_id, turn)
) WITHOUT ROWID;";

const V2_SCHEMA: &str =
    "CREATE VIRTUAL TABLE event_fts USING fts5(session_id UNINDEXED, seq UNINDEXED, text);
CREATE VIRTUAL TABLE memory_fts USING fts5(note_id UNINDEXED, title, body);
CREATE TABLE memory_notes (
    id         INTEGER PRIMARY KEY,
    session_id TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    title      TEXT NOT NULL,
    body       TEXT NOT NULL
);
CREATE TABLE leases (
    session_id   TEXT PRIMARY KEY,
    owner_id     TEXT NOT NULL,
    boot_id      TEXT NOT NULL,
    pid          INTEGER NOT NULL,
    acquired_at  INTEGER NOT NULL,
    heartbeat_at INTEGER NOT NULL
);
CREATE TABLE store_state (
    key   TEXT PRIMARY KEY,
    value INTEGER NOT NULL
);
ALTER TABLE sessions ADD COLUMN imported_from TEXT;
ALTER TABLE sessions ADD COLUMN imported_unknown INTEGER;";

/// The schema version this build writes.
pub(crate) const SCHEMA_VERSION: u32 = 2;

/// One schema generation: `up` moves `user_version` from `version - 1`
/// to `version`. A generation is only valid with a matching down-step
/// in [`DOWNS`] (same index) — one-way generations are forbidden.
pub(crate) struct Generation {
    /// Version after `up` runs.
    pub(crate) version: u32,
    /// Forward step.
    pub(crate) up: fn(&Connection) -> Result<(), SessionError>,
}

#[cfg(test)]
type DownStep = fn(&Connection) -> Result<(), SessionError>;

/// Down-steps, index-matched to [`GENERATIONS`]: `DOWNS[n]` reverses
/// `GENERATIONS[n]`. Used only by the round-trip test; the store itself
/// only ever migrates forward.
#[cfg(test)]
const DOWNS: &[DownStep] = &[down1, down2];

pub(crate) const GENERATIONS: &[Generation] = &[
    Generation {
        version: 1,
        up: up1,
    },
    Generation {
        version: 2,
        up: up2,
    },
];

fn up1(conn: &Connection) -> Result<(), SessionError> {
    conn.execute_batch(V1_SCHEMA).map_err(SessionError::sqlite)
}

#[cfg(test)]
fn down1(conn: &Connection) -> Result<(), SessionError> {
    conn.execute_batch(
        "DROP TABLE IF EXISTS turn_index;
         DROP TABLE IF EXISTS events;
         DROP TABLE IF EXISTS sessions;",
    )
    .map_err(SessionError::sqlite)
}

fn up2(conn: &Connection) -> Result<(), SessionError> {
    conn.execute_batch(V2_SCHEMA).map_err(SessionError::sqlite)
}

#[cfg(test)]
fn down2(conn: &Connection) -> Result<(), SessionError> {
    conn.execute_batch(
        "DROP TABLE IF EXISTS memory_fts;
         DROP TABLE IF EXISTS event_fts;
         DROP TABLE IF EXISTS memory_notes;
         DROP TABLE IF EXISTS leases;
         DROP TABLE IF EXISTS store_state;
         ALTER TABLE sessions DROP COLUMN imported_from;
         ALTER TABLE sessions DROP COLUMN imported_unknown;",
    )
    .map_err(SessionError::sqlite)
}

/// Applies forward migrations up to [`SCHEMA_VERSION`] inside one
/// transaction (atomic: all steps commit together or none do).
pub(crate) fn migrate(conn: &mut Connection) -> Result<(), SessionError> {
    let tx = conn.transaction().map_err(SessionError::sqlite)?;
    let version = user_version(&tx)?;
    if version > SCHEMA_VERSION {
        return Err(SessionError::Schema {
            version,
            supported: SCHEMA_VERSION,
        });
    }
    for generation in GENERATIONS {
        if generation.version > version {
            (generation.up)(&tx)?;
            tx.pragma_update(None, "user_version", generation.version)
                .map_err(SessionError::sqlite)?;
        }
    }
    tx.commit().map_err(SessionError::sqlite)
}

pub(crate) fn user_version(conn: &Connection) -> Result<u32, SessionError> {
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(SessionError::sqlite)?;
    u32::try_from(version).map_err(|_| SessionError::Corrupt {
        detail: format!("negative user_version {version}"),
    })
}

/// Canonical schema fingerprint: sorted `sqlite_master` entries plus the
/// version stamp. Two databases with equal fingerprints have equal
/// logical schemas.
#[cfg(test)]
pub(crate) fn fingerprint(conn: &Connection) -> Result<u64, SessionError> {
    let mut stmt = conn
        .prepare(
            "SELECT type, name, COALESCE(sql, '') FROM sqlite_master
             WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name",
        )
        .map_err(SessionError::sqlite)?;
    let mut rows = stmt.query([]).map_err(SessionError::sqlite)?;
    let mut hasher = blake3::Hasher::new();
    while let Some(row) = rows.next().map_err(SessionError::sqlite)? {
        let kind: String = row.get(0).map_err(SessionError::sqlite)?;
        let name: String = row.get(1).map_err(SessionError::sqlite)?;
        let sql: String = row.get(2).map_err(SessionError::sqlite)?;
        hasher.update(kind.as_bytes());
        hasher.update(b"|");
        hasher.update(name.as_bytes());
        hasher.update(b"|");
        hasher.update(sql.as_bytes());
        hasher.update(b"\n");
    }
    hasher.update(&user_version(conn)?.to_le_bytes());
    let digest = hasher.finalize();
    let mut tail = [0u8; 8];
    tail.copy_from_slice(&digest.as_bytes()[..8]);
    Ok(u64::from_le_bytes(tail))
}

/// Runs every generation forward, then all of them back down to zero,
/// then forward again — proving the registry has no one-way generations.
/// Returns the fingerprints at "latest" before and after the round trip
/// (the test asserts equality).
#[cfg(test)]
pub(crate) fn round_trip(conn: &mut Connection) -> Result<(u64, u64), SessionError> {
    assert_eq!(
        GENERATIONS.len(),
        DOWNS.len(),
        "every generation must carry a down-step"
    );
    migrate(conn)?;
    let before = fingerprint(conn)?;
    for (generation, down) in GENERATIONS.iter().rev().zip(DOWNS.iter().rev()) {
        let tx = conn.transaction().map_err(SessionError::sqlite)?;
        down(&tx)?;
        let target = generation.version - 1;
        tx.pragma_update(None, "user_version", target)
            .map_err(SessionError::sqlite)?;
        tx.commit().map_err(SessionError::sqlite)?;
    }
    migrate(conn)?;
    let after = fingerprint(conn)?;
    Ok((before, after))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_generation_round_trips_both_directions() {
        let mut conn = Connection::open_in_memory().unwrap();
        let (before, after) = round_trip(&mut conn).unwrap();
        assert_eq!(
            before, after,
            "down+up must land on the identical schema fingerprint"
        );
        assert_eq!(user_version(&conn).unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn future_version_is_a_typed_schema_error() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        let err = migrate(&mut conn).unwrap_err();
        assert!(matches!(
            err,
            SessionError::Schema { version, supported } if version == SCHEMA_VERSION + 1 && supported == SCHEMA_VERSION
        ));
    }
}
