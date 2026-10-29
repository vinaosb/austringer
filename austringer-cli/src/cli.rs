//! The clap argument surface (plan todo 39 names clap): five
//! daemon-client subcommands — `run` (headless turn), `capture`
//! (quick one-liner), `repl`, `attach`, and `doctor` — plus the
//! daemon-less `mcp` family (todo 7), the transport flags, and the
//! shared `--image` / `--output-json` flags (global so the top-level
//! help documents them).

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// The agent austringer CLI: a thin client over the daemon.
#[derive(Debug, Parser)]
#[command(
    name = "austringer",
    version,
    about = "Thin client over the austringer daemon"
)]
pub struct Cli {
    /// Daemon base URL (`http://127.0.0.1:PORT`); omit for the local
    /// IPC socket.
    #[arg(long, global = true)]
    pub url: Option<String>,

    /// Auth token (default: read {data-dir}/daemon-token).
    #[arg(long, global = true)]
    pub token: Option<String>,

    /// Daemon data dir (IPC default `.austringer-daemon`).
    #[arg(long, global = true)]
    pub data_dir: Option<PathBuf>,

    /// IPC socket name (default `austringer-daemon`).
    #[arg(long, global = true)]
    pub ipc_name: Option<String>,

    /// Image attachment, repeatable (png/jpeg/gif/webp); applies to
    /// `run`/`capture` submits and the first REPL submit.
    #[arg(long = "image", global = true)]
    pub images: Vec<PathBuf>,

    /// Emit NDJSON (eval adapter contract) instead of human text
    /// (`run` event stream + `turn_result`, `attach` envelopes).
    #[arg(long = "output-json", global = true)]
    pub output_json: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// The five daemon subcommands plus the daemon-less `mcp` family;
/// `None` (bare `austringer`) execs the TUI — the codex/opencode
/// default-experience pattern (research §34.2).
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run one headless turn and print its result.
    Run(RunArgs),
    /// Quick capture: submit a prompt without waiting for the turn.
    Capture(CaptureArgs),
    /// Interactive REPL.
    Repl(ReplArgs),
    /// Attach to a session and render its event stream.
    Attach(AttachArgs),
    /// Health-check config, provider, sandbox, MCP, sessions, env.
    Doctor(DoctorArgs),
    /// MCP surface: list scanned servers, serve austringer over stdio,
    /// smoke-call one server (never contacts the daemon).
    Mcp(McpArgs),
    /// Session-store surface over the daemon's HTTP routes (todo 11:
    /// transcript search; todo 22: session list + run history).
    Sessions(SessionsArgs),
    /// Ensure the daemon is running and open its served web dashboard.
    Web(WebArgs),
}

/// `web` arguments.
#[derive(Debug, Args)]
pub struct WebArgs {
    /// Print the dashboard URL without launching a browser.
    #[arg(long = "no-browser")]
    pub no_browser: bool,
}

/// `mcp` arguments.
#[derive(Debug, Args)]
pub struct McpArgs {
    #[command(subcommand)]
    pub command: McpCommand,
}

/// The `mcp` subcommands.
#[derive(Debug, Subcommand)]
pub enum McpCommand {
    /// List MCP server entries found in config files (parse-only).
    #[command(visible_alias = "scan")]
    List(McpListArgs),
    /// Serve the austringer MCP server over stdio until disconnect.
    Serve,
    /// Smoke-test one configured server: connect, then list tools
    /// (or call one with `--tool`/`--args`).
    Call(McpCallArgs),
}

/// `mcp list` arguments.
#[derive(Debug, Args)]
pub struct McpListArgs {
    /// Directory to scan (repeatable; default: the working directory).
    #[arg(long = "dir")]
    pub dirs: Vec<PathBuf>,
}

/// `mcp call` arguments.
#[derive(Debug, Args)]
pub struct McpCallArgs {
    /// Server name as keyed in the scanned config files.
    pub server: String,

    /// Call this one tool instead of listing tools.
    #[arg(long)]
    pub tool: Option<String>,

    /// Tool arguments as one JSON object (with `--tool`).
    #[arg(long)]
    pub args: Option<String>,

    /// Directory to scan (repeatable; default: the working directory).
    #[arg(long = "dir")]
    pub dirs: Vec<PathBuf>,
}

/// `sessions` arguments.
#[derive(Debug, Args)]
pub struct SessionsArgs {
    #[command(subcommand)]
    pub command: SessionsCommand,
}

/// The `sessions` subcommands.
#[derive(Debug, Subcommand)]
pub enum SessionsCommand {
    /// Full-text search (FTS5) over session transcripts through the
    /// daemon's search route; hits are ranked by bm25. Requires `--url`
    /// (the HTTP transport).
    Search(SessionsSearchArgs),
    /// Every live session id in the daemon (its `GET /sessions`
    /// route). Requires `--url` (the HTTP transport).
    List,
    /// One session's scheduled-run lifecycle rows with their gate
    /// verdicts (the daemon's `GET /s/{sid}/history` route). Requires
    /// `--url` (the HTTP transport).
    History(SessionsHistoryArgs),
}

/// `sessions search` arguments.
#[derive(Debug, Args)]
pub struct SessionsSearchArgs {
    /// Maximum hits (the daemon caps at 100).
    #[arg(long)]
    pub limit: Option<u32>,

    /// Comma-separated session ids to scope (default: all sessions in
    /// the daemon's store).
    #[arg(long)]
    pub sessions: Option<String>,

    /// The FTS5 query (a word, a quoted phrase, `a OR b`).
    #[arg(trailing_var_arg = true, required = true)]
    pub query: Vec<String>,
}

/// `sessions history` arguments.
#[derive(Debug, Args)]
pub struct SessionsHistoryArgs {
    /// The session id whose run rows are listed.
    pub session: String,
}

/// `run` arguments.
#[derive(Debug, Args)]
pub struct RunArgs {
    /// Session id (default: a fresh `cli-<pid>-<millis>` id).
    #[arg(long)]
    pub session: Option<String>,

    /// Whole-turn deadline in seconds (default 120).
    #[arg(long)]
    pub timeout: Option<u64>,

    /// The prompt text.
    #[arg(trailing_var_arg = true, required = true)]
    pub prompt: Vec<String>,
}

/// `capture` arguments.
#[derive(Debug, Args)]
pub struct CaptureArgs {
    /// Session id (default: a fresh `cli-<pid>-<millis>` id).
    #[arg(long)]
    pub session: Option<String>,

    /// The prompt text.
    #[arg(trailing_var_arg = true, required = true)]
    pub prompt: Vec<String>,
}

/// `repl` arguments (the prompt arrives interactively).
#[derive(Debug, Args)]
pub struct ReplArgs {
    /// Session id (default: a fresh `cli-<pid>-<millis>` id).
    #[arg(long)]
    pub session: Option<String>,
}

/// `attach` arguments.
#[derive(Debug, Args)]
pub struct AttachArgs {
    /// Session id to observe.
    #[arg(long)]
    pub session: Option<String>,

    /// Replay threshold (`?after=`; 0 replays everything retained).
    #[arg(long)]
    pub after: Option<u64>,

    /// Stop after N event frames (omit to follow live).
    #[arg(long)]
    pub max_events: Option<u64>,
}

/// `doctor` arguments.
#[derive(Debug, Args)]
pub struct DoctorArgs {
    /// Sessions store root override (default `<cwd>/.austringer-daemon`,
    /// the daemon data dir).
    #[arg(long)]
    pub sessions: Option<PathBuf>,
}
