//! The `sessions` verb family (todo 11 search; todo 22 list/history),
//! all riding the daemon's HTTP routes and rendered as ranked human
//! lines. The subcommands are HTTP-only — the IPC carrier has no HTTP
//! base to reach the routes on (the daemon does not persist its port),
//! so a missing `--url` is a typed transport hint, never a confusing
//! token-file failure.

use crate::cli::SessionsSearchArgs;
use crate::error::CliError;
use crate::target::TargetSpec;

/// The resolved HTTP surface for a sessions verb: daemon base plus the
/// bearer token when one is known.
struct HttpSurface<'a> {
    base: &'a str,
    token: Option<String>,
}

/// Resolve the HTTP surface every sessions verb needs, or fail with
/// the typed transport hint.
fn http_surface(spec: &TargetSpec) -> Result<HttpSurface<'_>, CliError> {
    let Some(base) = spec
        .url
        .as_deref()
        .map(|url| url.trim_end_matches('/'))
    else {
        return Err(CliError::Unreachable {
            detail: "sessions commands need the HTTP transport; pass \
                     --url http://127.0.0.1:PORT (the daemon prints its URL at startup)"
                .into(),
        });
    };
    let token = match (&spec.token, &spec.data_dir) {
        (Some(token), _) => Some(token.clone()),
        (None, Some(dir)) => crate::target::read_token(dir),
        (None, None) => crate::target::read_token(std::path::Path::new(
            crate::target::DEFAULT_DATA_DIR,
        )),
    };
    Ok(HttpSurface { base, token })
}

/// Send one authenticated request: 200 returns the body text, 401/403
/// maps to auth, anything else to the daemon's typed error detail.
async fn send_typed(request: reqwest::RequestBuilder) -> Result<String, CliError> {
    let response = request.send().await.map_err(|error| CliError::Unreachable {
        detail: error.to_string(),
    })?;
    match response.status().as_u16() {
        200 => response.text().await.map_err(|error| CliError::Protocol {
            detail: error.to_string(),
        }),
        401 | 403 => Err(CliError::Auth {
            detail: "daemon refused the token".into(),
        }),
        status => {
            let body = response.text().await.unwrap_or_default();
            Err(CliError::Protocol {
                detail: format!("status {status}: {}", error_detail(&body)),
            })
        }
    }
}

/// Runs `sessions search`: one GET against the daemon's search route,
/// rendered as ranked human lines. Exits 0 on 200 (empty or not),
/// typed failure otherwise.
///
/// # Errors
/// Typed [`CliError`]: unreachable (no daemon / transport hint), auth
/// refused, protocol (bad status or malformed body).
pub async fn search(spec: &TargetSpec, args: &SessionsSearchArgs) -> Result<i32, CliError> {
    let surface = http_surface(spec)?;
    let query = args.query.join(" ");
    let mut request = reqwest::Client::new()
        .get(format!("{}/sessions/search", surface.base))
        .query(&[("q", query.as_str())]);
    if let Some(limit) = args.limit {
        request = request.query(&[("limit", limit.to_string())]);
    }
    if let Some(sessions) = &args.sessions {
        request = request.query(&[("sessions", sessions.as_str())]);
    }
    if let Some(token) = surface.token.as_deref() {
        request = request.bearer_auth(token);
    }
    let body = send_typed(request).await?;
    render_search(&body, &query)
}

/// Runs `sessions list`: every live session id in the daemon, one per
/// line.
///
/// # Errors
/// Typed [`CliError`] like [`search`].
pub async fn list(spec: &TargetSpec) -> Result<i32, CliError> {
    let surface = http_surface(spec)?;
    let mut request = reqwest::Client::new().get(format!("{}/sessions", surface.base));
    if let Some(token) = surface.token.as_deref() {
        request = request.bearer_auth(token);
    }
    let body = send_typed(request).await?;
    render_list(&body)
}

/// Runs `sessions history`: one session's scheduled-run lifecycle rows
/// (started + completion rows with their gate verdicts), one per line.
///
/// # Errors
/// Typed [`CliError`] like [`search`]; an unknown session is the
/// daemon's typed 404.
pub async fn history(spec: &TargetSpec, session: &str) -> Result<i32, CliError> {
    let surface = http_surface(spec)?;
    let path = format!(
        "{}/s/{}/history",
        surface.base,
        crate::client::percent_encode(session)
    );
    let mut request = reqwest::Client::new().get(path);
    if let Some(token) = surface.token.as_deref() {
        request = request.bearer_auth(token);
    }
    let body = send_typed(request).await?;
    render_history(&body, session)
}

