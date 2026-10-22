//! Task-19 acceptance: the hunk panel renders and is decideable, the
//! HUD goes live from Usage frames, scroll bindings move the body
//! window, and fallback/plan asks are modal and answerable. Frames
//! that need provider types are built through the real wire decode
//! (no provider dep is added to the TUI).

mod common;

use common::screen;
use harness_daemon::protocol::{
    ClientMsg, Frame, PlanAskPayload, PlanDecision, PlanOutcome, ServerMsg, SessionEvent,
    decode_server,
};
use harness_tui::app::{App, UiAction, UiEvent};
use harness_tui::diffs::HunkDecision;
use harness_tui::keys::Key;

fn plain(c: char) -> UiEvent {
    UiEvent::Key(Key {
        code: crossterm::event::KeyCode::Char(c),
        mods: crossterm::event::KeyModifiers::NONE,
    })
}

fn ctrl(c: char) -> UiEvent {
    UiEvent::Key(Key {
        code: crossterm::event::KeyCode::Char(c),
        mods: crossterm::event::KeyModifiers::CONTROL,
    })
}

fn arrow(code: crossterm::event::KeyCode) -> UiEvent {
    UiEvent::Key(Key {
        code,
        mods: crossterm::event::KeyModifiers::NONE,
    })
}

fn diff_msg(id: u64, path: &str, hunks: &[&str]) -> ServerMsg {
    ServerMsg::Event {
        frame: Frame {
            id,
            event: SessionEvent::Diff {
                path: path.to_owned(),
                before_hash: String::new(),
                after_hash: "abc".to_owned(),
                hunks: hunks.iter().map(|h| (*h).to_owned()).collect(),
            },
        },
    }
}

fn user_msg(id: u64, text: &str) -> ServerMsg {
    ServerMsg::Event {
        frame: Frame {
            id,
            event: SessionEvent::Recorded {
                seq: id,
                event: harness_core::event::Event::User {
                    blocks: vec![harness_core::event::ContentBlock::Text {
                        text: text.to_owned(),
                    }],
                },
            },
        },
    }
}

/// Build one ServerMsg through the wire decoder (fixture frames that
/// carry provider-owned types without a provider dependency).
fn decoded(json: &str) -> ServerMsg {
    decode_server(json.as_bytes()).expect("fixture frame decodes")
}

const HUNK_A: &str = "@@ -1,2 +1,3 @@\n context\n-removed line\n+added line\n+second add";
const HUNK_B: &str = "@@ -10 +10 @@\n-sole removal";

#[test]
fn diff_event_renders_hunk_lines_and_diffstats() {
    // Given: one Diff frame and the diffs accordion open (ctrl+5).
    let mut app = App::new(harness_tui::app::DEFAULT_CONTEXT_WINDOW);
    app.step(UiEvent::Server(diff_msg(1, "src/lib.rs", &[HUNK_A])));
    app.step(ctrl('5'));
    // Then: the panel renders the path, the diffstats, and hunk body
    // lines (README hunk-review claim made true).
    let rendered = screen(&app, 100, 30);
    assert!(rendered.contains("diffs (1)"), "rendered:\n{rendered}");
    assert!(rendered.contains("src/lib.rs"), "rendered:\n{rendered}");
    assert!(rendered.contains("+2/-1"), "rendered:\n{rendered}");
    assert!(rendered.contains("+added line"), "rendered:\n{rendered}");
}

#[test]
fn accept_and_reject_decide_pending_hunks_in_order() {
    // Given: a two-hunk diff with the panel open.
    let mut app = App::new(harness_tui::app::DEFAULT_CONTEXT_WINDOW);
    app.step(UiEvent::Server(diff_msg(
        1,
        "src/lib.rs",
        &[HUNK_A, HUNK_B],
    )));
    app.step(ctrl('5'));
    // When: y accepts — then the FIRST pending hunk is decided.
    let actions = app.step(plain('y'));
    assert!(
        actions.is_empty(),
        "hunk decisions are local view state (no wire surface)"
    );
    assert_eq!(
        app.view.diffs()[0].hunks[0].decision,
        HunkDecision::Accepted
    );
    assert_eq!(app.view.diffs()[0].hunks[1].decision, HunkDecision::Pending);
    // When: n rejects — the NEXT pending hunk is decided.
    app.step(plain('n'));
    assert_eq!(
        app.view.diffs()[0].hunks[1].decision,
        HunkDecision::Rejected
    );
    // And: with nothing pending, y changes no state.
    app.step(plain('y'));
    assert_eq!(
        app.view.diffs()[0].hunks[0].decision,
        HunkDecision::Accepted
    );
    // And: decisions render and are noted for the transcript record.
    let rendered = screen(&app, 100, 30);
    assert!(rendered.contains("[accepted]"), "rendered:\n{rendered}");
    assert!(rendered.contains("[rejected]"), "rendered:\n{rendered}");
    assert!(
        app.view
            .notes()
            .iter()
            .any(|n| n.contains("accepted") && n.contains("src/lib.rs")),
        "notes: {:?}",
        app.view.notes()
    );
}

