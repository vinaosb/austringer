//! End-to-end against the REAL daemon (todo 35 crate consumed
//! read-only as a client): a full session driven purely by key
//! scripts through the App + client task, typed failure channels
//! (bad token, absent daemon), and the fresh-attach resync path.

mod common;

use std::time::Duration;

use austringer_tui::app::{App, Conn, UiAction, UiEvent};
use austringer_tui::client::ClientCmd;
use austringer_tui::keys::Key;
use common::{pump, screen, spawn_daemon, wait_until, wire};

fn plain(code: crossterm::event::KeyCode) -> Key {
    Key {
        code,
        mods: crossterm::event::KeyModifiers::NONE,
    }
}

fn type_and_submit(app: &mut App, text: &str) -> Vec<UiAction> {
    let mut actions = Vec::new();
    actions.extend(app.step(UiEvent::Key(plain(crossterm::event::KeyCode::Char('i')))));
    for c in text.chars() {
        actions.extend(app.step(UiEvent::Key(plain(crossterm::event::KeyCode::Char(c)))));
    }
    actions.extend(app.step(UiEvent::Key(plain(crossterm::event::KeyCode::Enter))));
    actions
}

async fn dispatch(wired: &common::Wired, actions: Vec<UiAction>) {
    for action in actions {
        let cmd = match action {
            UiAction::Send(msg) => ClientCmd::Send(msg),
            UiAction::Reconnect => ClientCmd::Reconnect,
            UiAction::Shutdown => ClientCmd::Shutdown,
        };
        let _ = wired.cmd.send(cmd).await;
    }
}

/// HAPPY channel: attach -> live events -> permission answered ->
/// turn observed; asserted on the RENDERED STATE, not absence of
/// errors (misleading_success guard).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn full_session_driven_by_key_scripts() {
    let Ok(daemon) = spawn_daemon().await else {
        panic!("daemon spawn failed")
    };
    let wired = wire(&daemon.base, "qa-happy", &daemon.token);
    let mut app = App::new(austringer_tui::app::DEFAULT_CONTEXT_WINDOW);
    assert!(
        wait_until(
            &mut app,
            &wired,
            |a| matches!(a.conn, Conn::Attached),
            Duration::from_secs(5)
        ),
        "attach; conn {:?}",
        app.conn
    );

    dispatch(&wired, type_and_submit(&mut app, "run the echo tool")).await;
    assert!(
        wait_until(
            &mut app,
            &wired,
            |a| a.gate().is_some(),
            Duration::from_secs(10)
        ),
        "permission ask surfaced; entries {:?}",
        app.view.entries()
    );

    let answer = app.step(UiEvent::Key(plain(crossterm::event::KeyCode::Char('y'))));
    dispatch(&wired, answer).await;
    assert!(
        wait_until(
            &mut app,
            &wired,
            |a| a.view.turns().last().is_some_and(|t| t.outcome.is_some()),
            Duration::from_secs(10)
        ),
        "turn finished; turns {:?}",
        app.view.turns()
    );

    assert!(app.gate().is_none(), "gate closed after resolution");
    assert!(
        app.view.entries().iter().any(
            |e| matches!(e, austringer_tui::transcript::Entry::User(t) if t == "run the echo tool")
        ),
        "user turn recorded"
    );
    assert!(
        app.view.entries().iter().any(
            |e| matches!(e, austringer_tui::transcript::Entry::Assistant(t) if t.contains("done"))
        ),
        "assistant completion recorded"
    );
    assert!(
        app.view.tools().iter().any(|t| t.result.is_some()),
        "tool call paired with its result"
    );
    assert!(
        app.view
            .notes()
            .iter()
            .any(|n| n.contains("allowed by tui-test")),
        "resolution note: {:?}",
        app.view.notes()
    );

    let rendered = {
        app.step(UiEvent::Key(austringer_tui::keys::Key {
            code: crossterm::event::KeyCode::Char('1'),
            mods: crossterm::event::KeyModifiers::CONTROL,
        }));
        screen(&app, 100, 30)
    };
    assert!(
        rendered.contains("you: run the echo tool"),
        "rendered:\n{rendered}"
    );
    assert!(rendered.contains("ctx:"), "HUD rendered");
    let _ = pump(&mut app, &wired);
    let _ = wired.cmd.send(ClientCmd::Shutdown).await;
}

