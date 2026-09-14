//! Interactive REPL fallback (plan todo 39): colored prompt, streaming
//! turns, `/exit`, `/cancel`, `/image <path>` mid-session attachment,
//! and permission asks answered interactively. Lines that are not
//! commands submit a turn and stream it to completion before the next
//! prompt (matching the headless run loop exactly).

use std::path::Path;
use std::time::{Duration, Instant};

use crate::client::SessionTransport;
use crate::error::CliError;
use crate::image::{self, PreparedImage};
use crate::run::{replay_until_live, wait_turn};

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
    let mut pending: Vec<PreparedImage> = images.to_vec();
    replay_until_live(transport, false).await?;
    println!("harness repl — /exit quits, /cancel steers, /image <path> attaches");
    let mut stdin = crate::run::stdin_lines();
    while let Some(line) = stdin.recv().await {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match line {
            "/exit" => return Ok(0),
            "/cancel" => {
                transport.cancel().await?;
                continue;
            }
            _ => {}
        }
        if let Some(path) = line.strip_prefix("/image ") {
            match image::attachments(&[path.into()], false) {
                Ok(found) => {
                    pending.extend(found);
                    println!("image attached (applies to the next submit)");
                }
                Err(error) => println!("{error}"),
            }
            continue;
        }
        if let Some(dir) = session_dir {
            let _ = image::store_into(dir, &pending);
        }
        transport
            .submit(&image::submit_text(line, &pending))
            .await?;
        pending.clear();
        match wait_turn(transport, false, Some(&mut stdin), deadline()).await {
            Ok((_, outcome)) => println!("turn outcome: {outcome}"),
            Err(error) => println!("{error}"),
        }
    }
    Ok(0)
}

fn deadline() -> Instant {
    Instant::now() + REPL_TURN_TIMEOUT
}
