//! The process backend driver: spawns the external harness child over
//! piped stdio, writes one JSONL request, reads one JSONL response under
//! a deadline, and kills the child on drop (no orphans). Disabled configs
//! refuse loudly without spawning anything.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use super::protocol::{PromptRequest, PromptResponse, decode_response, encode_request};

/// Process-backend failure.
#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    /// The backend is disabled (supply-chain carve-out).
    #[error("backend disabled (opt-in required)")]
    Disabled,
    /// Spawning the child failed.
    #[error("spawn failed: {detail}")]
    Spawn {
        /// Detail.
        detail: String,
    },
    /// Writing the request failed.
    #[error("stdin write failed: {detail}")]
    WriteStdin {
        /// Detail.
        detail: String,
    },
    /// Reading the response failed.
    #[error("stdout read failed: {detail}")]
    ReadStdout {
        /// Detail.
        detail: String,
    },
    /// The response line failed protocol decoding.
    #[error(transparent)]
    Protocol(#[from] super::protocol::ProtocolError),
    /// The deadline elapsed; the child was killed.
    #[error("backend timed out after {secs}s")]
    Timeout {
        /// Seconds elapsed.
        secs: u64,
    },
    /// The child exited nonzero without a response.
    #[error("child exited {code}")]
    NonZeroExit {
        /// Exit code.
        code: u32,
    },
    /// The response id does not match the request.
    #[error("response id mismatch: {id}")]
    ResponseMismatch {
        /// The mismatched id.
        id: u64,
    },
}

/// A configured external-harness backend.
#[derive(Debug, Clone)]
pub struct ProcessBackend {
    kind: super::BackendKind,
    config: super::BackendConfig,
}

impl ProcessBackend {
    /// Builds the backend for a kind and config.
    pub fn new(kind: super::BackendKind, config: super::BackendConfig) -> Self {
        Self { kind, config }
    }

    /// Which harness this backend drives.
    pub fn kind(&self) -> super::BackendKind {
        self.kind
    }

    /// Runs one prompt/request exchange with the child process.
    pub fn run(
        &self,
        request: &PromptRequest,
        env: &BTreeMap<String, String>,
        timeout: Duration,
    ) -> Result<PromptResponse, BackendError> {
        if !self.config.enabled {
            return Err(BackendError::Disabled);
        }
        let mut command = Command::new(&self.config.program);
        command
            .args(&self.config.args)
            .env_clear()
            .envs(env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command.spawn().map_err(|e| BackendError::Spawn {
            detail: format!("{}: {e}", self.config.program),
        })?;

        let line = encode_request(request);
        let mut stdin = child.stdin.take();
        if let Some(stdin) = stdin.as_mut() {
            writeln!(stdin, "{line}")
                .and_then(|_| stdin.flush())
                .map_err(|e| BackendError::WriteStdin {
                    detail: e.to_string(),
                })?;
        }
        drop(stdin);

        let stdout = child.stdout.take();
        let (tx, rx) = mpsc::channel::<String>();
        let reader = std::thread::spawn(move || {
            let Some(stdout) = stdout else {
                return;
            };
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => {
                        if tx.send(line).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let deadline = std::time::Instant::now() + timeout;
        let mut response: Option<PromptResponse> = None;
        let mut last_err = super::protocol::ProtocolError::BadJson {
            detail: "child closed stdout without a response".to_string(),
        };
        let mut timed_out = false;
        loop {
            let Some(remaining) = deadline.checked_duration_since(std::time::Instant::now()) else {
                timed_out = true;
                break;
            };
            match rx.recv_timeout(remaining) {
                Ok(line) => match decode_response(&line) {
                    Ok(parsed) => {
                        response = Some(parsed);
                        break;
                    }
                    Err(err) => last_err = err,
                },
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    timed_out = true;
                    break;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        let response = if let Some(parsed) = response {
            parsed
        } else {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            if timed_out {
                return Err(BackendError::Timeout {
                    secs: timeout.as_secs(),
                });
            }
            return Err(last_err.into());
        };
        drop(rx);
        let _ = reader.join();
        if response.id != request.id {
            return Err(BackendError::ResponseMismatch { id: response.id });
        }
        let code = child.wait().map_err(|e| BackendError::ReadStdout {
            detail: e.to_string(),
        })?;
        if !code.success()
            && !response.ok
            && let Some(exit) = code.code()
        {
            return Err(BackendError::NonZeroExit { code: exit as u32 });
        }
        Ok(response)
    }
}
