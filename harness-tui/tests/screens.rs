//! insta snapshot suite for the main screens (plan acceptance) plus
//! content assertions so a stub renderer cannot pass silently.

mod common;

use common::screen;
use harness_core::event::{ContentBlock, Event};
use harness_daemon::protocol::{AskPayload, Frame, ServerMsg, SessionEvent};
use harness_tui::app::{App, UiEvent};
use harness_tui::hud::HudUpdate;
use harness_tui::omnibar::CompletionSource;

struct Fixture;

impl CompletionSource for Fixture {
    fn files(&self, query: &str) -> Vec<String> {
        ["src/lib.rs", "src/main.rs", "README.md"]
            .iter()
            .filter(|f| f.contains(query))
            .map(|s| (*s).to_owned())
            .collect()
    }
    fn symbols(&self, _query: &str) -> Vec<String> {
        Vec::new()
    }
    fn git_refs(&self, _query: &str) -> Vec<String> {
        Vec::new()
    }
}

fn populated_app() -> App {
    let mut app = App::new(10_000);
    let recorded = |id: u64, event: Event| ServerMsg::Event {
        frame: Frame {
            id,
            event: SessionEvent::Recorded { seq: id, event },
        },
    };
    app.step(UiEvent::Server(recorded(
        1,
        Event::User {
            blocks: vec![ContentBlock::Text {
                text: "ship the fix".into(),
            }],
        },
    )));
    app.step(UiEvent::Server(recorded(
        2,
        Event::Assistant {
            blocks: vec![ContentBlock::Text {
                text: "# Plan\n- edit the parser\n- add a test\n```rust\nfn x() {}\n```".into(),
            }],
        },
    )));
    app.step(UiEvent::Server(ServerMsg::Event {
        frame: Frame {
            id: 3,
            event: SessionEvent::WorkerClaimed {
                queue: "build".into(),
                worker: "w1".into(),
            },
        },
    }));
    app.hud.apply(&HudUpdate {
        cache_hit_pct: Some(72.5),
        model: "frontier-x".into(),
        cost_delta: 0.42,
        routing_savings: 0.09,
    });
    app
}

#[test]
fn expanded_transcript_tools_and_team_render() {
    let mut app = populated_app();
    app.step(UiEvent::Attached);
    app.step(UiEvent::Key(harness_tui::keys::Key {
        code: crossterm::event::KeyCode::Char('1'),
        mods: crossterm::event::KeyModifiers::CONTROL,
    }));
    app.step(UiEvent::Key(harness_tui::keys::Key {
        code: crossterm::event::KeyCode::Char('2'),
        mods: crossterm::event::KeyModifiers::CONTROL,
    }));
    app.step(UiEvent::Key(harness_tui::keys::Key {
        code: crossterm::event::KeyCode::Char('3'),
        mods: crossterm::event::KeyModifiers::CONTROL,
    }));
    let rendered = screen(&app, 80, 24);
    assert!(rendered.contains("you: ship the fix"), "{rendered}");
    assert!(rendered.contains("Plan"), "{rendered}");
    assert!(rendered.contains("build"), "{rendered}");
    assert!(rendered.contains("cache: 72.5%"), "{rendered}");
    insta::assert_snapshot!(rendered);
}

#[test]
fn permission_gate_is_modal_and_prominent() {
    let mut app = populated_app();
    app.step(UiEvent::Server(ServerMsg::PermissionAsk {
        ask: AskPayload {
            ask_id: 1,
            tool: "bash".into(),
            target: Some("cargo test".into()),
            reason: "run the suite".into(),
            deadline_ms: 120_000,
        },
    }));
    app.step(UiEvent::Key(harness_tui::keys::Key {
        code: crossterm::event::KeyCode::Char('y'),
        mods: crossterm::event::KeyModifiers::NONE,
    }));
    let rendered = screen(&app, 80, 24);
    assert!(rendered.contains("permission: bash"), "{rendered}");
    assert!(rendered.contains("answered: allow-once"), "{rendered}");
    insta::assert_snapshot!(rendered);
}

#[test]
fn collapsed_default_with_failure_status() {
    let mut app = populated_app();
    app.step(UiEvent::ConnectFailed("daemon unreachable: refused".into()));
    let rendered = screen(&app, 80, 24);
    assert!(rendered.contains("> transcript"), "{rendered}");
    assert!(rendered.contains("> tools"), "{rendered}");
    assert!(rendered.contains("failed:"), "{rendered}");
    insta::assert_snapshot!(rendered);
}

#[test]
fn omnibar_focused_with_completions_popup() {
    let mut app = populated_app();
    app.set_completions(Box::new(Fixture));
    app.step(UiEvent::Key(harness_tui::keys::Key {
        code: crossterm::event::KeyCode::Char('i'),
        mods: crossterm::event::KeyModifiers::NONE,
    }));
    for c in "open @src".chars() {
        app.step(UiEvent::Key(harness_tui::keys::Key {
            code: crossterm::event::KeyCode::Char(c),
            mods: crossterm::event::KeyModifiers::NONE,
        }));
    }
    let rendered = screen(&app, 80, 24);
    assert!(rendered.contains("open @src"), "{rendered}");
    assert!(rendered.contains("src/lib.rs"), "{rendered}");
    assert!(rendered.contains("completions"), "{rendered}");
    insta::assert_snapshot!(rendered);
}

#[test]
fn resolution_and_local_notes_render_dim_rows() {
    let mut app = populated_app();
    app.step(UiEvent::Attached);
    app.step(UiEvent::Server(ServerMsg::PermissionResolved {
        ask_id: 4,
        resolution: harness_daemon::protocol::Resolution::Denied { by: "web".into() },
    }));
    let rendered = screen(&app, 80, 24);
    assert!(rendered.contains("ask 4 denied by web"), "{rendered}");
    assert!(rendered.contains("cost: frontier-x $0.42"), "{rendered}");
    insta::assert_snapshot!(rendered);
}
