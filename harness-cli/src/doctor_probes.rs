//! Doctor probes (plan todo 39): config/provider/sandbox/MCP/sessions
//! health + env + migration + network — all passive (no provider HTTP,
//! no MCP connections, no sandbox enforcement, no session creation on
//! the daemon). Every probe is bounded and typed.

use std::path::{Path, PathBuf};

use harness_sessions::SessionError;
use harness_sessions::store::SessionStore;

use crate::error::CliError;
use crate::target::Target;

/// Provider names the daemon's factory knows (todo 40 wires them; the
/// doctor validates the *selection*, never the credentials) plus the
/// env var each reads its key from.
const PROVIDER_KEYS: [(&str, &str); 4] = [
    ("anthropic", "ANTHROPIC_API_KEY"),
    ("openai", "OPENAI_API_KEY"),
    ("google", "GOOGLE_API_KEY"),
    ("xai", "XAI_API_KEY"),
];

/// One probe outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    /// Probe name as printed ("daemon", "provider", …).
    pub name: &'static str,
    /// PASS / WARN / FAIL.
    pub status: Status,
    /// Bounded human detail.
    pub detail: String,
}

/// Probe verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Healthy.
    Pass,
    /// Suspicious but not fatal.
    Warn,
    /// Broken; doctor exits nonzero.
    Fail,
}

/// Daemon reachability (also the loopback network probe). Deliberately
/// side-effect free: HTTP GETs `/sessions`; IPC only opens the socket —
/// no Hello, so no session is ever created on the daemon.
pub async fn daemon(target: &Target) -> Probe {
    let (detail, healthy) = match target {
        Target::Http { base, token } => match http_probe(base, token.as_deref()).await {
            Ok(()) => (format!("reachable at {base}"), true),
            Err(error) => (error, false),
        },
        Target::Ipc { name, .. } => match ipc_probe(name).await {
            Ok(()) => (format!("local socket {name} accepting connections"), true),
            Err(error) => (error, false),
        },
    };
    Probe {
        name: "daemon",
        status: if healthy { Status::Pass } else { Status::Fail },
        detail,
    }
}

async fn http_probe(base: &str, token: Option<&str>) -> Result<(), String> {
    let mut request = reqwest::Client::new().get(format!("{base}/sessions"));
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    match request.send().await {
        Ok(response) => match response.status().as_u16() {
            200 => Ok(()),
            401 | 403 => Err(format!(
                "reachable but refused the token ({})",
                response.status()
            )),
            status => Err(format!("unexpected status {status}")),
        },
        Err(error) => Err(format!("{error}")),
    }
}

async fn ipc_probe(name: &str) -> Result<(), String> {
    use interprocess::local_socket::GenericNamespaced;
    use interprocess::local_socket::tokio::Stream;
    use interprocess::local_socket::tokio::prelude::*;
    let ns_name = name
        .to_ns_name::<GenericNamespaced>()
        .map_err(|error| format!("bad ipc name: {error}"))?;
    Stream::connect(ns_name)
        .await
        .map_err(|error| format!("local socket {name}: {error}"))?;
    Ok(())
}

/// The loopback network probe rides the daemon connectivity result.
pub fn network(daemon: &Probe) -> Probe {
    Probe {
        name: "network",
        status: daemon.status,
        detail: match daemon.status {
            Status::Pass => "loopback transport healthy".to_owned(),
            _ => format!("loopback unhealthy: {}", daemon.detail),
        },
    }
}

