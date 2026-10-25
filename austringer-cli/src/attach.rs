//! `attach` (plan todo 39 / todo 35 replay surface): re-bind to a
//! session and render its event stream — replayed frames first (the
//! detach/reattach contract: missed turns reappear), then live events.
//! Attach is an observer: it renders permission asks and resolutions
//! but does not answer them (that is run/repl's job). `--max-events`
//! bounds the render for scripted use.

use austringer_daemon::protocol::ServerMsg;

use crate::client::SessionTransport;
use crate::error::CliError;
use crate::run::print_msg;

/// Stream a session's events. Returns the process exit code.
///
/// # Errors
/// typed transport failures.
pub async fn attach(
    transport: &mut SessionTransport,
    json: bool,
    max_events: Option<u64>,
) -> Result<i32, CliError> {
    let mut seen: u64 = 0;
    loop {
        if max_events.is_some_and(|max| seen >= max) {
            return Ok(0);
        }
        let Some(msg) = transport.next_msg().await? else {
            return Ok(0);
        };
        match &msg {
            ServerMsg::HelloAck { .. } | ServerMsg::Pong => continue,
            // IPC attach arrives cursor-less: the daemon answers with a
            // full snapshot whose frames ARE the replayed history.
            ServerMsg::Snapshot { frames } => {
                for frame in frames {
                    print_msg(
                        &ServerMsg::Event {
                            frame: frame.clone(),
                        },
                        json,
                    );
                    seen += 1;
                    if max_events.is_some_and(|max| seen >= max) {
                        return Ok(0);
                    }
                }
            }
            ServerMsg::Event { .. } => {
                seen += 1;
                print_msg(&msg, json);
            }
            ServerMsg::Bye { .. } => {
                print_msg(&msg, json);
                return Ok(0);
            }
            other => print_msg(other, json),
        }
    }
}