#[test]
fn unparseable_hunk_renders_placeholder_not_crash() {
    // FAILURE channel: a hunk without a unified-diff header renders a
    // placeholder instead of being parsed (or panicking).
    let mut app = App::new(harness_tui::app::DEFAULT_CONTEXT_WINDOW);
    app.step(UiEvent::Server(diff_msg(
        1,
        "bad.rs",
        &["not a hunk at all"],
    )));
    app.step(ctrl('5'));
    let rendered = screen(&app, 100, 30);
    assert!(
        rendered.contains("(unparseable hunk)"),
        "rendered:\n{rendered}"
    );
    assert!(rendered.contains("bad.rs"), "rendered:\n{rendered}");
}

#[test]
fn usage_frame_feeds_hud_cost_and_cache() {
    // Given: a task-1 Usage frame (900 of 1000 prompt tokens served
    // from cache, $0.25 in micro-USD).
    let mut app = App::new(harness_tui::app::DEFAULT_CONTEXT_WINDOW);
    let usage = decoded(
        r#"{"type":"usage","turn":1,"model":{"provider":"openai","model":"gpt-sol"},"effort":null,
            "input_tokens":1000,"output_tokens":40,"cache_read":900,"cache_write":0,"cost_usd_micros":250000}"#,
    );
    // When: it arrives.
    app.step(UiEvent::Server(usage));
    // Then: cache and cost go live — no honest-unknown `n/a` remains.
    assert_eq!(app.hud.cache_line(), "cache: 90.0%");
    let cost = app.hud.cost_line();
    assert!(cost.contains("gpt-sol $0.25"), "cost line: {cost}");
    assert!(!cost.contains("unpriced"), "cost line: {cost}");
    let rendered = screen(&app, 100, 30);
    assert!(rendered.contains("cache: 90.0%"), "rendered:\n{rendered}");
    assert!(rendered.contains("gpt-sol $0.25"), "rendered:\n{rendered}");
}

#[test]
fn scroll_keys_move_the_body_window_and_refollow_on_new_events() {
    // Given: a 40-entry transcript with the accordion open.
    let mut app = App::new(harness_tui::app::DEFAULT_CONTEXT_WINDOW);
    for i in 0..40u64 {
        app.step(UiEvent::Server(user_msg(i, &format!("line-{i:02}"))));
    }
    app.step(ctrl('1'));
    let tail = screen(&app, 80, 24);
    assert!(tail.contains("you: line-39"), "tail:\n{tail}");
    assert!(!tail.contains("you: line-10"), "tail:\n{tail}");
    // When: ScrollUp five times — the window lifts off the tail (the
    // body also carries the three collapsed accordion headers below
    // the transcript, so the window spans line-18..line-37).
    for _ in 0..5 {
        app.step(arrow(crossterm::event::KeyCode::Up));
    }
    let scrolled = screen(&app, 80, 24);
    assert!(scrolled.contains("you: line-18"), "scrolled:\n{scrolled}");
    assert!(!scrolled.contains("you: line-39"), "scrolled:\n{scrolled}");
    // When: ScrollDown five times — the tail returns.
    for _ in 0..5 {
        app.step(arrow(crossterm::event::KeyCode::Down));
    }
    let back = screen(&app, 80, 24);
    assert!(back.contains("you: line-39"), "back:\n{back}");
    // And: a new frame resets the offset (auto-follow).
    for _ in 0..3 {
        app.step(arrow(crossterm::event::KeyCode::Up));
    }
    app.step(UiEvent::Server(user_msg(40, "line-40")));
    let followed = screen(&app, 80, 24);
    assert!(followed.contains("you: line-40"), "followed:\n{followed}");
}

