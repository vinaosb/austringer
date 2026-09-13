//! Web SPA static serving: bytes from `harness-web` assets with
//! content-type and cache headers. HTML is `no-cache` (entry documents
//! must revalidate); hashed build artifacts get `immutable` (todo 37).

use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

/// Cache policy per asset class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachePolicy {
    /// Entry documents: revalidate every time.
    NoCache,
    /// Content-hashed artifacts: cache forever.
    Immutable,
}

/// Correct content-type for common SPA file extensions.
#[must_use]
pub fn mime_for(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if lower.ends_with(".js") || lower.ends_with(".mjs") {
        "text/javascript; charset=utf-8"
    } else if lower.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if lower.ends_with(".json") {
        "application/json"
    } else if lower.ends_with(".svg") {
        "image/svg+xml"
    } else if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".wasm") {
        "application/wasm"
    } else if lower.ends_with(".ico") {
        "image/x-icon"
    } else {
        "application/octet-stream"
    }
}

/// Cache policy for a path: entry documents revalidate; extension-less
/// hashed artifacts and known static files are immutable.
#[must_use]
pub fn cache_for(path: &str) -> CachePolicy {
    if path.ends_with(".html") || path == "/" || path.ends_with(".json") {
        CachePolicy::NoCache
    } else {
        CachePolicy::Immutable
    }
}

fn cache_header(policy: CachePolicy) -> &'static str {
    match policy {
        CachePolicy::NoCache => "no-cache",
        CachePolicy::Immutable => "public, max-age=31536000, immutable",
    }
}

/// Serve the SPA shell (any session deep-link route renders it).
#[must_use]
pub fn spa_shell() -> Response {
    let (_, _, body) = harness_web::assets()[0];
    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime_for("/index.html")),
        )
        .header(
            header::CACHE_CONTROL,
            HeaderValue::from_static(cache_header(cache_for("/"))),
        )
        .body(axum::body::Body::from(body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// Serve one registered asset by path.
#[must_use]
pub fn serve(path: &str) -> Response {
    let found = harness_web::assets()
        .iter()
        .find(|(route, _, _)| *route == path)
        .copied();
    match found {
        Some((_, mime, body)) => {
            let mime = if path.contains('.') {
                mime_for(path)
            } else {
                mime
            };
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, HeaderValue::from_static(mime))
                .header(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static(cache_header(cache_for(path))),
                )
                .body(axum::body::Body::from(body))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_table_covers_spa_types() {
        assert_eq!(mime_for("index.html"), "text/html; charset=utf-8");
        assert_eq!(mime_for("app.js"), "text/javascript; charset=utf-8");
        assert_eq!(mime_for("app.css"), "text/css; charset=utf-8");
        assert_eq!(mime_for("icon.svg"), "image/svg+xml");
        assert_eq!(mime_for("engine.wasm"), "application/wasm");
        assert_eq!(mime_for("blob.bin"), "application/octet-stream");
    }

    #[test]
    fn cache_policy_splits_entry_from_artifacts() {
        assert_eq!(cache_for("/"), CachePolicy::NoCache);
        assert_eq!(cache_for("/index.html"), CachePolicy::NoCache);
        assert_eq!(cache_for("/manifest.json"), CachePolicy::NoCache);
        assert_eq!(cache_for("/app-a1b2c3.js"), CachePolicy::Immutable);
    }

    #[test]
    fn root_serves_the_web_index_with_headers() {
        let response = serve("/");
        assert_eq!(response.status(), StatusCode::OK);
        let headers = response.headers();
        assert_eq!(
            headers
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("text/html; charset=utf-8")
        );
        assert_eq!(
            headers
                .get(header::CACHE_CONTROL)
                .and_then(|v| v.to_str().ok()),
            Some("no-cache")
        );
    }

    #[test]
    fn unknown_path_is_404() {
        assert_eq!(serve("/nope.xyz").status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn spa_shell_is_the_deep_link_target() {
        let response = spa_shell();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().contains_key(header::CONTENT_TYPE));
    }
}
