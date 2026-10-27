//! The austringer TUI binary: a thin wiring layer. A crossterm raw-mode
//! input thread on one side, the daemon WebSocket client task on the
//! other, and the synchronous event-driven loop in between (the loop
//! renders only when the app reports a state change; there is no
//! timer anywhere on this thread).

use std::sync::mpsc::{Receiver, Sender};

use austringer_tui::app::{App, UiAction, UiEvent};
use austringer_tui::client::{ClientCmd, ClientEvent};
use austringer_tui::complete::FileCompletions;
use austringer_tui::keys::Key;
use clap::Parser as _;
use crossterm::event::{Event as TermEvent, KeyEvent};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

#[derive(Debug, clap::Parser)]
#[command(
    name = "austringer-tui",
    version,
    about = "Terminal client over the austringer daemon"
)]
struct Args {
    /// Daemon base URL; omit to attach to (or spawn) the local default
    /// daemon.
    #[arg(long)]
    url: Option<String>,
    /// Session id to bind.
    #[arg(long, default_value = "default")]
    session: String,
    /// Daemon auth token file (default: {data-dir}/daemon-token).
    #[arg(long = "token-file")]
    token_file: Option<std::path::PathBuf>,
    /// Daemon data dir (default .austringer-daemon).
    #[arg(long = "data-dir")]
    data_dir: Option<std::path::PathBuf>,
    /// IPC socket name used when spawning (default austringer-daemon).
    #[arg(long = "ipc-name")]
    ipc_name: Option<String>,
}

/// The wired connection the client task consumes.
struct Connection {
    url: String,
    session: String,
    token_file: std::path::PathBuf,
}

fn data_dir_of(args: &Args) -> std::path::PathBuf {
    args.data_dir
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from(austringer_launch::DEFAULT_DATA_DIR))
}

/// Explicit `--url` passes straight through; without it the TUI uses
/// the running daemon's `daemon-url` file, probing it first, and
/// auto-spawns one (visible notice) when nothing answers — the same
/// just-starts posture as the CLI.
async fn resolve_connection(args: Args) -> Result<Connection, String> {
    let data_dir = data_dir_of(&args);
    let token_file = args
        .token_file
        .clone()
        .unwrap_or_else(|| data_dir.join(austringer_launch::TOKEN_FILENAME));
    let url = match args.url {
        Some(url) => url,
        None => match live_daemon_url(&data_dir).await {
            Some(url) => url,
            None => {
                let ipc_name = args
                    .ipc_name
                    .clone()
                    .unwrap_or_else(|| austringer_launch::DEFAULT_IPC_NAME.to_owned());
                austringer_launch::ensure_daemon(&data_dir, &ipc_name)
                    .await
                    .map_err(|failure| failure.to_string())?;
                austringer_launch::read_daemon_url(&data_dir)
                    .ok_or_else(|| "daemon became ready but wrote no daemon-url".to_owned())?
            }
        },
    };
    Ok(Connection {
        url,
        session: args.session,
        token_file,
    })
}

async fn live_daemon_url(data_dir: &std::path::Path) -> Option<String> {
    let url = austringer_launch::read_daemon_url(data_dir)?;
    let addr = url.trim_start_matches("http://").to_owned();
    let probe = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        tokio::net::TcpStream::connect(&addr),
    )
    .await
    .ok()?
    .ok()?;
    drop(probe);
    Some(url)
}

fn to_ui_event(event: TermEvent) -> Option<UiEvent> {
    match event {
        TermEvent::Key(KeyEvent {
            code,
            modifiers,
            kind: crossterm::event::KeyEventKind::Press,
            ..
        }) => Some(UiEvent::Key(Key {
            code,
            mods: modifiers,
        })),
        TermEvent::Resize(_, _) => Some(UiEvent::Resize),
        _ => None,
    }
}

/// Reader thread: blocks in crossterm `read`; a read error (EOF or
/// terminal gone) becomes [`UiEvent::Eof`].
fn spawn_input_thread(sender: Sender<UiEvent>) {
    std::thread::spawn(move || {
        loop {
            match crossterm::event::read() {
                Ok(event) => {
                    let Some(ui) = to_ui_event(event) else {
                        continue;
                    };
                    if sender.send(ui).is_err() {
                        return;
                    }
                }
                Err(_) => {
                    let _ = sender.send(UiEvent::Eof);
                    return;
                }
            }
        }
    });
}

/// Bridge thread: client-task events -> the merged UI channel.
fn spawn_bridge_thread(receiver: Receiver<ClientEvent>, sender: Sender<UiEvent>) {
    std::thread::spawn(move || {
        while let Ok(event) = receiver.recv() {
            if sender.send(UiEvent::from(event)).is_err() {
                return;
            }
        }
        let _ = sender.send(UiEvent::Disconnected);
    });
}

