//! Hermetic contract tests for the SPA asset registry (todo 37): the
//! SERVED CONTENT — structure, wiring, and the no-cloud guard — proven
//! without a browser. Behavioral e2e lives in `e2e/run-e2e.mjs`
//! (playwright-core) against the scripted daemon in `examples/`.

use std::collections::BTreeSet;

use austringer_web::assets;
use base64ct::Encoding;

fn index() -> &'static str {
    let (_, _, body) = assets()[0];
    body
}

fn routes() -> BTreeSet<&'static str> {
    assets().iter().map(|(route, _, _)| *route).collect()
}

/// Extract every `/assets/...` reference from the built index
/// (attribute values in the entry document + import-map values).
fn referenced_routes() -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut rest = index();
    while let Some(pos) = rest.find("/assets/") {
        let tail = &rest[pos..];
        let end = tail
            .find(|c: char| c == '"' || c == '\'' || c.is_whitespace() || c == '>')
            .unwrap_or(tail.len());
        found.insert(tail[..end].to_owned());
        rest = &rest[pos + end..];
    }
    found
}

fn import_map_json() -> String {
    let marker = "<script type=\"importmap\">";
    let start = index().find(marker).map_or(0, |p| p + marker.len());
    let end = index()[start..].find("</script>").map_or(0, |p| start + p);
    index()[start..end].to_owned()
}

#[test]
fn entry_zero_is_the_index_document_at_root() {
    let (route, mime, body) = assets()[0];
    assert_eq!(route, "/");
    assert_eq!(mime, "text/html; charset=utf-8");
    assert!(body.contains("<!doctype html>"));
}

#[test]
fn every_artifact_route_is_content_addressed() {
    for (route, _, body) in assets().iter().skip(1) {
        let file = route.rsplit('/').next().unwrap_or(route);
        let (stem, ext) = file.rsplit_once('.').unwrap_or((file, ""));
        let Some((name, digest)) = stem.rsplit_once('-') else {
            panic!("route {route} lacks a content-hash segment");
        };
        assert_eq!(
            digest.len(),
            12,
            "route {route}: blake3[0..12] hash segment expected"
        );
        let expected = &blake3_hash12(body);
        assert_eq!(
            digest, expected,
            "route {route} hash does not match its bytes"
        );
        assert!(
            !name.is_empty() && !ext.is_empty(),
            "route {route} malformed"
        );
    }
}

fn blake3_hash12(body: &str) -> String {
    blake3::hash(body.as_bytes()).to_hex().to_string()[..12].to_owned()
}

#[test]
fn index_references_only_registered_routes() {
    let registered = routes();
    let referenced = referenced_routes();
    for route in &referenced {
        assert!(
            registered.contains(route.as_str()),
            "index references unregistered route {route}"
        );
    }
}

#[test]
fn import_map_targets_decode_to_the_exact_module_sources() {
    let map: serde_json::Value = serde_json::from_str(&import_map_json())
        .unwrap_or_else(|e| panic!("import map must be valid JSON: {e}"));
    let imports = map
        .get("imports")
        .and_then(|v| v.as_object())
        .unwrap_or_else(|| panic!("import map lacks an imports object"));
    // Count derives from the mirror, not a pinned literal: a registry
    // module added without a mirror entry fails here or in the
    // per-module equality below.
    assert_eq!(imports.len(), JS_SOURCES.len());
    let by_bare_name = |bare: &str| {
        JS_SOURCES
            .iter()
            .find(|(name, _)| *name == bare)
            .map(|(_, body)| *body)
            .unwrap_or_default()
    };
    for (name, target) in imports {
        let target = target.as_str().unwrap_or_default();
        let payload = target
            .strip_prefix("data:text/javascript;base64,")
            .unwrap_or_default();
        assert!(!payload.is_empty(), "module {name} is not a data: URL");
        let decoded = base64ct::Base64::decode_vec(payload)
            .unwrap_or_else(|e| panic!("module {name} base64: {e}"));
        let source = String::from_utf8_lossy(&decoded).to_string();
        let bare = name.strip_prefix("spa/").unwrap_or(name);
        assert_eq!(
            source,
            by_bare_name(bare),
            "module {name} data: payload must be its exact source"
        );
    }
}

