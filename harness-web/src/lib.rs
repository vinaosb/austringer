//! Static web dashboard assets for the harness; contains no server logic.
//!
//! The daemon (todo 35) serves these bytes over HTTP with content-type
//! and cache headers. The real SPA lands with todo 37; the registry
//! below is the serving contract frontends compile against.

/// The SPA entry document.
pub const INDEX_HTML: &str = include_str!("../assets/index.html");

/// One servable asset: (route path, content-type, bytes).
pub fn assets() -> &'static [(&'static str, &'static str, &'static str)] {
    &[("/", "text/html; charset=utf-8", INDEX_HTML)]
}

#[test]
fn workspace_scaffold_smoke() {
    let scaffold_in_place = true;
    assert!(scaffold_in_place);
}
