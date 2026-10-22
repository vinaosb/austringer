//! Key-input half of the [`App`] state machine: ask modals
//! (permission y/a/n/c, fallback y/n, plan y/n), hunk review,
//! omnibar editing, scrolling, global actions, and submit routing.
//! Split from `app` for the size budget; it is the same state and
//! one dispatch chain (`App::step` -> `App::key`).

use harness_daemon::protocol::{Answer, ClientMsg, PlanDecision};

use crate::app::{App, UiAction};
use crate::diffs::HunkDecision;
use crate::keys::{Action, Key};
use crate::panels;

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
        if self.fallback_gate.is_some() {
            return self.fallback_key(key);
        }
        if self.plan_gate.is_some() {
            return self.plan_key(key);
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

    /// Fallback-ask modal (permission-gate pattern): y hops to the
    /// proposed identity, n keeps the current one. Both answer
    /// through the mediator-owned wire message — never a local hop.
    fn fallback_key(&mut self, key: Key) -> Vec<UiAction> {
        let Some(ask) = self.fallback_gate.as_ref() else {
            return Vec::new();
        };
        let chosen = match self.binds.action(key) {
            Some(Action::AllowOnce) => Some((ask.proposed.clone(), "hop")),
            Some(Action::Deny) => Some((ask.current.clone(), "keep current")),
            _ => None,
        };
        match chosen {
            Some((chosen, label)) => {
                self.fallback_pending = Some(label);
                self.dirty = true;
                vec![UiAction::Send(ClientMsg::FallbackAnswer {
                    ask: ask.ask_id,
                    chosen,
                })]
            }
            None => Vec::new(),
        }
    }

    /// Plan-approval modal (task 16): y approves, n rejects (plan
    /// mode stays active); edit flows through the daemon-side
    /// artifact surface, not the TUI.
    fn plan_key(&mut self, key: Key) -> Vec<UiAction> {
        let Some(ask) = self.plan_gate.as_ref() else {
            return Vec::new();
        };
        let decision = match self.binds.action(key) {
            Some(Action::AllowOnce) => Some((PlanDecision::Approve, "approve")),
            Some(Action::Deny) => Some((PlanDecision::Reject, "reject")),
            _ => None,
        };
        match decision {
            Some((decision, label)) => {
                self.plan_pending = Some(label);
                self.dirty = true;
                vec![UiAction::Send(ClientMsg::PlanAnswer {
                    ask: ask.ask_id,
                    decision,
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
            Some(Action::ScrollUp) => {
                let max = panels::body_line_count(self);
                if self.scroll_offset < max {
                    self.scroll_offset += 1;
                    self.dirty = true;
                }
                Vec::new()
            }
            Some(Action::ScrollDown) => {
                if self.scroll_offset > 0 {
                    self.scroll_offset -= 1;
                    self.dirty = true;
                }
                Vec::new()
            }
            Some(Action::AllowOnce) if self.hunk_review_active() => self.decide_hunk(true),
            Some(Action::Deny) if self.hunk_review_active() => self.decide_hunk(false),
            Some(Action::Submit) => self.submit(),
            _ => Vec::new(),
        }
    }

    /// True when the diffs accordion is open with hunks to review.
    fn hunk_review_active(&self) -> bool {
        self.panels.diffs && self.view.pending_hunks() > 0
    }

    /// Decide the next pending hunk and record the decision as a
    /// client-side note (the wire has no hunk-decision surface yet).
    fn decide_hunk(&mut self, accept: bool) -> Vec<UiAction> {
        let decision = if accept {
            HunkDecision::Accepted
        } else {
            HunkDecision::Rejected
        };
        let verb = if accept { "accepted" } else { "rejected" };
        if let Some(path) = self.view.decide_next_hunk(decision) {
            self.view.note(&format!("hunk {verb}: {path}"));
            self.dirty = true;
        }
        Vec::new()
    }
    /// Apply a panel/hud visibility toggle; true when `action` was
    /// one.
    fn toggle(&mut self, action: Action) -> bool {
        let target = match action {
            Action::ToggleTranscript => &mut self.panels.transcript,
            Action::ToggleTools => &mut self.panels.tools,
            Action::ToggleTeam => &mut self.panels.team,
            Action::ToggleDiffs => &mut self.panels.diffs,
            Action::ToggleHud => &mut self.show_hud,
            _ => return false,
        };
        *target = !*target;
        self.dirty = true;
        true
    }
}