/// FAILURE channel: wrong token -> typed refusal.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auth_failure_is_typed_refused() {
    let Ok(daemon) = spawn_daemon().await else {
        panic!("daemon spawn failed")
    };
    let wired = wire(&daemon.base, "qa-auth", "wrong-token");
    let mut app = App::new(austringer_tui::app::DEFAULT_CONTEXT_WINDOW);
    assert!(
        wait_until(
            &mut app,
            &wired,
            |a| matches!(a.conn, Conn::Failed(ref err) if err.contains("refused")),
            Duration::from_secs(5)
        ),
        "typed refused failure, conn {:?}",
        app.conn
    );
    let _ = wired.cmd.send(ClientCmd::Shutdown).await;
}

/// Task 19: the attach negotiates protocol version 2. The daemon
/// echoes the CLIENT's proposed Hello version in HelloAck, so this
/// fails the moment a frontend proposes 1 (v1 wires would silently
/// lose the usage/fallback/verdict surface).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn attach_negotiates_protocol_version_two() {
    let Ok(daemon) = spawn_daemon().await else {
        panic!("daemon spawn failed")
    };
    let wired = wire(&daemon.base, "qa-version", &daemon.token);
    let mut app = App::new(austringer_tui::app::DEFAULT_CONTEXT_WINDOW);
    assert!(
        wait_until(
            &mut app,
            &wired,
            |a| matches!(a.conn, Conn::Attached) && a.negotiated_version().is_some(),
            Duration::from_secs(5)
        ),
        "attached; conn {:?} version {:?}",
        app.conn,
        app.negotiated_version()
    );
    assert_eq!(
        app.negotiated_version(),
        Some(austringer_daemon::protocol::PROTOCOL_VERSION),
        "negotiated wire version must be 2"
    );
    let _ = wired.cmd.send(ClientCmd::Shutdown).await;
}

/// FAILURE + stale_state channel: a fresh client attaching to a
/// session with history gets the resync marker + snapshot and
/// rebuilds identical state (reconnect-and-replay path).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fresh_attach_rebuilds_via_resync_snapshot() {
    let Ok(daemon) = spawn_daemon().await else {
        panic!("daemon spawn failed")
    };
    let first = wire(&daemon.base, "qa-resync", &daemon.token);
    let mut app_a = App::new(austringer_tui::app::DEFAULT_CONTEXT_WINDOW);
    assert!(
        wait_until(
            &mut app_a,
            &first,
            |a| matches!(a.conn, Conn::Attached),
            Duration::from_secs(5)
        ),
        "first client attached"
    );
    dispatch(&first, type_and_submit(&mut app_a, "prime the session")).await;
    assert!(
        wait_until(
            &mut app_a,
            &first,
            |a| a.gate().is_some(),
            Duration::from_secs(10)
        ),
        "ask arrived"
    );
    let answer = app_a.step(UiEvent::Key(plain(crossterm::event::KeyCode::Char('y'))));
    dispatch(&first, answer).await;
    assert!(
        wait_until(
            &mut app_a,
            &first,
            |a| a.view.turns().last().is_some_and(|t| t.outcome.is_some()),
            Duration::from_secs(10)
        ),
        "priming turn finished"
    );
    let expected_entries = app_a.view.entries().len();
    let expected_last = app_a.view.last_event_id;
    let _ = first.cmd.send(ClientCmd::Shutdown).await;

    let second = wire(&daemon.base, "qa-resync", &daemon.token);
    let mut app_b = App::new(austringer_tui::app::DEFAULT_CONTEXT_WINDOW);
    assert!(
        wait_until(
            &mut app_b,
            &second,
            // The transport order is resync marker, snapshot, then
            // HelloAck — the view fills before the attach notice, so
            // the predicate must wait for both.
            |a| matches!(a.conn, Conn::Attached)
                && !a.view.resync_pending
                && a.view.entries().len() == expected_entries
                && a.view.last_event_id == expected_last,
            Duration::from_secs(10)
        ),
        "snapshot rebuilt identical state: A entries {expected_entries} last {expected_last}, B entries {} last {}",
        app_b.view.entries().len(),
        app_b.view.last_event_id
    );
    let _ = second.cmd.send(ClientCmd::Shutdown).await;
}
