//! Transport target resolution: flags (`--url`, `--token`,
//! `--data-dir`, `--ipc-name`) into one typed [`Target`]. `--url`
//! selects the HTTP transport (POST commands + SSE events); without
//! it the CLI speaks the interprocess local-socket protocol, reading
//! the daemon's token from `{data-dir}/daemon-token` unless one was
//! passed explicitly.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::CliError;

/// The daemon's token filename under its data dir (todo 35 contract).
pub const TOKEN_FILENAME: &str = "daemon-token";

/// Default IPC name and data dir, matching the daemon binary's
/// `DaemonConfig::default()`.
pub const DEFAULT_IPC_NAME: &str = "harness-daemon";
pub const DEFAULT_DATA_DIR: &str = ".harness-daemon";

/// Raw flag inputs before resolution.
#[derive(Debug, Clone, Default)]
pub struct TargetSpec {
    /// `--url http://127.0.0.1:PORT` selects HTTP.
    pub url: Option<String>,
    /// `--token` (skips the token file when present).
    pub token: Option<String>,
    /// `--data-dir` (token file lookup + session media store).
    pub data_dir: Option<PathBuf>,
    /// `--ipc-name`.
    pub ipc_name: Option<String>,
}

/// A resolved daemon connection.
#[derive(Debug, Clone)]
pub enum Target {
    /// HTTP/SSE transport.
    Http {
        /// Base URL, no trailing slash.
        base: String,
        /// Bearer token when one is known.
        token: Option<String>,
    },
    /// Local-socket transport (the default).
    Ipc {
        /// Namespaced socket name.
        name: String,
        /// Daemon data dir (token file, session media).
        data_dir: PathBuf,
        /// The daemon token.
        token: String,
    },
}

impl Target {
    /// Resolve flags into a target. HTTP without `--token` stays
    /// token-less deliberately: connecting dominates, and a live daemon
    /// answers the missing token with a typed 401.
    pub fn resolve(spec: &TargetSpec) -> Result<Self, CliError> {
        match &spec.url {
            Some(url) => Ok(Self::Http {
                base: url.trim_end_matches('/').to_owned(),
                token: match (&spec.token, &spec.data_dir) {
                    (Some(token), _) => Some(token.clone()),
                    (None, Some(data_dir)) => read_token(data_dir),
                    (None, None) => None,
                },
            }),
            None => {
                let data_dir = spec
                    .data_dir
                    .clone()
                    .unwrap_or_else(|| PathBuf::from(DEFAULT_DATA_DIR));
                let token = match &spec.token {
                    Some(token) => token.clone(),
                    None => read_token(&data_dir).ok_or_else(|| CliError::Unreachable {
                        detail: format!(
                            "no token: {} not found (start the daemon or pass --token)",
                            data_dir.join(TOKEN_FILENAME).display()
                        ),
                    })?,
                };
                Ok(Self::Ipc {
                    name: spec
                        .ipc_name
                        .clone()
                        .unwrap_or_else(|| DEFAULT_IPC_NAME.to_owned()),
                    data_dir,
                    token,
                })
            }
        }
    }

    /// The session media dir under this target's daemon data dir
    /// (`{data}/sessions/{sid}/media`), when the data dir is known.
    #[must_use]
    pub fn session_dir(&self, session: &str) -> Option<PathBuf> {
        match self {
            Self::Http { .. } => None,
            Self::Ipc { data_dir, .. } => Some(data_dir.join("sessions").join(session)),
        }
    }
}

/// Read the daemon-generated token file (trailing newline tolerated).
fn read_token(data_dir: &Path) -> Option<String> {
    std::fs::read_to_string(data_dir.join(TOKEN_FILENAME))
        .ok()
        .map(|token| token.trim().to_owned())
        .filter(|token| !token.is_empty())
}

/// A client-side session id: printable ASCII, unique per invocation.
#[must_use]
pub fn default_session() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis());
    format!("cli-{}-{}", std::process::id(), millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_selects_http_and_trims_the_trailing_slash() {
        let target = Target::resolve(&TargetSpec {
            url: Some("http://127.0.0.1:8080/".into()),
            token: Some("t".into()),
            data_dir: None,
            ipc_name: None,
        })
        .unwrap_or_else(|e| panic!("resolve: {e}"));
        match target {
            Target::Http { base, token } => {
                assert_eq!(base, "http://127.0.0.1:8080");
                assert_eq!(token.as_deref(), Some("t"));
            }
            other => panic!("expected http, got {other:?}"),
        }
    }

    #[test]
    fn http_token_falls_back_to_the_data_dir_file() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
        std::fs::write(dir.path().join(TOKEN_FILENAME), b"file-token\n").unwrap_or_default();
        let target = Target::resolve(&TargetSpec {
            url: Some("http://127.0.0.1:1".into()),
            token: None,
            data_dir: Some(dir.path().to_path_buf()),
            ipc_name: None,
        })
        .unwrap_or_else(|e| panic!("resolve: {e}"));
        match target {
            Target::Http { token, .. } => assert_eq!(token.as_deref(), Some("file-token")),
            other => panic!("expected http, got {other:?}"),
        }
    }

    #[test]
    fn ipc_defaults_come_from_the_daemon_config() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
        std::fs::write(dir.path().join(TOKEN_FILENAME), b"ipc-token").unwrap_or_default();
        let target = Target::resolve(&TargetSpec {
            url: None,
            token: None,
            data_dir: Some(dir.path().to_path_buf()),
            ipc_name: Some("custom-pipe".into()),
        })
        .unwrap_or_else(|e| panic!("resolve: {e}"));
        match target {
            Target::Ipc {
                name,
                data_dir,
                token,
            } => {
                assert_eq!(name, "custom-pipe");
                assert_eq!(token, "ipc-token");
                assert_eq!(data_dir, dir.path().to_path_buf());
            }
            other => panic!("expected ipc, got {other:?}"),
        }
        let session_dir = Target::Ipc {
            name: String::new(),
            data_dir: dir.path().to_path_buf(),
            token: String::new(),
        }
        .session_dir("s1")
        .unwrap_or_default();
        assert_eq!(session_dir, dir.path().join("sessions").join("s1"));
    }

    #[test]
    fn missing_ipc_token_file_is_a_typed_unreachable() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
        let error = Target::resolve(&TargetSpec {
            url: None,
            token: None,
            data_dir: Some(dir.path().to_path_buf()),
            ipc_name: None,
        })
        .unwrap_err();
        assert!(error.to_string().contains("unreachable"), "{error}");
    }

    #[test]
    fn default_session_ids_are_unique_and_printable() {
        let first = default_session();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let second = default_session();
        assert_ne!(first, second);
        assert!(first.bytes().all(|b| b.is_ascii_graphic()));
    }
}
