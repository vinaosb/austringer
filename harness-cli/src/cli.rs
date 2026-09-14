//! The clap argument surface (plan todo 39 names clap): five
//! subcommands over one daemon client — `run` (headless turn),
//! `capture` (quick one-liner), `repl`, `attach`, and `doctor` — plus
//! the transport flags and the shared `--image` / `--output-json`
//! flags (global so the top-level help documents them).

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// The agent harness CLI: a thin client over the daemon.
#[derive(Debug, Parser)]
#[command(
    name = "harness",
    version,
    about = "Thin client over the harness daemon"
)]
pub struct Cli {
    /// Daemon base URL (`http://127.0.0.1:PORT`); omit for the local
    /// IPC socket.
    #[arg(long, global = true)]
    pub url: Option<String>,

    /// Auth token (default: read {data-dir}/daemon-token).
    #[arg(long, global = true)]
    pub token: Option<String>,

    /// Daemon data dir (IPC default `.harness-daemon`).
    #[arg(long, global = true)]
    pub data_dir: Option<PathBuf>,

    /// IPC socket name (default `harness-daemon`).
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
    pub command: Command,
}

/// The five subcommands.
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
    /// Sessions store root override (default `.harness/store` in cwd).
    #[arg(long)]
    pub sessions: Option<PathBuf>,
}
