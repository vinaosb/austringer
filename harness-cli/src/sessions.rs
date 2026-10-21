//! The `sessions search` flow (todo 11, ecosystem alignment): one GET
//! against the daemon's `GET /sessions/search` route, rendered as
//! ranked human lines. The subcommand is HTTP-only — the IPC carrier
//! has no HTTP base to reach the route on (the daemon does not persist
//! its port), so a missing `--url` is a typed transport hint, never a
//! confusing token-file failure.

use crate::cli::SessionsSearchArgs;
use crate::error::CliError;
use crate::target::TargetSpec;

/// Runs `sessions search`: one GET against the daemon's search route,
/// rendered as ranked human lines. Exits 0 on 200 (empty or not),
/// typed failure otherwise.
///
/// # Errors
/// Typed [`CliError`]: unreachable (no daemon / transport hint), auth
/// refused, protocol (bad status or malformed body).
pub async fn search(spec: &TargetSpec, args: &SessionsSearchArgs) -> Result<i32, CliError> {
    let Some(base) = spec
        .url
        .as_deref()
        .map(|url| url.trim_end_matches('/'))
    else {
        return Err(CliError::Unreachable {
            detail: "sessions search needs the HTTP transport; pass \
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
    let query = args.query.join(" ");
    let mut request = reqwest::Client::new()
        .get(format!("{base}/sessions/search"))
        .query(&[("q", query.as_str())]);
    if let Some(limit) = args.limit {
        request = request.query(&[("limit", limit.to_string())]);
    }
    if let Some(sessions) = &args.sessions {
        request = request.query(&[("sessions", sessions.as_str())]);
    }
    if let Some(token) = token.as_deref() {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.map_err(|error| CliError::Unreachable {
        detail: error.to_string(),
    })?;
    match response.status().as_u16() {
        200 => {
            let body = response.text().await.map_err(|error| CliError::Protocol {
                detail: error.to_string(),
            })?;
            render(&body, &query)
        }
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

/// Render the daemon's hit array (best-first) as one line per hit.
fn render(body: &str, query: &str) -> Result<i32, CliError> {
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
}
