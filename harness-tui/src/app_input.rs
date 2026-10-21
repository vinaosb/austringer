//! Key-input half of the [`App`] state machine: gate answers
//! (y/a/n/c), omnibar editing, global actions, and submit routing.
//! Split from `app` for the size budget; it is the same state and
//! one dispatch chain (`App::step` -> `App::key`).

use harness_daemon::protocol::{Answer, ClientMsg, SessionCommand};

use crate::app::{App, UiAction};
use crate::keys::{Action, Key};
use crate::omnibar::{Omnibar, Parsed};

impl App {
    /// What this client answered for the open gate, if it did.
    #[must_use]
    pub fn pending_answer_label(&self) -> Option<&'static str> {
        self.pending.map(|answer| match answer {
            Answer::AllowOnce => "allow-once",
            Answer::AllowSession => "allow-session",
            Answer::Deny => "deny",
        })
    }

    pub(crate) fn key(&mut self, key: Key) -> Vec<UiAction> {
        if self.gate.is_some() {
            return self.gate_key(key);
        }
        if self.bar_focused {
            return self.bar_key(key);
        }
        self.global_key(key)
    }

    fn gate_key(&mut self, key: Key) -> Vec<UiAction> {
        let Some(ask) = self.gate.as_ref() else {
            return Vec::new();
        };
        let answer = match self.binds.action(key) {
            Some(Action::AllowOnce) => Some(Answer::AllowOnce),
            Some(Action::AllowSession) => Some(Answer::AllowSession),
            Some(Action::Deny) => Some(Answer::Deny),
            Some(Action::CancelAsk) => {
                let ask_id = ask.ask_id;
                self.dirty = true;
                return vec![UiAction::Send(ClientMsg::PermissionCancel { ask_id })];
            }
            _ => None,
        };
        match answer {
            Some(answer) => {
                self.pending = Some(answer);
                self.dirty = true;
                vec![UiAction::Send(ClientMsg::PermissionAnswer {
                    ask_id: ask.ask_id,
                    answer,
                })]
            }
            None => Vec::new(),
        }
    }

    fn bar_key(&mut self, key: Key) -> Vec<UiAction> {
        let plain = key.mods == crossterm::event::KeyModifiers::NONE
            || key.mods == crossterm::event::KeyModifiers::SHIFT;
        if plain && let crossterm::event::KeyCode::Char(c) = key.code {
            self.bar.insert(c);
            self.dirty = true;
            return Vec::new();
        }
        if key.code == crossterm::event::KeyCode::Backspace {
            self.bar.backspace();
            self.dirty = true;
            return Vec::new();
        }
        match self.binds.action(key) {
            Some(Action::Submit) => self.submit(),
            Some(Action::BlurOmnibar) => {
                self.bar_focused = false;
                self.dirty = true;
                Vec::new()
            }
            Some(action) if self.toggle(action) => Vec::new(),
            _ => Vec::new(),
        }
    }

    fn global_key(&mut self, key: Key) -> Vec<UiAction> {
        match self.binds.action(key) {
            Some(Action::Quit) => {
                self.quit = true;
                self.dirty = true;
                Vec::new()
            }
            Some(Action::Reconnect) => {
                self.dirty = true;
                vec![UiAction::Reconnect]
            }
            Some(Action::FocusOmnibar) => {
                self.bar_focused = true;
                self.dirty = true;
                Vec::new()
            }
            Some(action) if self.toggle(action) => Vec::new(),
            Some(Action::Submit) => self.submit(),
            _ => Vec::new(),
        }
    }

    /// Apply a panel/hud visibility toggle; true when `action` was
    /// one.
    fn toggle(&mut self, action: Action) -> bool {
        let target = match action {
            Action::ToggleTranscript => &mut self.panels.transcript,
            Action::ToggleTools => &mut self.panels.tools,
            Action::ToggleTeam => &mut self.panels.team,
            Action::ToggleHud => &mut self.show_hud,
            _ => return false,
        };
        *target = !*target;
        self.dirty = true;
        true
    }

    /// `/mode` dispatch (todo 16): `plan` enters daemon plan mode and
    /// `off`/`exit` leaves it; the engine-side autonomy levels have no
    /// daemon surface, so they note honestly instead of pretending.
    fn mode_command(&mut self, name: &str) -> Vec<UiAction> {
        match name {
            "plan" => vec![UiAction::Send(ClientMsg::PlanMode { on: true })],
            "off" | "exit" => vec![UiAction::Send(ClientMsg::PlanMode { on: false })],
            other => {
                self.view.note(&format!(
                    "/mode {other}: daemon plan mode is the wired surface (/mode plan, /mode off); \
                     autonomy levels are engine-side"
                ));
                Vec::new()
            }
        }
    }

    fn submit(&mut self) -> Vec<UiAction> {
        let text = self.bar.text();
        if text.is_empty() {
            return Vec::new();
        }
        let parsed = crate::omnibar::parse(&text);
        self.bar = Omnibar::new();
        self.bar_focused = false;
        self.dirty = true;
        match parsed {
            Parsed::Chat(text) => {
                let command = if self.turn_running() {
                    SessionCommand::Steer { text }
                } else {
                    SessionCommand::Submit { text }
                };
                vec![UiAction::Send(ClientMsg::Command { command })]
            }
            Parsed::Mode { name } => self.mode_command(&name),
            Parsed::Rewind { steps } => {
                vec![UiAction::Send(ClientMsg::Rewind {
                    steps: steps.unwrap_or(1),
                })]
            }
            Parsed::Skill { name, rest } => {
                // Submit verbatim, never Steer: the daemon-side trust
                // gate parses /skill off the submit lane — steering
                // would bypass it.
                let text = if rest.is_empty() {
                    format!("/skill {name}")
                } else {
                    format!("/skill {name} {rest}")
                };
                vec![UiAction::Send(ClientMsg::Command {
                    command: SessionCommand::Submit { text },
                })]
            }
            Parsed::UnknownCommand { word } => {
                self.view.note(&format!("unknown command /{word}"));
                Vec::new()
            }
            Parsed::Malformed { usage } => {
                self.view.note(usage);
                Vec::new()
            }
        }
    }
}
