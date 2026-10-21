//! The headless UI state machine: one input event in, state transition
//! plus outgoing actions out. This is the entire interface logic, and
//! it is fully testable without a terminal — the binary only wires a
//! crossterm reader and the daemon client to [`App::step`].
//!
//! Event-driven by construction: `step` never blocks, never sleeps,
//! and marks the app dirty only when observable state changed. The UI
//! loop renders iff [`App::should_redraw`] — there is no timer, no
//! poll, and therefore 0% idle CPU (pinned by test).

use harness_daemon::protocol::{Answer, AskPayload, ClientMsg, Resolution, ServerMsg};

use crate::hud;
use crate::keys::{Key, Keybinds};
use crate::omnibar::{CompletionSource, NoCompletions, Omnibar};
use crate::transcript;

/// Merged input events: terminal thread and daemon client task.
#[derive(Debug, Clone, PartialEq)]
pub enum UiEvent {
    /// A key press.
    Key(Key),
    /// Terminal resized.
    Resize,
    /// Terminal input closed (EOF / hangup).
    Eof,
    /// The client task completed the handshake.
    Attached,
    /// The client task could not connect (typed reason).
    ConnectFailed(String),
    /// An established connection was lost.
    Disconnected,
    /// A daemon message arrived.
    Server(ServerMsg),
}

/// Work the UI loop must perform outside the app (protocol sends and
/// client control), produced by [`App::step`].
#[derive(Debug, Clone, PartialEq)]
pub enum UiAction {
    /// Send one protocol message to the daemon.
    Send(ClientMsg),
    /// Ask the client task to reconnect now.
    Reconnect,
    /// Stop the client task (loop exit).
    Shutdown,
}

/// Connection state rendered in the status area.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conn {
    /// No client task yet.
    Disconnected,
    /// Handshake in flight.
    Connecting,
    /// HelloAck seen.
    Attached,
    /// Typed failure; manual `ctrl+r` reconnect.
    Failed(String),
}

/// Collapsed-accordion panel visibility (false = collapsed header).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Panels {
    /// Transcript accordion.
    pub transcript: bool,
    /// Tool-call accordion.
    pub tools: bool,
    /// Team-grid accordion.
    pub team: bool,
}

impl Panels {
    /// All collapsed (the plan's default posture).
    #[must_use]
    pub const fn collapsed() -> Self {
        Self {
            transcript: false,
            tools: false,
            team: false,
        }
    }
}

/// Documented context-window default until model profiles (todo 34)
/// supply the real per-model number.
pub const DEFAULT_CONTEXT_WINDOW: u64 = 200_000;

impl From<crate::client::ClientEvent> for UiEvent {
    fn from(event: crate::client::ClientEvent) -> Self {
        use crate::client::ClientEvent;
        match event {
            ClientEvent::Attached { .. } => Self::Attached,
            ClientEvent::Server(msg) => Self::Server(msg),
            ClientEvent::Dropped => Self::Disconnected,
            ClientEvent::Failed(err) => Self::ConnectFailed(err.to_string()),
        }
    }
}

/// The whole interface state.
pub struct App {
    /// Reduced session view (entries, tools, turns, diffs, workers).
    pub view: transcript::SessionView,
    /// HUD aggregates (context gauge, cache, per-model cost).
    pub hud: hud::Hud,
    /// Omnibar input line.
    pub bar: Omnibar,
    pub(crate) binds: Keybinds,
    pub(crate) bar_focused: bool,
    /// Collapsed accordions.
    pub panels: Panels,
    pub(crate) gate: Option<AskPayload>,
    pub(crate) pending: Option<Answer>,
    pub(crate) completions: Box<dyn CompletionSource>,
    /// Connection state for the status area.
    pub conn: Conn,
    /// HUD bar visibility (`ctrl+4`).
    pub show_hud: bool,
    pub(crate) quit: bool,
    pub(crate) dirty: bool,
    renders: u64,
}

