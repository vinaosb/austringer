//! Submit-half of the [`App`] state machine (todo 23): one omnibar
//! submit parsed against the shared grammar, dispatched to wire
//! messages or client notes. Split from `app_input` (key handling)
//! for the size budget; same state, same dispatch chain.

use austringer_daemon::grammar;
use austringer_daemon::protocol::{ClientMsg, SessionCommand};

use crate::app::{App, UiAction};
use crate::commands::{self, Parsed};
use crate::omnibar::Omnibar;

impl App {
    /// `/mode` dispatch (todo 16): `plan` enters daemon plan mode and
    /// `off`/`exit` leaves it; the engine-side autonomy levels have no
    /// daemon surface, so they note honestly instead of pretending.
    fn mode_command(&mut self, name: &str) -> Vec<UiAction> {
        match grammar::mode_on(name) {
            Some(on) => vec![UiAction::Send(ClientMsg::PlanMode { on })],
            None => {
                self.view.note(&grammar::mode_note(name));
                Vec::new()
            }
        }
    }

    /// `/vote-cancel` dispatch (todo 23): an explicit id wins; without
    /// one the TUI single tracked gate is the open ask (the modal is
    /// the bound surface; a second concurrent ask replaces the gate).
    fn vote_cancel(&mut self, ask_id: Option<u64>) -> Vec<UiAction> {
        let resolved = match ask_id {
            Some(id) => Some(id),
            None => self.gate.as_ref().map(|ask| ask.ask_id),
        };
        let Some(ask_id) = resolved else {
            self.view.note("no open ask to cancel");
            return Vec::new();
        };
        self.dirty = true;
        vec![UiAction::Send(ClientMsg::PermissionCancel { ask_id })]
    }

    pub(crate) fn submit(&mut self) -> Vec<UiAction> {
        let text = self.bar.text();
        if text.is_empty() {
            return Vec::new();
        }
        let parsed = commands::parse(&text);
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
            Parsed::Help => {
                for line in grammar::cheat_sheet().lines() {
                    self.view.note(line);
                }
                Vec::new()
            }
            Parsed::Steer { text } => {
                vec![UiAction::Send(ClientMsg::Command {
                    command: SessionCommand::Steer { text },
                })]
            }
            Parsed::Cancel => {
                vec![UiAction::Send(ClientMsg::Command {
                    command: SessionCommand::Cancel,
                })]
            }
            Parsed::VoteCancel { ask_id } => self.vote_cancel(ask_id),
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
            Parsed::Condense => {
                // Submit verbatim, never Steer: the daemon-side slash
                // stage runs the compaction.
                vec![UiAction::Send(ClientMsg::Command {
                    command: SessionCommand::Submit {
                        text: "/condense".into(),
                    },
                })]
            }
            Parsed::CliOnly { word } => {
                self.view.note(&grammar::local_only_note(&word));
                Vec::new()
            }
            Parsed::UnknownCommand { word } => {
                self.view.note(&grammar::unknown_help(&word));
                Vec::new()
            }
            Parsed::Malformed { usage } => {
                self.view.note(usage);
                Vec::new()
            }
        }
    }
}
