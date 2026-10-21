//! Session metadata rows and the cross-session FTS search surface.

use rusqlite::{Connection, OptionalExtension};

use harness_core::event::{ContentBlock, Event};

use crate::error::SessionError;

/// Session metadata — one `sessions` row. Never holds payloads: this is
/// the whole cost of opening a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionMeta {
    /// Session id.
    pub id: String,
    /// Human title.
    pub title: String,
    /// Creation time (clock ticks).
    pub created_at: u64,
    /// Last write time.
    pub updated_at: u64,
    /// Last event sequence (0 = empty).
    pub last_seq: u64,
    /// Number of stored events.
    pub event_count: u64,
    /// Number of turns (user-anchored).
    pub turn_count: u32,
    /// Total stored payload bytes.
    pub payload_bytes: u64,
    /// Whether payloads are sealed under a per-session key.
    pub encrypted: bool,
    /// Session this one was forked from.
    pub forked_from: Option<String>,
    /// Exempt from retention pruning.
    pub pinned: bool,
    /// When the session was archived (cold), if ever.
    pub archived_at: Option<u64>,
    /// Foreign source this session was imported from.
    pub imported_from: Option<String>,
    /// Foreign entries that could not be mapped.
    pub imported_unknown: u64,
}

/// One cross-session full-text hit (BM25 via FTS5).
#[derive(Debug, Clone, PartialEq)]
pub struct EventSearchHit {
    /// Session containing the match.
    pub session_id: String,
    /// Event sequence of the match.
    pub seq: u64,
    /// Match snippet with the query terms.
    pub snippet: String,
    /// Raw FTS5 `bm25` score (lower = better).
    pub bm25: f64,
}

pub(crate) const META_COLUMNS: &str = "id, title, created_at, updated_at, last_seq, event_count, \
     turn_count, payload_bytes, encrypted, forked_from, pinned, archived_at, \
     imported_from, imported_unknown";

pub(crate) fn row_to_meta(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionMeta> {
    Ok(SessionMeta {
        id: row.get(0)?,
        title: row.get(1)?,
        created_at: row.get::<_, i64>(2)?.max(0) as u64,
        updated_at: row.get::<_, i64>(3)?.max(0) as u64,
        last_seq: row.get::<_, i64>(4)?.max(0) as u64,
        event_count: row.get::<_, i64>(5)?.max(0) as u64,
        turn_count: u32::try_from(row.get::<_, i64>(6)?).unwrap_or(u32::MAX),
        payload_bytes: row.get::<_, i64>(7)?.max(0) as u64,
        encrypted: row.get::<_, i64>(8)? != 0,
        forked_from: row.get(9)?,
        pinned: row.get::<_, i64>(10)? != 0,
        archived_at: row.get::<_, Option<i64>>(11)?.map(|v| v.max(0) as u64),
        imported_from: row.get(12)?,
        imported_unknown: row.get::<_, Option<i64>>(13)?.unwrap_or(0).max(0) as u64,
    })
}

pub(crate) fn load_meta_conn(conn: &Connection, id: &str) -> Result<SessionMeta, SessionError> {
    conn.query_row(
        &format!("SELECT {META_COLUMNS} FROM sessions WHERE id = ?1"),
        [id],
        row_to_meta,
    )
    .optional()
    .map_err(SessionError::sqlite)?
    .ok_or_else(|| SessionError::NotFound { id: id.to_owned() })
}

fn hit_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventSearchHit> {
    Ok(EventSearchHit {
        session_id: row.get(0)?,
        seq: row.get::<_, i64>(1)?.max(0) as u64,
        snippet: row.get(2)?,
        bm25: row.get(3)?,
    })
}

/// FTS5 query-parse failures: parser messages carry the `fts5` prefix,
/// but tokenizer-level messages ("unterminated string") do not — those
/// surface as SQLITE_ERROR (mapped to [`rusqlite::ErrorCode::Unknown`]),
/// which on a MATCH-only statement can only be a query error.
fn is_query_error(error: &rusqlite::Error) -> bool {
    match error {
        rusqlite::Error::SqliteFailure(ffi, _) => {
            ffi.code == rusqlite::ErrorCode::Unknown
        }
        _ => error.to_string().contains("fts5"),
    }
}

fn fts_error_mapper(query: &str) -> impl Fn(rusqlite::Error) -> SessionError + '_ {
    move |error| {
        if is_query_error(&error) {
            let detail = error.to_string();
            SessionError::BadQuery {
                query: query.to_owned(),
                detail,
            }
        } else {
            SessionError::sqlite(error)
        }
    }
}

pub(crate) fn search_events_conn(
    conn: &Connection,
    query: &str,
    limit: u32,
) -> Result<Vec<EventSearchHit>, SessionError> {
    let map_error = fts_error_mapper(query);
    let mut stmt = conn
        .prepare(
            "SELECT session_id, seq, snippet(event_fts, 2, '', '', ' … ', 12), bm25(event_fts)
             FROM event_fts WHERE event_fts MATCH ?1 ORDER BY rank LIMIT ?2",
        )
        .map_err(&map_error)?;
    let rows = stmt
        .query_map(rusqlite::params![query, limit], hit_row)
        .map_err(&map_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(&map_error)?;
    Ok(rows)
}

/// Session-scoped twin of [`search_events_conn`]: the same
/// MATCH/bm25/snippet/rank idiom, restricted to the listed session
/// ids. An empty scope matches nothing — callers that want every
/// session use [`search_events_conn`].
pub(crate) fn search_events_in_conn(
    conn: &Connection,
    query: &str,
    session_ids: &[&str],
    limit: u32,
) -> Result<Vec<EventSearchHit>, SessionError> {
    if session_ids.is_empty() {
        return Ok(Vec::new());
    }
    let map_error = fts_error_mapper(query);
    let id_placeholders = (2..=session_ids.len() + 1)
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT session_id, seq, snippet(event_fts, 2, '', '', ' … ', 12), bm25(event_fts)
         FROM event_fts WHERE event_fts MATCH ?1 AND session_id IN ({id_placeholders})
         ORDER BY rank LIMIT ?{}",
        session_ids.len() + 2
    );
    let mut stmt = conn.prepare(&sql).map_err(&map_error)?;
    let mut params: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(session_ids.len() + 2);
    params.push(&query);
    for id in session_ids {
        params.push(id);
    }
    params.push(&limit);
    let rows = stmt
        .query_map(params.as_slice(), hit_row)
        .map_err(&map_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(&map_error)?;
    Ok(rows)
}

/// LLM-visible text extracted for the cross-session index.
pub(crate) fn extract_text(event: &Event) -> Option<String> {
    let blocks = match event {
        Event::User { blocks } | Event::Assistant { blocks } | Event::ToolResult { blocks, .. } => {
            blocks
        }
        Event::System { text } => return Some(text.clone()),
        Event::ToolCall { .. } | Event::Condensation { .. } | Event::Snapshot { .. } => {
            return None;
        }
    };
    let text: Vec<&str> = blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            ContentBlock::Image { .. } | ContentBlock::Media(_) => None,
        })
        .collect();
    if text.is_empty() {
        None
    } else {
        Some(text.join("\n"))
    }
}