impl App {
    /// New app for a session rendered into `context_window` tokens.
    #[must_use]
    pub fn new(context_window: u64) -> Self {
        Self {
            view: transcript::SessionView::new(),
            hud: hud::Hud::new(context_window),
            bar: Omnibar::new(),
            binds: Keybinds::defaults(),
            bar_focused: false,
            panels: Panels::collapsed(),
            gate: None,
            pending: None,
            completions: Box::new(NoCompletions),
            conn: Conn::Disconnected,
            show_hud: true,
            quit: false,
            dirty: true,
            renders: 0,
        }
    }

    /// True when the UI loop should draw before waiting again.
    #[must_use]
    pub const fn should_redraw(&self) -> bool {
        self.dirty
    }

    /// The loop finished one draw; resets the dirty flag. Test surface
    /// for the renders-only-on-change proof.
    pub fn rendered(&mut self) {
        self.dirty = false;
        self.renders += 1;
    }

    /// Draws performed (idle-CPU assertion input).
    #[must_use]
    pub const fn renders(&self) -> u64 {
        self.renders
    }

    /// The user asked to exit.
    #[must_use]
    pub const fn quit(&self) -> bool {
        self.quit
    }

    /// True while the omnibar owns typing keys.
    #[must_use]
    pub const fn bar_focused(&self) -> bool {
        self.bar_focused
    }

    /// The active permission ask, if the gate is open.
    #[must_use]
    pub fn gate(&self) -> Option<&AskPayload> {
        self.gate.as_ref()
    }

    /// Completion candidates for the omnibar's active token.
    #[must_use]
    pub fn completions_for(&self, limit: usize) -> Vec<String> {
        self.bar.completions(self.completions.as_ref(), limit)
    }

    /// Swap the completion source (binary wiring; tests).
    pub fn set_completions(&mut self, source: Box<dyn CompletionSource>) {
        self.completions = source;
    }

    /// True between `turn_started` and `turn_finished`.
    #[must_use]
    pub fn turn_running(&self) -> bool {
        self.view
            .turns()
            .last()
            .is_some_and(|turn| turn.outcome.is_none())
    }

    /// Apply one input event; returns work for the UI loop.
    pub fn step(&mut self, event: UiEvent) -> Vec<UiAction> {
        match event {
            UiEvent::Eof => {
                self.quit = true;
                self.dirty = true;
                vec![UiAction::Shutdown]
            }
            UiEvent::Resize => {
                self.dirty = true;
                Vec::new()
            }
            UiEvent::Attached => {
                if self.conn != Conn::Attached {
                    self.conn = Conn::Attached;
                    self.dirty = true;
                }
                Vec::new()
            }
            UiEvent::ConnectFailed(err) => {
                self.conn = Conn::Failed(err);
                self.dirty = true;
                Vec::new()
            }
            UiEvent::Disconnected => {
                self.conn = Conn::Disconnected;
                self.dirty = true;
                Vec::new()
            }
            UiEvent::Key(key) => self.key(key),
            UiEvent::Server(msg) => self.server(msg),
        }
    }

