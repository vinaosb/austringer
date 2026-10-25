//! The event_fts SELECT surface (todo 11, ecosystem alignment): a
//! read-only reader over the session store's already-maintained FTS5
//! index. Two consumers share this module:
//!
//! * `GET /sessions/search` — the human/frontend surface: cross-session
//!   over the daemon's ONE store root, optionally scoped via
//!   `&sessions=`; an empty result is 200 `[]`, never 404; an
//!   unparseable FTS5 query is a typed 400, never 500.
//! * the model-facing `search_sessions` tool, registered through the
//!   task-3 per-session ToolRegistry — session-scoped BY DEFAULT;
//!   cross-session search requires explicit session ids. Both surfaces
//!   open exactly one `SessionStore` at the daemon data dir, so no
//!   search path can cross working-dir roots (hits flow into model
//!   context = provider egress; the boundary is structural: there is
//!   no path argument anywhere).
//!
//! Receipt note: Zed #58686 is a roadmap ticket, not shipped-proven —
//! justified under the filter's lightweight clause (a reader over an
//! already-maintained index).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;

use austringer_core::tools::{ProviderError, ToolSchema};
use austringer_sessions::SessionError;
use austringer_sessions::SessionStore;

use crate::server::DaemonState;
use crate::tools::providers::{optional_u64_arg, str_arg};

/// Route default and ceiling for `limit`.
const DEFAULT_ROUTE_LIMIT: u32 = 20;
const MAX_ROUTE_LIMIT: u32 = 100;
/// Tool default and hard ceiling for `limit`: model-facing results stay
/// small even when the model asks for a thousand.
const DEFAULT_TOOL_LIMIT: u32 = 8;
const MAX_TOOL_LIMIT: u32 = 25;

fn typed_error(status: StatusCode, code: &str, extra: Value) -> Response {
    let mut error = json!({ "code": code });
    if let Value::Object(fields) = extra {
        for (key, value) in fields {
            error[key] = value;
        }
    }
    (status, Json(json!({ "error": error }))).into_response()
}

fn hit_json(hit: &austringer_sessions::EventSearchHit) -> Value {
    json!({
        "session_id": hit.session_id,
        "seq": hit.seq,
        "snippet": hit.snippet,
        "bm25": hit.bm25,
    })
}

#[derive(Deserialize)]
pub(crate) struct SearchQuery {
    pub(crate) q: Option<String>,
    pub(crate) limit: Option<u32>,
    /// Comma-separated session ids; absent = the whole store root.
    pub(crate) sessions: Option<String>,
}

/// `GET /sessions/search?q=<fts5>&limit=N&sessions=a,b` — BM25-ranked
/// event hits (best first) over the daemon's session store. Empty
/// results are 200 `[]`; FTS5 syntax errors are a typed 400.
pub(crate) async fn sessions_search(
    State(state): State<Arc<DaemonState>>,
    Query(params): Query<SearchQuery>,
) -> Response {
    let Some(query) = params.q.filter(|q| !q.is_empty()) else {
        return typed_error(
            StatusCode::BAD_REQUEST,
            "missing_query",
            json!({ "detail": "pass ?q=<fts5 query>" }),
        );
    };
    let limit = params
        .limit
        .unwrap_or(DEFAULT_ROUTE_LIMIT)
        .min(MAX_ROUTE_LIMIT);
    let scope: Vec<String> = params
        .sessions
        .map(|list| {
            list.split(',')
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let store = match SessionStore::open(&state.config.data_dir) {
        Ok(store) => store,
        Err(error) => {
            return typed_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "store_unavailable",
                json!({ "detail": error.to_string() }),
            );
        }
    };
    let result = if scope.is_empty() {
        store.search_events(&query, limit)
    } else {
        let refs: Vec<&str> = scope.iter().map(String::as_str).collect();
        store.search_events_in_sessions(&query, &refs, limit)
    };
    match result {
        Ok(hits) => Json(hits.iter().map(hit_json).collect::<Vec<_>>()).into_response(),
        Err(SessionError::BadQuery { query, detail }) => typed_error(
            StatusCode::BAD_REQUEST,
            "bad_query",
            json!({ "query": query, "detail": detail }),
        ),
        Err(error) => typed_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "store_error",
            json!({ "detail": error.to_string() }),
        ),
    }
}

