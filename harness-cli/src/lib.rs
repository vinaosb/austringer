//! Command-line frontend: a thin client over the daemon's typed
//! protocol (todo 35), plus the `doctor` health surface. Plan todo 39.
//!
//! The CLI NEVER duplicates daemon logic: sessions live in the daemon,
//! turns run in the daemon, the CLI submits commands and renders the
//! event stream. Two carriers, one protocol: the WebSocket attach
//! socket (`--url`, duplex) and the interprocess local socket
//! (newline-delimited JSON duplex, the default when no `--url` is
//! given).

pub mod attach;
pub mod cli;
pub mod client;
pub mod doctor;
pub mod doctor_probes;
pub mod error;
pub mod image;
pub mod mcp;
pub mod render;
pub mod repl;
pub mod run;
pub mod target;
pub mod version;