    fn server(&mut self, msg: ServerMsg) -> Vec<UiAction> {
        match msg {
            ServerMsg::HelloAck { .. } => {
                if self.conn != Conn::Attached {
                    self.conn = Conn::Attached;
                    self.dirty = true;
                }
            }
            ServerMsg::Event { frame } => {
                self.view.apply_frame(&frame);
                self.hud.estimate_context(self.view.context_chars());
                self.dirty = true;
            }
            ServerMsg::PermissionAsk { ask } => {
                self.gate = Some(ask);
                self.pending = None;
                self.dirty = true;
            }
            ServerMsg::PermissionResolved { ask_id, resolution } => {
                if self.gate.as_ref().is_some_and(|ask| ask.ask_id == ask_id) {
                    self.gate = None;
                    self.pending = None;
                }
                self.view.note(&resolution_note(ask_id, &resolution));
                self.dirty = true;
            }
            ServerMsg::StateResyncRequired { .. } | ServerMsg::Snapshot { .. } => {
                self.view.apply_server_msg(&msg);
                self.hud.estimate_context(self.view.context_chars());
                self.dirty = true;
            }
            ServerMsg::Error { code, message } => {
                self.view
                    .note(&format!("daemon error ({code:?}): {message}"));
                self.dirty = true;
            }
            ServerMsg::MediaResult {
                producer,
                diff_summary,
                ..
            } => {
                self.view.note(&format!("media ({producer}): {diff_summary}"));
                self.dirty = true;
            }
            ServerMsg::Usage {
                model,
                input_tokens,
                output_tokens,
                ..
            } => {
                self.view.note(&format!(
                    "model {}: {input_tokens} in / {output_tokens} out",
                    model.model
                ));
                self.dirty = true;
            }
            ServerMsg::FallbackAsk { ask } => {
                self.view.note(&format!(
                    "fallback proposed: {} -> {} ({})",
                    ask.current.model, ask.proposed.model, ask.reason
                ));
                self.dirty = true;
            }
            ServerMsg::FallbackResolved { ask_id, chosen } => {
                self.view.note(&format!("fallback {ask_id} resolved: next model {}", chosen.model));
                self.dirty = true;
            }
            ServerMsg::Verdict {
                turn,
                verdict,
                evidence,
            } => {
                self.view.note(&format!(
                    "turn {turn} verdict: {} ({} evidence events)",
                    verdict.as_str(),
                    evidence.len()
                ));
                self.dirty = true;
            }
            ServerMsg::Bye { reason } => {
                self.view.note(&format!("daemon closed: {reason}"));
                self.dirty = true;
            }
            ServerMsg::Pong => {}
        }
        Vec::new()
    }
}