/// The served module sources as (bare name, source) — mirrors the
/// registry's JS_MODULES table from the austringer-web crate.
const JS_SOURCES: &[(&str, &str)] = &[
    ("proto", include_str!("../assets/proto.js")),
    ("grammar", include_str!("../assets/grammar.js")),
    ("store", include_str!("../assets/store.js")),
    ("share", include_str!("../assets/share.js")),
    ("ui", include_str!("../assets/ui.js")),
];

#[test]
fn entry_document_is_self_contained() {
    // No artifact fetches: the daemon router ships no /assets route
    // (disclosed); the boot path must not request one.
    let body = index();
    assert_eq!(
        referenced_routes().len(),
        0,
        "entry must not reference artifact routes"
    );
    assert!(body.contains("<style>"), "stylesheet is inlined");
    assert!(
        body.contains("data:text/javascript;base64,"),
        "modules ride data: URLs"
    );
    assert!(
        body.contains("data:image/svg+xml;base64,"),
        "favicon is a data: URL"
    );
    assert!(
        body.contains("script-src 'self' 'unsafe-inline' data:"),
        "CSP admits data: modules"
    );
}

#[test]
fn no_cloud_dependencies_anywhere() {
    // The W3C SVG namespace is an XML identifier, never fetched; it is
    // the single allowed non-loopback literal. Schemes immediately
    // followed by `${` are same-origin template URLs resolved from
    // location.host at runtime (WebSocket has no relative form).
    const NAMESPACE_IDS: &[&str] = &["www.w3.org/2000/svg"];
    for &(route, _, body) in assets() {
        assert!(
            !body.contains("https://"),
            "{route} must not reference https:// (no cloud services)"
        );
        for scheme in ["http://", "ws://"] {
            let mut rest = body;
            while let Some(pos) = rest.find(scheme) {
                let after_scheme = &rest[pos + scheme.len()..];
                let dynamic_origin = after_scheme.starts_with("${");
                let loopback =
                    after_scheme.starts_with("127.0.0.1") || after_scheme.starts_with("localhost");
                let namespace = NAMESPACE_IDS.iter().any(|id| after_scheme.starts_with(id));
                assert!(
                    loopback || namespace || dynamic_origin,
                    "{route}: non-loopback {scheme} reference at byte {pos}"
                );
                rest = after_scheme;
            }
        }
    }
}

#[test]
fn entry_document_has_hardening_and_mount_point() {
    let body = index();
    assert!(
        body.contains("Content-Security-Policy"),
        "CSP meta required (cloud-dep guard at the browser level too)"
    );
    assert!(body.contains("default-src 'self'"));
    assert!(body.contains("id=\"app\""), "mount point required");
    assert!(body.contains("charset=\"utf-8\""));
    assert!(body.contains("type=\"module\""), "entry module script");
    assert!(body.contains("<noscript>"));
}

#[test]
fn js_modules_are_substantive() {
    for (route, mime, body) in assets().iter() {
        if *mime == "text/javascript; charset=utf-8" {
            assert!(
                body.len() > 500,
                "{route} is too small to be a real module ({})",
                body.len()
            );
        }
    }
}

#[tokio::test]
async fn daemon_serves_registry_content_with_the_contract_headers() {
    // SERVED CONTENT: through the daemon's own serving module (the exact
    // path production takes), not just the registry array.
    let shell = austringer_daemon::web::spa_shell();
    assert_eq!(shell.status(), axum::http::StatusCode::OK);
    let shell_bytes = axum::body::to_bytes(shell.into_body(), 1 << 20)
        .await
        .unwrap_or_default();
    assert_eq!(shell_bytes.as_ref(), index().as_bytes());

    let js_route = assets()
        .iter()
        .find(|(route, _, _)| route.ends_with(".js"))
        .map(|(route, _, _)| *route)
        .unwrap_or_default();
    let served = austringer_daemon::web::serve(js_route);
    assert_eq!(served.status(), axum::http::StatusCode::OK);
    assert_eq!(
        served
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("text/javascript; charset=utf-8")
    );
    assert_eq!(
        served
            .headers()
            .get(axum::http::header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok()),
        Some("public, max-age=31536000, immutable")
    );
}
