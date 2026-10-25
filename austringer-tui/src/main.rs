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
use crossterm::event::{Event as TermEvent, KeyEvent};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

struct Args {
    url: String,
    session: String,
    token_file: std::path::PathBuf,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        url: String::new(),
        session: "default".into(),
        token_file: std::path::PathBuf::from(".austringer-daemon/daemon-token"),
    };
    let mut iter = std::env::args().skip(1);
    while let Some(flag) = iter.next() {
        let value = iter.next();
        match flag.as_str() {
            "--url" => args.url = value.ok_or("--url needs a value")?,
            "--session" => args.session = value.ok_or("--session needs a value")?,
            "--token-file" => {
                args.token_file =
                    std::path::PathBuf::from(value.ok_or("--token-file needs a value")?)
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if args.url.is_empty() {
        return Err("usage: austringer-tui --url http://127.0.0.1:<port> [--session <id>] [--token-file <path>]".into());
    }
    Ok(args)
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
    args: Args,
    events: Sender<ClientEvent>,
    cmd: tokio::sync::mpsc::Receiver<ClientCmd>,
) {
    let token = std::fs::read_to_string(&args.token_file)
        .unwrap_or_default()
        .trim()
        .to_owned();
    austringer_tui::client::run(&args.url, &args.session, &token, "tui", events, cmd).await;
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(usage) => {
            eprintln!("austringer-tui: {usage}");
            std::process::exit(2);
        }
    };
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

    let (ui_tx, ui_rx) = std::sync::mpsc::channel::<UiEvent>();
    let (client_tx, client_rx) = std::sync::mpsc::channel::<ClientEvent>();
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<ClientCmd>(16);
    runtime.spawn(client_task(args, client_tx, cmd_rx));
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