/// The model-facing `search_sessions` tool: FTS5 search over session
/// transcripts, ranked by bm25. Session-scoped by default — without
/// `session_ids` it searches the CALLING session only; cross-session
/// search needs explicit ids and stays inside this daemon's store root
/// (no path argument exists, so no search can cross roots).
pub(crate) struct SearchSessionsTool {
    store_root: PathBuf,
    session_id: String,
}

impl SearchSessionsTool {
    /// Derives the store root (the session dir's grandparent) and the
    /// calling session id from the engine session dir.
    pub(crate) fn new(session_dir: &Path) -> Self {
        Self {
            store_root: crate::rewind::sessions_root(session_dir),
            session_id: session_dir
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
        }
    }
}

#[async_trait]
impl austringer_core::tools::ToolProvider for SearchSessionsTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "search_sessions".into(),
            description: "Full-text search (FTS5) over session transcripts, ranked by bm25 (best first). Session-scoped by default: without session_ids it searches THIS session only; pass explicit session ids to search other sessions in this daemon's store (never beyond it). Results are bounded.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "minLength": 1, "description": "FTS5 query (e.g. a word, a quoted phrase, word1 OR word2)." },
                    "session_ids": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Sessions to search; omit for THIS session only."
                    },
                    "limit": { "type": "integer", "minimum": 1, "description": "Maximum hits (default 8, hard cap 25)." }
                },
                "required": ["query"]
            }),
        }
    }

    async fn execute(&self, arguments: &Value) -> Result<Vec<u8>, ProviderError> {
        let query = str_arg(arguments, "query")?;
        let limit = optional_u64_arg(arguments, "limit")?
            .unwrap_or(u64::from(DEFAULT_TOOL_LIMIT))
            .clamp(1, u64::from(MAX_TOOL_LIMIT)) as u32;
        let mut scope: Vec<String> = Vec::new();
        if let Some(ids) = arguments
            .get("session_ids")
            .filter(|value| !value.is_null())
        {
            let ids = ids
                .as_array()
                .ok_or_else(|| ProviderError::InvalidArguments {
                    message: "'session_ids' must be an array of session id strings".to_owned(),
                })?;
            for id in ids {
                scope.push(
                    id.as_str()
                        .ok_or_else(|| ProviderError::InvalidArguments {
                            message: "'session_ids' must be an array of session id strings"
                                .to_owned(),
                        })?
                        .to_owned(),
                );
            }
        } else {
            scope.push(self.session_id.clone());
        }
        let refs: Vec<&str> = scope.iter().map(String::as_str).collect();
        let store =
            SessionStore::open(&self.store_root).map_err(|error| ProviderError::Execution {
                message: error.to_string(),
            })?;
        let hits = store
            .search_events_in_sessions(query, &refs, limit)
            .map_err(|error| match error {
                SessionError::BadQuery { detail, .. } => ProviderError::InvalidArguments {
                    message: format!("invalid FTS5 query: {detail}"),
                },
                other => ProviderError::Execution {
                    message: other.to_string(),
                },
            })?;
        let mut out = String::new();
        out.push_str(&format!(
            "search_sessions {:?} (scope: {}): {} hit(s)\n",
            query,
            refs.join(", "),
            hits.len()
        ));
        for (index, hit) in hits.iter().enumerate() {
            out.push_str(&format!(
                "[{}] session={} seq={} bm25={:.2} :: {}\n",
                index + 1,
                hit.session_id,
                hit.seq,
                hit.bm25,
                hit.snippet
            ));
        }
        Ok(out.into_bytes())
    }
}
