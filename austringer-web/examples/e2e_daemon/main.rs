//! E2E daemon driver for the web frontend (todo 37). Two modes:
//!
//! * `--serve-for-e2e` — serve on 127.0.0.1:18937 with a fixed token
//!   and a mock worker whose `echo` tool returns a diff; prints one
//!   JSON line for the playwright driver, then serves until killed.
//! * default — pasteable QA transcript: served content + headers,
//!   auth negatives, a full submitted turn answered through the real
//!   permission mediator, and the absent-daemon failure.
//!
//! Run: `cargo run -p austringer-web --example e2e_daemon [-- --serve-for-e2e]`

mod daemon;
mod transcript;

#[tokio::main]
async fn main() -> Result<(), String> {
    if std::env::args().any(|arg| arg == "--serve-for-e2e") {
        daemon::serve().await
    } else {
        transcript::run().await
    }
}
