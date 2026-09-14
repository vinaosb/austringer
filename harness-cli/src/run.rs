//! Headless `run` and the `capture` quick one-liner (plan todo 39).
//! `run` binds a session, submits the prompt, renders the live event
//! stream until the turn finishes, and exits by outcome (`--output-json`
//! appends the eval-adapter `turn_result` line). `capture` submits and
//! returns immediately — the detached turn replays later via `attach`.

use std::time::{Duration, Instant};

use harness_daemon::protocol::{Answer, ServerMsg, SessionEvent};
use tokio::sync::mpsc;

use crate::client::SessionTransport;
use crate::error::CliError;
use crate::image::{self, PreparedImage};
use crate::render;

/// Options for a headless run.
#[derive(Debug, Clone)]
pub struct RunOptions {
    /// `--output-json` (NDJSON event envelope + turn_result).
    pub json: bool,
    /// Whole-turn deadline.
    pub timeout: Duration,
}

/// Default whole-turn deadline (comfortably under the daemon's
/// 300 s IPC idle eviction).
const DEFAULT_RUN_TIMEOUT: Duration = Duration::from_secs(120);

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            json: false,
            timeout: DEFAULT_RUN_TIMEOUT,
        }
    }
}

/// Spawn the stdin reader task (shared by run and repl): lines flow
/// out until EOF; permission answers and repl input both arrive here.
pub fn stdin_lines() -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::channel(1);
    tokio::spawn(async move {
        use tokio::io::{AsyncBufReadExt, BufReader};
        let mut stdin = BufReader::new(tokio::io::stdin());
        let mut line = String::new();
        loop {
            line.clear();
            match stdin.read_line(&mut line).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if sender.send(line.clone()).await.is_err() {
                        break;
                    }
                }
            }
        }
    });
    receiver
}

/// Run one headless turn. Returns the process exit code.
///
/// # Errors
/// typed CLI failures; a failed turn prints its `turn_result` line
/// first, then surfaces [`CliError::TurnFailed`].
pub async fn run(
    transport: &mut SessionTransport,
    prompt: &str,
    images: &[PreparedImage],
    session_dir: Option<&std::path::Path>,
    options: &RunOptions,
) -> Result<i32, CliError> {
    let mut stdin = stdin_lines();
    replay_until_live(transport, options.json).await?;
    if let Some(dir) = session_dir {
        image::store_into(dir, images)?;
    }
    transport
        .submit(&image::submit_text(prompt, images))
        .await?;
    let deadline = Instant::now() + options.timeout;
    let outcome = wait_turn(transport, options.json, Some(&mut stdin), deadline).await?;
    println!("{}", render::turn_result(outcome.0, &outcome.1));
    if render::outcome_ok(&outcome.1) {
        Ok(0)
    } else {
        Err(CliError::TurnFailed { outcome: outcome.1 })
    }
}

/// Quick capture: submit, then hold the connection until the daemon
/// CONFIRMS acceptance (its `TurnStarted` frame) before exiting — a
/// process exit racing the server's read loop otherwise drops the
/// submit on the floor (observed against the real binary).
///
/// # Errors
/// typed transport failures.
pub async fn capture(
    transport: &mut SessionTransport,
    prompt: &str,
    images: &[PreparedImage],
    session_dir: Option<&std::path::Path>,
) -> Result<i32, CliError> {
    let options = RunOptions::default();
    replay_until_live(transport, options.json).await?;
    if let Some(dir) = session_dir {
        image::store_into(dir, images)?;
    }
    transport
        .submit(&image::submit_text(prompt, images))
        .await?;
    let deadline = Instant::now() + options.timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let msg = tokio::time::timeout(remaining, transport.next_msg())
            .await
            .map_err(|_| CliError::Timeout {
                secs: remaining.as_secs().max(1),
            })??;
        let Some(msg) = msg else {
            break;
        };
        if let ServerMsg::Event { frame } = &msg
            && matches!(
                frame.event,
                harness_daemon::protocol::SessionEvent::TurnStarted { .. }
            )
        {
            break;
        }
    }
    println!("queued");
    Ok(0)
}

/// Drain replayed state and the HelloAck so submissions start from
/// the live edge. Both replay shapes render here: delta `Event`
/// frames and the post-divergence `Snapshot` batch (frames inside it
/// ARE the missed history — detached turns reappear in either).
pub(crate) async fn replay_until_live(
    transport: &mut SessionTransport,
    json: bool,
) -> Result<(), CliError> {
    loop {
        let Some(msg) = transport.next_msg().await? else {
            return Err(CliError::Protocol {
                detail: "stream ended before the handshake completed".to_owned(),
            });
        };
        match msg {
            ServerMsg::HelloAck { .. } => return Ok(()),
            ServerMsg::Snapshot { frames } => {
                for frame in frames {
                    print_msg(&ServerMsg::Event { frame }, json);
                }
            }
            other => print_msg(&other, json),
        }
    }
}

/// The shared turn-wait loop: render events, answer asks from stdin,
/// stop at TurnFinished. Returns `(turn, outcome)`.
pub(crate) async fn wait_turn(
    transport: &mut SessionTransport,
    json: bool,
    mut stdin: Option<&mut mpsc::Receiver<String>>,
    deadline: Instant,
) -> Result<(u64, String), CliError> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(CliError::Timeout { secs: 1 });
        }
        let msg = tokio::time::timeout(remaining, transport.next_msg())
            .await
            .map_err(|_| CliError::Timeout {
                secs: remaining.as_secs().max(1),
            })??;
        let Some(msg) = msg else {
            return Err(CliError::Protocol {
                detail: "stream ended mid-turn".to_owned(),
            });
        };
        if let ServerMsg::Event { frame } = &msg
            && let SessionEvent::TurnFinished { turn, outcome } = &frame.event
        {
            print_msg(&msg, json);
            return Ok((*turn, outcome.clone()));
        }
        if let ServerMsg::PermissionAsk { ask } = &msg {
            print_msg(&msg, json);
            if let Some(stdin) = stdin.as_mut()
                && let Some(line) = stdin.recv().await
            {
                let answer = parse_answer(&line);
                let ask_id = ask.ask_id;
                transport.answer(ask_id, answer).await?;
            }
            continue;
        }
        print_msg(&msg, json);
    }
}

/// Map one stdin line to a permission answer (default: deny).
pub(crate) fn parse_answer(line: &str) -> Answer {
    match line.trim().to_ascii_lowercase().as_str() {
        "y" | "yes" => Answer::AllowOnce,
        "s" | "session" => Answer::AllowSession,
        _ => Answer::Deny,
    }
}

pub(crate) fn print_msg(msg: &ServerMsg, json: bool) {
    if json {
        let line = match msg {
            ServerMsg::Event { frame } => render::frame_envelope(frame),
            other => serde_json::to_string(other).unwrap_or_else(|_| "{}".to_owned()),
        };
        println!("{line}");
        return;
    }
    if let Some(line) = render::human(msg) {
        println!("{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdin_answers_map_to_typed_verdicts_defaulting_to_deny() {
        assert_eq!(parse_answer("y\n"), Answer::AllowOnce);
        assert_eq!(parse_answer("YES"), Answer::AllowOnce);
        assert_eq!(parse_answer("s\n"), Answer::AllowSession);
        assert_eq!(parse_answer("n\n"), Answer::Deny);
        assert_eq!(parse_answer("garbage"), Answer::Deny);
        assert_eq!(parse_answer(""), Answer::Deny);
    }
}