fn resolution_note(ask_id: u64, resolution: &Resolution) -> String {
    let describe = |verb: &str, by: &str| format!("ask {ask_id} {verb} by {by}");
    match resolution {
        Resolution::Allowed { by } => describe("allowed", by),
        Resolution::Denied { by } => describe("denied", by),
        Resolution::Cancelled { by } => describe("cancelled", by),
        Resolution::Timeout => format!("ask {ask_id} timed out"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::Action;
    use crate::omnibar::{self, Parsed};
    use harness_core::event::{ContentBlock, Event};
    use harness_daemon::protocol::{Frame, SessionCommand, SessionEvent};

    struct NoSrc;
    impl CompletionSource for NoSrc {
        fn files(&self, _: &str) -> Vec<String> {
            Vec::new()
        }
        fn symbols(&self, _: &str) -> Vec<String> {
            Vec::new()
        }
        fn git_refs(&self, _: &str) -> Vec<String> {
            Vec::new()
        }
    }

    fn key(code: crossterm::event::KeyCode) -> Key {
        Key {
            code,
            mods: crossterm::event::KeyModifiers::NONE,
        }
    }

    fn char_key(c: char) -> Key {
        key(crossterm::event::KeyCode::Char(c))
    }

    fn user_msg(id: u64, text: &str) -> ServerMsg {
        ServerMsg::Event {
            frame: Frame {
                id,
                event: SessionEvent::Recorded {
                    seq: id,
                    event: Event::User {
                        blocks: vec![ContentBlock::Text {
                            text: text.to_owned(),
                        }],
                    },
                },
            },
        }
    }

    fn ask(id: u64) -> ServerMsg {
        ServerMsg::PermissionAsk {
            ask: AskPayload {
                ask_id: id,
                tool: "bash".into(),
                target: Some("rm -rf tmp".into()),
                reason: "cleanup".into(),
                deadline_ms: 120_000,
            },
        }
    }

    fn resolved(id: u64, by: &str) -> ServerMsg {
        ServerMsg::PermissionResolved {
            ask_id: id,
            resolution: Resolution::Allowed { by: by.into() },
        }
    }

    #[test]
    fn omnibar_typing_and_submit_send_chat_command() {
        let mut app = App::new(DEFAULT_CONTEXT_WINDOW);
        let connect: Vec<UiAction> = Vec::new();
        assert_eq!(
            app.step(UiEvent::Key(char_key('i'))),
            connect,
            "focus key is an action, not a send"
        );
        assert!(app.bar_focused());
        for c in "fix the bug".chars() {
            app.step(UiEvent::Key(char_key(c)));
        }
        assert_eq!(app.bar.text(), "fix the bug");
        let actions = app.step(UiEvent::Key(key(crossterm::event::KeyCode::Enter)));
        assert_eq!(
            actions,
            vec![UiAction::Send(ClientMsg::Command {
                command: SessionCommand::Submit {
                    text: "fix the bug".into()
                }
            })]
        );
        assert!(!app.bar_focused());
        assert_eq!(app.bar.text(), "");
    }

    #[test]
    fn submit_while_turn_running_steers() {
        let mut app = App::new(DEFAULT_CONTEXT_WINDOW);
        app.step(UiEvent::Server(ServerMsg::Event {
            frame: Frame {
                id: 1,
                event: SessionEvent::TurnStarted { turn: 1 },
            },
        }));
        assert!(app.turn_running());
        app.step(UiEvent::Key(char_key('i')));
        for c in "also run tests".chars() {
            app.step(UiEvent::Key(char_key(c)));
        }
        let actions = app.step(UiEvent::Key(key(crossterm::event::KeyCode::Enter)));
        assert_eq!(
            actions,
            vec![UiAction::Send(ClientMsg::Command {
                command: SessionCommand::Steer {
                    text: "also run tests".into()
                }
            })]
        );
    }

    #[test]
    fn gate_is_modal_and_yan_answers() {
        let mut app = App::new(DEFAULT_CONTEXT_WINDOW);
        app.step(UiEvent::Key(char_key('i')));
        app.step(UiEvent::Server(ask(7)));
        assert_eq!(app.gate().map(|g| g.ask_id), Some(7));
        assert_eq!(app.bar.text(), "", "gate swallows typing");
        let actions = app.step(UiEvent::Key(char_key('y')));
        assert_eq!(
            actions,
            vec![UiAction::Send(ClientMsg::PermissionAnswer {
                ask_id: 7,
                answer: Answer::AllowOnce,
            })]
        );
        assert!(app.gate().is_some(), "stays open until resolved");
        app.step(UiEvent::Server(resolved(7, "tui-1")));
        assert!(app.gate().is_none());
        assert!(app.view.notes().iter().any(|n| n.contains("allowed")));
    }

    #[test]
    fn gate_deny_and_cancel_are_bound() {
        let mut app = App::new(DEFAULT_CONTEXT_WINDOW);
        app.step(UiEvent::Server(ask(9)));
        assert_eq!(
            app.step(UiEvent::Key(char_key('n'))),
            vec![UiAction::Send(ClientMsg::PermissionAnswer {
                ask_id: 9,
                answer: Answer::Deny,
            })]
        );
        assert_eq!(
            app.step(UiEvent::Key(char_key('c'))),
            vec![UiAction::Send(ClientMsg::PermissionCancel { ask_id: 9 })]
        );
    }

    #[test]
    fn slash_commands_parse_locally_and_note_honestly() {
        let mut app = App::new(DEFAULT_CONTEXT_WINDOW);
        app.step(UiEvent::Key(char_key('i')));
        app.bar.set_text("/mode auto");
        let actions = app.step(UiEvent::Key(key(crossterm::event::KeyCode::Enter)));
        assert!(actions.is_empty(), "protocol v1 has no mode command yet");
        assert!(app.view.notes().iter().any(|n| n.contains("/mode auto")));
        app.step(UiEvent::Key(char_key('i')));
        app.bar.set_text("/wat x");
        app.step(UiEvent::Key(key(crossterm::event::KeyCode::Enter)));
        assert!(
            app.view
                .notes()
                .iter()
                .any(|n| n.contains("unknown command"))
        );
    }

    #[test]
    fn accordions_toggle_and_track_content() {
        let mut app = App::new(DEFAULT_CONTEXT_WINDOW);
        assert!(!app.panels.transcript, "collapsed by default");
        app.step(UiEvent::Key(Key {
            code: crossterm::event::KeyCode::Char('1'),
            mods: crossterm::event::KeyModifiers::CONTROL,
        }));
        assert!(app.panels.transcript);
        app.step(UiEvent::Server(user_msg(1, "hello")));
        assert!(app.panels.transcript);
    }

    #[test]
    fn unbound_keys_and_pong_never_render() {
        let mut app = App::new(DEFAULT_CONTEXT_WINDOW);
        app.rendered();
        for _ in 0..1000 {
            app.step(UiEvent::Key(Key {
                code: crossterm::event::KeyCode::F(24),
                mods: crossterm::event::KeyModifiers::NONE,
            }));
        }
        for _ in 0..1000 {
            app.step(UiEvent::Server(ServerMsg::Pong));
        }
        assert!(!app.should_redraw());
        assert_eq!(
            app.renders(),
            1,
            "no-op inputs must not trigger a single extra render"
        );
    }

    #[test]
    fn events_update_view_and_hud_marks_dirty() {
        let mut app = App::new(DEFAULT_CONTEXT_WINDOW);
        app.rendered();
        app.step(UiEvent::Server(user_msg(1, "0123456789")));
        assert_eq!(app.view.entries().len(), 1);
        assert!(app.should_redraw(), "state change renders");
        assert!(app.hud.gauge_pct().is_some(), "context estimated");
    }

    #[test]
    fn eof_quits_and_requests_shutdown() {
        let mut app = App::new(DEFAULT_CONTEXT_WINDOW);
        let actions = app.step(UiEvent::Eof);
        assert_eq!(actions, vec![UiAction::Shutdown]);
        assert!(app.quit());
    }

    #[test]
    fn resize_marks_dirty_for_redraw() {
        let mut app = App::new(DEFAULT_CONTEXT_WINDOW);
        app.rendered();
        assert!(!app.should_redraw());
        let _ = app.step(UiEvent::Resize);
        assert!(app.should_redraw());
    }

    #[test]
    fn disconnect_then_reconnect_key() {
        let mut app = App::new(DEFAULT_CONTEXT_WINDOW);
        app.step(UiEvent::Attached);
        assert_eq!(app.conn, Conn::Attached);
        app.step(UiEvent::Disconnected);
        assert_eq!(app.conn, Conn::Disconnected);
        let actions = app.step(UiEvent::Key(Key {
            code: crossterm::event::KeyCode::Char('r'),
            mods: crossterm::event::KeyModifiers::CONTROL,
        }));
        assert_eq!(actions, vec![UiAction::Reconnect]);
        app.step(UiEvent::ConnectFailed("daemon unreachable".into()));
        assert_eq!(app.conn, Conn::Failed("daemon unreachable".into()));
    }

    #[test]
    fn resync_marker_and_snapshot_rebuild_app_state() {
        let mut app = App::new(DEFAULT_CONTEXT_WINDOW);
        app.step(UiEvent::Server(user_msg(1, "stale")));
        app.step(UiEvent::Server(ServerMsg::StateResyncRequired {
            last_event_id: 9,
        }));
        assert!(app.view.resync_pending);
        app.step(UiEvent::Server(ServerMsg::Snapshot {
            frames: vec![Frame {
                id: 5,
                event: SessionEvent::Recorded {
                    seq: 5,
                    event: Event::User {
                        blocks: vec![ContentBlock::Text {
                            text: "fresh".into(),
                        }],
                    },
                },
            }],
        }));
        assert!(!app.view.resync_pending);
        assert_eq!(app.view.entries().len(), 1);
        let completions = app.bar.completions(&NoSrc, 5);
        assert!(completions.is_empty());
    }

    #[test]
    fn quit_key_only_when_bar_blurred() {
        let mut app = App::new(DEFAULT_CONTEXT_WINDOW);
        app.step(UiEvent::Key(char_key('i')));
        app.step(UiEvent::Key(char_key('q')));
        assert!(!app.quit(), "q is text while typing");
        app.step(UiEvent::Key(key(crossterm::event::KeyCode::Esc)));
        assert!(!app.bar_focused());
        app.step(UiEvent::Key(char_key('q')));
        assert!(app.quit());
    }

    #[test]
    fn actions_used_in_tests_exist() {
        let _ = Action::ToggleHud.config_name();
        let parsed = omnibar::parse("plain text");
        assert_eq!(parsed, Parsed::Chat("plain text".into()));
    }
}
