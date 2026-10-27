//! Static web dashboard assets for the austringer; contains no server logic.
//!
//! The daemon (todo 35) serves these bytes over HTTP with content-type
//! and cache headers. Todo 37 fills the registry with the real SPA: a
//! zero-build, framework-free ES-module app (no bundler, no CDN, no
//! cloud services — enforced by the test suite).
//!
//! The entry document is SELF-CONTAINED: stylesheet, import map, and
//! entry module are inlined, and module sources ride `data:` URLs.
//! The daemon's router ships no `/assets/{path}` route (a disclosed
//! integration gap, see `.omo/evidence/task-37-rust-agent-harness.txt`),
//! so the boot path must not request artifacts. The registry still
//! publishes every artifact under a CONTENT-ADDRESSED route
//! (`/assets/<name>-<blake3[0..12]>.<ext>`) with `immutable` cache
//! semantics, ready for the daemon route that makes them reachable.

use std::sync::OnceLock;

use base64ct::{Base64, Encoding};

/// Entry document template. `{{FAVICON}}`, `{{IMPORT_MAP}}`,
/// `{{STYLE}}`, and `{{APP_JS}}` are replaced when the registry is
/// built.
const INDEX_TEMPLATE: &str = include_str!("../assets/index.html");

/// Stylesheet source.
const APP_CSS: &str = include_str!("../assets/app.css");

/// Entry module (bootstrap, router, composer); imports the modules
/// below through the generated import map.
const APP_JS: &str = include_str!("../assets/app.js");

/// Protocol codec + transports (WS primary, SSE fallback).
const PROTO_JS: &str = include_str!("../assets/proto.js");

/// Shared command grammar (todo 23; pinned to the daemon table).
const GRAMMAR_JS: &str = include_str!("../assets/grammar.js");

/// State store: total, non-throwing reducer over server messages.
const STORE_JS: &str = include_str!("../assets/store.js");

/// Share links: redaction, expiry, URL-safe codec (410 on expiry).
const SHARE_JS: &str = include_str!("../assets/share.js");

/// DOM rendering root (text-only injection; untrusted text never
/// reaches `innerHTML`): token gate, banner, view switch.
const UI_JS: &str = include_str!("../assets/ui.js");

/// DOM primitives shared by the panels (the ui split's base module).
const UI_DOM_JS: &str = include_str!("../assets/ui-dom.js");

/// Dashboard panels (sessions list, tool heatmap, packs).
const UI_DASHBOARD_JS: &str = include_str!("../assets/ui-dashboard.js");

/// History panels (trends, scheduled-run history, changed files).
const UI_HISTORY_JS: &str = include_str!("../assets/ui-history.js");

/// The team grid panel (cells, budget, Warp-precedence aggregate).
const UI_TEAM_JS: &str = include_str!("../assets/ui-team.js");

/// The session surface (HUD, gates, hunks, transcript, composer,
/// replay).
const UI_SESSION_JS: &str = include_str!("../assets/ui-session.js");

/// Share surfaces (mint box + standalone reader).
const UI_SHARE_JS: &str = include_str!("../assets/ui-share.js");

/// Mark inline icon.
const FAVICON_SVG: &str = include_str!("../assets/favicon.svg");

/// Import-map modules: (bare specifier, source). Bare names avoid
/// relative imports, which content-addressed routes would break.
const JS_MODULES: &[(&str, &str)] = &[
    ("spa/proto", PROTO_JS),
    ("spa/grammar", GRAMMAR_JS),
    ("spa/store", STORE_JS),
    ("spa/share", SHARE_JS),
    ("spa/ui", UI_JS),
    ("spa/ui-dom", UI_DOM_JS),
    ("spa/ui-dashboard", UI_DASHBOARD_JS),
    ("spa/ui-history", UI_HISTORY_JS),
    ("spa/ui-team", UI_TEAM_JS),
    ("spa/ui-session", UI_SESSION_JS),
    ("spa/ui-share", UI_SHARE_JS),
];

