//! Per-language server table and client configuration.
//!
//! Availability is the user environment's decision: the default table names
//! the five planned servers (`rust-analyzer`, `gopls`,
//! `typescript-language-server --stdio` — raw tsserver speaks no LSP —
//! `clangd`, `pyright-langserver --stdio`) and whatever is not on PATH
//! fails at spawn time with a typed [`LspError::ServerUnavailable`].

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::proto::FrameCaps;

/// Callback invoked per captured server stderr line (bounded length).
/// When `None`, lines land in a small per-server ring buffer instead —
/// server stderr never reaches the host's stderr either way.
pub type StderrHook = Arc<dyn Fn(&str) + Send + Sync>;

/// Languages with a default server command. The set is closed for routing;
/// extending it is a deliberate config change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Language {
    /// Rust (`rust-analyzer`).
    Rust,
    /// Go (`gopls`).
    Go,
    /// TypeScript and JavaScript (`typescript-language-server --stdio`).
    TypeScript,
    /// C (`clangd`).
    C,
    /// C++ (`clangd`).
    Cpp,
    /// Python (`pyright-langserver --stdio`).
    Python,
}

impl Language {
    /// Every routable language, in declaration order.
    pub const ALL: &'static [Language] = &[
        Language::Rust,
        Language::Go,
        Language::TypeScript,
        Language::C,
        Language::Cpp,
        Language::Python,
    ];

    /// The LSP `languageId` string for this language.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Go => "go",
            Self::TypeScript => "typescript",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::Python => "python",
        }
    }

    /// Human-readable name (also used in `NoServer` errors for unknown
    /// extensions, so it stays stable).
    #[must_use]
    pub fn name(self) -> &'static str {
        self.id()
    }
}

/// How to launch one language server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerCommand {
    /// Program name or path (resolved via the user's PATH).
    pub program: String,
    /// Arguments (e.g. `--stdio` for the node-based servers).
    pub args: Vec<String>,
}

/// Deterministic exponential restart backoff. No jitter: restarts are
/// per-language and serialized inside the router, so there is no thundering
/// herd to spread — determinism buys directly testable timings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Backoff {
    /// Wait before the first restart (after the first crash).
    pub initial: Duration,
    /// Growth factor per additional crash (`>= 1`).
    pub multiplier: f64,
    /// Upper bound on any single wait.
    pub max: Duration,
}

impl Default for Backoff {
    fn default() -> Self {
        Self {
            initial: Duration::from_millis(500),
            multiplier: 2.0,
            max: Duration::from_secs(30),
        }
    }
}

impl Backoff {
    /// Wait before spawn attempt `crashes + 1`. The first spawn of a slot
    /// (`crashes == 0`) never waits; each observed crash grows the wait by
    /// `multiplier`, capped at `max`.
    #[must_use]
    pub fn wait_for(&self, crashes: u32) -> Duration {
        if crashes == 0 {
            return Duration::ZERO;
        }
        let growth = self
            .multiplier
            .max(1.0)
            .powi((crashes - 1).min(u32::from(u16::MAX)) as i32);
        let scaled = self.initial.as_secs_f64() * growth;
        Duration::from_secs_f64(scaled.min(self.max.as_secs_f64()))
    }
}

/// RSS-sampled bounded-LSP guard knobs (forge §26: LSP processes are the
/// memory hogs of coding harnesses; the guard stays cheap enough to run
/// always — one targeted per-PID refresh per interval).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RssGuardConfig {
    /// Whether the periodic sampler runs at all.
    pub enabled: bool,
    /// Sampling interval.
    pub sample_interval: Duration,
    /// Resident-set cap per server process; at or above it the server is
    /// killed and restarted through the normal crash machinery.
    pub rss_cap_bytes: u64,
}

impl Default for RssGuardConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sample_interval: Duration::from_secs(30),
            rss_cap_bytes: 2 * 1024 * 1024 * 1024, // 2 GiB
        }
    }
}

/// File-extension routing. Case-insensitive.
#[must_use]
pub fn language_for_extension(ext: &str) -> Option<Language> {
    match ext.to_ascii_lowercase().as_str() {
        "rs" => Some(Language::Rust),
        "go" => Some(Language::Go),
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => Some(Language::TypeScript),
        "c" | "h" => Some(Language::C),
        "cpp" | "cc" | "cxx" | "c++" | "hpp" | "hh" | "hxx" => Some(Language::Cpp),
        "py" | "pyi" => Some(Language::Python),
        _ => None,
    }
}

/// Full client configuration. Cloning it is the "resume" story: a new
/// router built from the same table lazily re-spawns the same servers.
#[derive(Clone)]
pub struct LspConfig {
    /// Workspace root sent as `rootUri` at initialize time.
    pub root: PathBuf,
    /// Language → command table (the default covers the five planned
    /// languages; entries are added/replaced via [`LspConfig::with_server`]).
    pub servers: BTreeMap<Language, ServerCommand>,
    /// IDE-as-toolbox priority flag: when a bridge provider covers a
    /// language, route to it and never spawn the native server (default
    /// `true`). `false` ignores the bridge entirely.
    pub prefer_bridge: bool,
    /// How long to wait for the `initialize` response.
    pub initialize_timeout: Duration,
    /// How long to wait for any single request response.
    pub request_timeout: Duration,
    /// Restart backoff shape.
    pub backoff: Backoff,
    /// Crash budget per language: the number of crashes that still get a
    /// restart. `1` (default) means first crash → restart, second crash →
    /// degraded-to-search.
    pub max_restarts: u32,
    /// Base-protocol framing caps (bounded reads).
    pub framing: FrameCaps,
    /// Maximum file text accepted for didOpen/didChange (full-text sync).
    pub file_text_cap_bytes: u64,
    /// Optional per-line stderr sink for spawned servers.
    pub stderr_hook: Option<StderrHook>,
    /// RSS guard knobs.
    pub rss: RssGuardConfig,
}

impl LspConfig {
    /// Default configuration for a workspace rooted at `root` with the
    /// five-language default server table.
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            servers: default_servers(),
            prefer_bridge: true,
            initialize_timeout: Duration::from_secs(60),
            request_timeout: Duration::from_secs(60),
            backoff: Backoff::default(),
            max_restarts: 1,
            framing: FrameCaps::default(),
            file_text_cap_bytes: 2 * 1024 * 1024,
            stderr_hook: None,
            rss: RssGuardConfig::default(),
        }
    }

    /// Replace or add the command for one language (builder-style).
    #[must_use]
    pub fn with_server(mut self, language: Language, command: ServerCommand) -> Self {
        self.servers.insert(language, command);
        self
    }
}

/// The default five-language table. Actual availability is decided by the
/// user's PATH at spawn time.
#[must_use]
pub fn default_servers() -> BTreeMap<Language, ServerCommand> {
    let clangd = || ServerCommand {
        program: "clangd".into(),
        args: vec![],
    };
    [
        (
            Language::Rust,
            ServerCommand {
                program: "rust-analyzer".into(),
                args: vec![],
            },
        ),
        (
            Language::Go,
            ServerCommand {
                program: "gopls".into(),
                args: vec![],
            },
        ),
        (
            Language::TypeScript,
            ServerCommand {
                program: "typescript-language-server".into(),
                args: vec!["--stdio".into()],
            },
        ),
        (Language::C, clangd()),
        (Language::Cpp, clangd()),
        (
            Language::Python,
            ServerCommand {
                program: "pyright-langserver".into(),
                args: vec!["--stdio".into()],
            },
        ),
    ]
    .into()
}
