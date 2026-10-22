//! Interactive REPL (plan todo 39, unified grammar todo 23): colored
//! prompt, streaming turns, the shared slash-command grammar
//! (`/help`, `/steer`, `/cancel`, `/vote-cancel`, `/mode`, `/rewind`,
//! `/skill` plus the CLI-local `/exit`, `/image`), and permission
//! asks answered interactively. Lines that are not commands submit a
//! turn and stream it to completion before the next prompt (matching
//! the headless run loop exactly).

use std::path::Path;
use std::time::{Duration, Instant};

use harness_daemon::grammar;
use tokio::sync::mpsc;

use crate::client::SessionTransport;
use crate::error::CliError;
use crate::image::{self, PreparedImage};
use crate::repl_input::{parse_line, ReplLine};
use crate::run::{replay_until_live, wait_turn, TurnWait};

/// Per-turn deadline inside the REPL.
const REPL_TURN_TIMEOUT: Duration = Duration::from_secs(300);

/// Run the REPL until `/exit` or stdin EOF. Returns the exit code.
///
/// # Errors
/// typed transport failures (a failed turn prints and continues).
pub async fn repl(
    transport: &mut SessionTransport,
    images: &[PreparedImage],
    session_dir: Option<&Path>,
) -> Result<i32, CliError> {
    replay_until_live(transport, false).await?;
    println!("harness repl — /help lists commands");
    let mut stdin = crate::run::stdin_lines();
    let mut session = ReplSession {
        transport,
        pending: images.to_vec(),
        session_dir,
        stdin: &mut stdin,
        open_asks: Vec::new(),
    };
    while let Some(line) = session.stdin.recv().await {
        let line = line.trim().to_owned();
        if line.is_empty() {
            continue;
        }
        if session.handle(&line).await? == Flow::Exit {
            return Ok(0);
        }
    }
    Ok(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    Continue,
    Exit,
}

struct ReplSession<'a> {
    transport: &'a mut SessionTransport,
    pending: Vec<PreparedImage>,
    session_dir: Option<&'a Path>,
    stdin: &'a mut mpsc::Receiver<String>,
    open_asks: Vec<u64>,
}

impl ReplSession<'_> {
    async fn handle(&mut self, line: &str) -> Result<Flow, CliError> {
        match parse_line(line) {
            ReplLine::Exit => return Ok(Flow::Exit),
            ReplLine::Help => println!("{}", grammar::cheat_sheet()),
            ReplLine::Steer(text) => self.transport.steer(&text).await?,
            ReplLine::Cancel => self.transport.cancel().await?,
            ReplLine::VoteCancel(ask_id) => self.vote_cancel(ask_id).await?,
            ReplLine::Mode(name) => self.mode_command(&name).await?,
            ReplLine::Rewind(steps) => self.transport.rewind(steps.unwrap_or(1)).await?,
            ReplLine::Image(path) => self.attach(&path),
            ReplLine::Skill(text) => self.submit_and_wait(&text).await?,
            ReplLine::Malformed(usage) => println!("{usage}"),
            ReplLine::Unknown(word) => println!("{}", grammar::unknown_help(&word)),
            ReplLine::Turn(text) => self.submit_and_wait(&text).await?,
        }
        Ok(Flow::Continue)
    }

    async fn vote_cancel(&mut self, ask_id: Option<u64>) -> Result<(), CliError> {
        let Some(target) = ask_id.or_else(|| match self.open_asks.as_slice() {
            [only] => Some(*only),
            [] => {
                println!("no open ask to cancel");
                None
            }
            many => {
                println!(
                    "multiple open asks ({}) — answer one at its y/s/n prompt or pass an id: \
                     /vote-cancel <ask-id>",
                    many.len()
                );
                None
            }
        }) else {
            return Ok(());
        };
        self.transport.permission_cancel(target).await
    }

    async fn mode_command(&mut self, name: &str) -> Result<(), CliError> {
        match grammar::mode_on(name) {
            Some(on) => self.transport.plan_mode(on).await,
            None => {
                println!("{}", grammar::mode_note(name));
                Ok(())
            }
        }
    }

    fn attach(&mut self, path: &str) {
        match image::attachments(&[path.into()], false) {
            Ok(found) => {
                self.pending.extend(found);
                println!("image attached (applies to the next submit)");
            }
            Err(error) => println!("{error}"),
        }
    }

    async fn submit_and_wait(&mut self, text: &str) -> Result<(), CliError> {
        if let Some(dir) = self.session_dir {
            let _ = image::store_into(dir, &self.pending);
        }
        let submit = image::submit_text(text, &self.pending);
        self.transport.submit(&submit).await?;
        self.pending.clear();
        let outcome = wait_turn(
            self.transport,
            TurnWait {
                json: false,
                stdin: Some(&mut self.stdin),
                deadline: Instant::now() + REPL_TURN_TIMEOUT,
                open_asks: &mut self.open_asks,
            },
        )
        .await;
        match outcome {
            Ok((_, outcome)) => println!("turn outcome: {outcome}"),
            Err(error) => println!("{error}"),
        }
        Ok(())
    }
}