/// Content-addressed route for one artifact: `/assets/<name>-<blake3[0..12]>.<ext>`.
fn content_route(name: &str, ext: &str, body: &str) -> &'static str {
    let digest = blake3::hash(body.as_bytes()).to_hex().to_string();
    let route = format!("/assets/{name}-{}.{}", &digest[..12], ext);
    Box::leak(route.into_boxed_str())
}

/// `data:` URL carrying one module source (import-map target for the
/// self-contained entry document; requires CSP `script-src ... data:`).
fn data_url(mime: &str, body: &str) -> String {
    format!(
        "data:{mime};base64,{}",
        Base64::encode_string(body.as_bytes())
    )
}

/// Build the registry once: the self-contained entry document plus the
/// content-addressed artifacts. Leaked strings live for the process
/// lifetime — the registry is a static serving contract, never rebuilt.
fn build() -> Vec<(&'static str, &'static str, &'static str)> {
    let css_route = content_route("app", "css", APP_CSS);
    let app_route = content_route("app", "js", APP_JS);
    let favicon_route = content_route("favicon", "svg", FAVICON_SVG);
    let imports = JS_MODULES
        .iter()
        .map(|(name, body)| format!("\"{name}\": \"{}\"", data_url("text/javascript", body)))
        .collect::<Vec<_>>()
        .join(", ");
    let import_map = format!("{{\"imports\":{{{imports}}}}}");
    let index = INDEX_TEMPLATE
        .replace("{{IMPORT_MAP}}", &import_map)
        .replace("{{STYLE}}", APP_CSS)
        .replace("{{APP_JS}}", APP_JS)
        .replace("{{FAVICON}}", &data_url("image/svg+xml", FAVICON_SVG));

    let mut entries = vec![
        (
            "/" as &'static str,
            "text/html; charset=utf-8",
            Box::leak(index.into_boxed_str()) as &'static str,
        ),
        (css_route, "text/css; charset=utf-8", APP_CSS),
        (app_route, "text/javascript; charset=utf-8", APP_JS),
        (favicon_route, "image/svg+xml", FAVICON_SVG),
    ];
    for (name, body) in JS_MODULES {
        let base = name.rsplit('/').next().unwrap_or(name);
        entries.push((
            content_route(base, "js", body),
            "text/javascript; charset=utf-8",
            *body,
        ));
    }
    entries
}

/// One servable asset: (route path, content-type, bytes). Entry `[0]`
/// is always the entry document at route `/` (the daemon's SPA-shell
/// contract). Assets are content-addressed and immutable across a
/// process lifetime.
#[must_use]
pub fn assets() -> &'static [(&'static str, &'static str, &'static str)] {
    static REGISTRY: OnceLock<Vec<(&'static str, &'static str, &'static str)>> = OnceLock::new();
    REGISTRY.get_or_init(build)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_stable_within_a_process() {
        let a = assets();
        let b = assets();
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.0, y.0);
        }
    }

    #[test]
    fn routes_are_unique() {
        let seen: std::collections::HashSet<&str> =
            assets().iter().map(|(route, _, _)| *route).collect();
        assert_eq!(seen.len(), assets().len());
    }

    #[test]
    fn built_index_has_no_leftover_template_tokens() {
        let (_, _, index) = assets()[0];
        assert!(!index.contains("{{"));
    }

    #[test]
    fn inline_sources_never_close_their_host_tag() {
        // The entry document embeds CSS and JS verbatim; a literal
        // </style> or </script> inside a source would break out of the
        // inline context.
        for source in [
            APP_CSS, APP_JS, PROTO_JS, GRAMMAR_JS, STORE_JS, SHARE_JS, UI_JS,
        ] {
            assert!(!source.contains("</style>"));
            assert!(!source.contains("</script>"));
        }
    }
}