/// Render the daemon's hit array (best-first) as one line per hit.
fn render_search(body: &str, query: &str) -> Result<i32, CliError> {
    let hits: Vec<serde_json::Value> =
        serde_json::from_str(body).map_err(|error| CliError::Protocol {
            detail: format!("malformed hit array: {error}"),
        })?;
    if hits.is_empty() {
        println!("no hits for {query:?}");
        return Ok(0);
    }
    for (index, hit) in hits.iter().enumerate() {
        println!(
            "[{}] session={} seq={} bm25={} :: {}",
            index + 1,
            hit["session_id"].as_str().unwrap_or("?"),
            hit["seq"].as_u64().unwrap_or(0),
            hit["bm25"]
                .as_f64()
                .map(|score| score.to_string())
                .unwrap_or_else(|| "?".into()),
            hit["snippet"].as_str().unwrap_or(""),
        );
    }
    Ok(0)
}

/// Render the list route's `{"sessions": [...]}` body as one line per
/// session id.
fn render_list(body: &str) -> Result<i32, CliError> {
    let list = serde_json::from_str::<serde_json::Value>(body).map_err(|error| {
        CliError::Protocol {
            detail: format!("malformed session list: {error}"),
        }
    })?;
    let ids = list
        .get("sessions")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| CliError::Protocol {
            detail: "malformed session list: no sessions array".into(),
        })?
        .iter()
        .filter_map(|value| value.as_str())
        .collect::<Vec<_>>();
    if ids.is_empty() {
        println!("no live sessions");
        return Ok(0);
    }
    for (index, id) in ids.iter().enumerate() {
        println!("[{}] {}", index + 1, id);
    }
    Ok(0)
}

/// Render the history route's bare run-record array as one line per
/// row, verdict included when the row completed.
fn render_history(body: &str, session: &str) -> Result<i32, CliError> {
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(body).map_err(|error| CliError::Protocol {
            detail: format!("malformed run history: {error}"),
        })?;
    if rows.is_empty() {
        println!("no runs recorded for session {session}");
        return Ok(0);
    }
    for (index, row) in rows.iter().enumerate() {
        let verdict = row["verdict"]
            .as_str()
            .map_or_else(String::new, |verdict| format!(" verdict={verdict}"));
        println!(
            "[{}] automation={} fired@{}m outcome={}{verdict}",
            index + 1,
            row["automation_id"].as_str().unwrap_or("?"),
            row["fired_at_minute"].as_u64().unwrap_or(0),
            row["outcome"].as_str().unwrap_or("?"),
        );
    }
    Ok(0)
}

/// The daemon's typed error body (`{"error":{"code","detail"}}`), with
/// underscores spaced for human stderr.
fn error_detail(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|json| {
            let error = &json["error"];
            let code = error["code"].as_str()?.replace('_', " ");
            let detail = error["detail"].as_str().unwrap_or_default();
            Some(format!("{code}: {detail}"))
        })
        .unwrap_or_else(|| body.chars().take(200).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_detail_spaces_the_typed_code() {
        let body = r#"{"error":{"code":"bad_query","detail":"unterminated string"}}"#;
        assert_eq!(error_detail(body), "bad query: unterminated string");
    }

    #[test]
    fn error_detail_falls_back_to_bounded_raw_body() {
        let long = "x".repeat(400);
        assert_eq!(error_detail(&long), "x".repeat(200));
    }

    #[test]
    fn list_render_prints_one_line_per_session() {
        let body = r#"{"sessions":["alpha","beta"]}"#;
        assert_eq!(render_list(body).unwrap_or(-1), 0);
    }

    #[test]
    fn list_render_rejects_a_body_without_the_sessions_array() {
        assert!(render_list(r#"{"nope":1}"#).is_err());
    }

    #[test]
    fn history_render_prints_rows_with_verdicts() {
        let body = r#"[{"automation_id":"nightly","session_id":"alpha","fired_at_minute":1000,"outcome":"started","verdict":null},
                       {"automation_id":"nightly","session_id":"alpha","fired_at_minute":1000,"outcome":"task_completed","verdict":"confirmed"}]"#;
        assert_eq!(render_history(body, "alpha").unwrap_or(-1), 0);
    }
}