#[test]
fn fallback_ask_is_modal_and_answerable() {
    // Given: a FallbackAsk broadcast.
    let mut app = App::new(harness_tui::app::DEFAULT_CONTEXT_WINDOW);
    let ask = decoded(
        r#"{"type":"fallback_ask","ask":{"ask_id":5,"current":{"provider":"openai","model":"model-a"},
            "proposed":{"provider":"openai","model":"model-b"},"reason":"rate limited","deadline_ms":30000}}"#,
    );
    app.step(UiEvent::Server(ask));
    // Then: the ask is modal and prominent (permission-gate pattern).
    assert!(app.fallback_gate().is_some());
    let rendered = screen(&app, 100, 30);
    assert!(
        rendered.contains("fallback proposed: model-a -> model-b"),
        "rendered:\n{rendered}"
    );
    assert!(rendered.contains("rate limited"), "rendered:\n{rendered}");
    // And: typing is swallowed while the ask is open.
    app.step(plain('i'));
    assert!(!app.bar_focused());
    // When: y answers — the hop goes to the PROPOSED identity.
    let actions = app.step(plain('y'));
    match actions.as_slice() {
        [UiAction::Send(ClientMsg::FallbackAnswer { ask, chosen })] => {
            assert_eq!(*ask, 5);
            assert_eq!(chosen.model, "model-b");
        }
        other => panic!("expected a fallback answer, got {other:?}"),
    }
    // And: the resolution closes the modal.
    let resolved = decoded(
        r#"{"type":"fallback_resolved","ask_id":5,"chosen":{"provider":"openai","model":"model-b"}}"#,
    );
    app.step(UiEvent::Server(resolved));
    assert!(app.fallback_gate().is_none());
}

#[test]
fn fallback_deny_keeps_the_current_model() {
    // Given: the same ask; When: n answers.
    let mut app = App::new(harness_tui::app::DEFAULT_CONTEXT_WINDOW);
    let ask = decoded(
        r#"{"type":"fallback_ask","ask":{"ask_id":6,"current":{"provider":"openai","model":"model-a"},
            "proposed":{"provider":"openai","model":"model-b"},"reason":"rate limited","deadline_ms":30000}}"#,
    );
    app.step(UiEvent::Server(ask));
    let actions = app.step(plain('n'));
    // Then: the answer pins the CURRENT identity (never a silent hop).
    match actions.as_slice() {
        [UiAction::Send(ClientMsg::FallbackAnswer { ask, chosen })] => {
            assert_eq!(*ask, 6);
            assert_eq!(chosen.model, "model-a");
        }
        other => panic!("expected a fallback answer, got {other:?}"),
    }
}

#[test]
fn plan_ask_is_modal_and_answerable() {
    // Given: a PlanAsk broadcast (task 16).
    let mut app = App::new(harness_tui::app::DEFAULT_CONTEXT_WINDOW);
    app.step(UiEvent::Server(ServerMsg::PlanAsk {
        ask: PlanAskPayload {
            ask_id: 4,
            plan_id: 1,
            revision: 2,
            artifact: "# Plan\n- edit the parser".into(),
            deadline_ms: 30_000,
        },
    }));
    // Then: the ask is modal with an artifact preview.
    assert!(app.plan_gate().is_some());
    let rendered = screen(&app, 100, 30);
    assert!(
        rendered.contains("plan 1 (revision 2) awaiting approval"),
        "rendered:\n{rendered}"
    );
    assert!(rendered.contains("# Plan"), "rendered:\n{rendered}");
    app.step(plain('i'));
    assert!(!app.bar_focused(), "plan modal swallows typing");
    // When: y approves.
    let actions = app.step(plain('y'));
    assert_eq!(
        actions,
        vec![UiAction::Send(ClientMsg::PlanAnswer {
            ask: 4,
            decision: PlanDecision::Approve
        })]
    );
    // And: the resolution closes the modal.
    app.step(UiEvent::Server(ServerMsg::PlanResolved {
        ask_id: 4,
        plan_id: 1,
        outcome: PlanOutcome::Approved {
            by: "tui-test".into(),
        },
    }));
    assert!(app.plan_gate().is_none());
}

#[test]
fn plan_reject_keeps_plan_mode_active() {
    // Given: the same ask; When: n rejects.
    let mut app = App::new(harness_tui::app::DEFAULT_CONTEXT_WINDOW);
    app.step(UiEvent::Server(ServerMsg::PlanAsk {
        ask: PlanAskPayload {
            ask_id: 7,
            plan_id: 2,
            revision: 1,
            artifact: String::new(),
            deadline_ms: 30_000,
        },
    }));
    let actions = app.step(plain('n'));
    // Then: the decision is Reject (plan mode stays; writes gated).
    assert_eq!(
        actions,
        vec![UiAction::Send(ClientMsg::PlanAnswer {
            ask: 7,
            decision: PlanDecision::Reject
        })]
    );
}