async fn client_task(
    connection: Connection,
    events: Sender<ClientEvent>,
    cmd: tokio::sync::mpsc::Receiver<ClientCmd>,
) {
    let token = std::fs::read_to_string(&connection.token_file)
        .unwrap_or_default()
        .trim()
        .to_owned();
    austringer_tui::client::run(
        &connection.url,
        &connection.session,
        &token,
        "tui",
        events,
        cmd,
    )
    .await;
}

fn main() {
    let args = Args::parse();
    // Multi-thread: the UI thread parks in recv() for long stretches,
    // so the client task needs its own runtime worker to keep the
    // socket pumping (pings, live events).
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("austringer-tui: runtime init failed: {error}");
            std::process::exit(1);
        }
    };
    let connection = match runtime.block_on(resolve_connection(args)) {
        Ok(connection) => connection,
        Err(failure) => {
            eprintln!("austringer-tui: {failure}");
            std::process::exit(1);
        }
    };

    let (ui_tx, ui_rx) = std::sync::mpsc::channel::<UiEvent>();
    let (client_tx, client_rx) = std::sync::mpsc::channel::<ClientEvent>();
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<ClientCmd>(16);
    runtime.spawn(client_task(connection, client_tx, cmd_rx));
    spawn_input_thread(ui_tx.clone());
    spawn_bridge_thread(client_rx, ui_tx);

    let mut app = App::new(austringer_tui::app::DEFAULT_CONTEXT_WINDOW);
    app.set_completions(Box::new(FileCompletions::new(std::path::Path::new("."))));

    if enable_raw_mode().is_err()
        || crossterm::execute!(std::io::stdout(), EnterAlternateScreen).is_err()
    {
        eprintln!("austringer-tui: terminal setup failed (is stdout a tty?)");
        std::process::exit(1);
    }
    let backend = ratatui::backend::CrosstermBackend::new(std::io::stdout());
    let mut terminal = match ratatui::Terminal::new(backend) {
        Ok(terminal) => terminal,
        Err(error) => {
            let _ = disable_raw_mode();
            eprintln!("austringer-tui: terminal init failed: {error}");
            std::process::exit(1);
        }
    };

    let dispatch = |action: UiAction| {
        let command = match action {
            UiAction::Send(msg) => ClientCmd::Send(msg),
            UiAction::Reconnect => ClientCmd::Reconnect,
            UiAction::Shutdown => ClientCmd::Shutdown,
        };
        let _ = runtime.block_on(async { cmd_tx.send(command).await });
    };

    app.step(UiEvent::Resize);
    'ui: while !app.quit() {
        if app.should_redraw() {
            if terminal
                .draw(|frame| austringer_tui::render::render(frame, &app))
                .is_err()
            {
                break 'ui;
            }
            app.rendered();
        }
        match ui_rx.recv() {
            Ok(event) => {
                for action in app.step(event) {
                    dispatch(action);
                }
                while let Ok(batched) = ui_rx.try_recv() {
                    for action in app.step(batched) {
                        dispatch(action);
                    }
                }
            }
            Err(_) => break 'ui,
        }
    }

    let _ = cmd_tx.blocking_send(ClientCmd::Shutdown);
    let _ = disable_raw_mode();
    let _ = crossterm::execute!(std::io::stdout(), LeaveAlternateScreen);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_url_and_session_parse() {
        let args = Args::try_parse_from([
            "austringer-tui",
            "--url",
            "http://127.0.0.1:9000",
            "--session",
            "work",
        ])
        .unwrap_or_else(|e| panic!("parse: {e}"));
        assert_eq!(args.url.as_deref(), Some("http://127.0.0.1:9000"));
        assert_eq!(args.session, "work");
        assert!(args.token_file.is_none());
    }

    #[test]
    fn bare_invocation_defaults_to_the_local_daemon_discovery() {
        let args =
            Args::try_parse_from(["austringer-tui"]).unwrap_or_else(|e| panic!("parse: {e}"));
        assert!(args.url.is_none());
        assert_eq!(args.session, "default");
        assert_eq!(
            data_dir_of(&args),
            std::path::PathBuf::from(austringer_launch::DEFAULT_DATA_DIR)
        );
    }

    #[tokio::test]
    async fn explicit_url_skips_daemon_discovery_and_spawn() {
        let connection = resolve_connection(Args {
            url: Some("http://127.0.0.1:9".to_owned()),
            session: "s".to_owned(),
            token_file: None,
            data_dir: Some(std::path::PathBuf::from("nowhere")),
            ipc_name: None,
        })
        .await
        .unwrap_or_else(|e| panic!("resolve: {e}"));
        assert_eq!(connection.url, "http://127.0.0.1:9");
        assert_eq!(connection.session, "s");
        assert_eq!(
            connection.token_file,
            std::path::PathBuf::from("nowhere").join(austringer_launch::TOKEN_FILENAME)
        );
    }
}