/// Selected-provider validity (HARNESS_PROVIDER) plus its key env.
pub fn provider() -> Probe {
    let Some(selected) = std::env::var("HARNESS_PROVIDER").ok() else {
        return Probe {
            name: "provider",
            status: Status::Pass,
            detail: "no HARNESS_PROVIDER selected".to_owned(),
        };
    };
    let Some((_, key_env)) = PROVIDER_KEYS.iter().find(|(known, _)| *known == selected) else {
        return Probe {
            name: "provider",
            status: Status::Fail,
            detail: format!(
                "HARNESS_PROVIDER={selected} is not a known provider (known: {})",
                PROVIDER_KEYS
                    .iter()
                    .map(|(known, _)| *known)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
    };
    if std::env::var_os(key_env).is_some_and(|value| !value.is_empty()) {
        Probe {
            name: "provider",
            status: Status::Pass,
            detail: format!("{selected} selected, {key_env} present"),
        }
    } else {
        Probe {
            name: "provider",
            status: Status::Warn,
            detail: format!("{selected} selected but {key_env} is unset"),
        }
    }
}

/// Informational harness-relevant env listing (never fails).
pub fn env() -> Probe {
    let names = [
        "HARNESS_PROVIDER",
        "HARNESS_DAEMON_URL",
        "NO_COLOR",
        "CLICOLOR",
    ];
    let present: Vec<&str> = names
        .iter()
        .copied()
        .filter(|name| std::env::var_os(name).is_some())
        .collect();
    Probe {
        name: "env",
        status: Status::Pass,
        detail: if present.is_empty() {
            "no harness env overrides".to_owned()
        } else {
            format!("set: {}", present.join(", "))
        },
    }
}

/// Platform sandbox backend name (passive — no enforcement run).
pub fn sandbox() -> Probe {
    let backend = harness_safety::backends::platform_backend();
    Probe {
        name: "sandbox",
        status: Status::Pass,
        detail: format!("backend: {}", backend.name()),
    }
}

/// MCP config scan of the working directory (parse-only, no dialing).
pub fn mcp() -> Probe {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let report = harness_mcp::manager::scan_directories(&[cwd]);
    if let Some(first) = report.diagnostics.first() {
        return Probe {
            name: "mcp",
            status: Status::Fail,
            detail: format!("{}: {}", first.path.display(), first.message),
        };
    }
    Probe {
        name: "mcp",
        status: Status::Pass,
        detail: format!(
            "{} server entries, config parses",
            report.installations.len()
        ),
    }
}

/// Sessions store open + schema migration probe. A root that does not
/// exist yet is a healthy fresh install (opening it here would create
/// state as a side effect — deliberately avoided).
pub fn sessions(root: &Path) -> Probe {
    if !root.join("store.db").exists() {
        return Probe {
            name: "sessions",
            status: Status::Pass,
            detail: "no store yet (fresh)".to_owned(),
        };
    }
    match SessionStore::open(root) {
        Ok(store) => {
            let count = store.list().map_or(0, |ids| ids.len());
            Probe {
                name: "sessions",
                status: Status::Pass,
                detail: format!("store opens, {count} sessions, schema current"),
            }
        }
        Err(SessionError::Schema { version, .. }) => Probe {
            name: "sessions",
            status: Status::Fail,
            detail: format!("store schema {version} needs a migration this build cannot do"),
        },
        Err(error) => Probe {
            name: "sessions",
            status: Status::Fail,
            detail: error.to_string(),
        },
    }
}

/// Skills scan of the working directory (multi-vendor loader).
pub fn skills() -> Probe {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut loader = harness_skills::loader::SkillLoader::new();
    loader.vendor_root(cwd);
    match loader.load() {
        Ok(index) => {
            let report = index.report();
            let errors = report.parse_errors.len();
            let parsed = index.metas().count();
            Probe {
                name: "skills",
                status: if errors == 0 {
                    Status::Pass
                } else {
                    Status::Warn
                },
                detail: format!("{parsed} skills indexed, {errors} parse errors"),
            }
        }
        Err(error) => Probe {
            name: "skills",
            status: Status::Fail,
            detail: error.to_string(),
        },
    }
}

/// Convert a probe failure into the doctor's typed error surface.
pub fn as_cli_error(failed: &Probe) -> CliError {
    CliError::Protocol {
        detail: format!("doctor: {} failed: {}", failed.name, failed.detail),
    }
}
