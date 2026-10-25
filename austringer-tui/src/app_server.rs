//! Server-message half of the [`App`] state machine: applies every
//! `ServerMsg` to the view, the HUD, and the ask modals. Split from
//! `app` for the size budget; it is the same state and one dispatch
//! chain (`App::step` -> `App::server`).

use austringer_daemon::protocol::{PlanOutcome, Resolution, ServerMsg};

use crate::app::{App, Conn, UiAction};
use crate::hud::HudUpdate;

/// micro-USD (the protocol's integer money path) to HUD dollars.
const MICROS_PER_DOLLAR: f64 = 1_000_000.0;

impl App {
    pub(crate) fn server(&mut self, msg: ServerMsg) -> Vec<UiAction> {
        match msg {
            ServerMsg::HelloAck { version, .. } => {
                self.negotiated_version = Some(version);
                if self.conn != Conn::Attached {
                    self.conn = Conn::Attached;
                    self.dirty = true;
                }
            }
            ServerMsg::Event { frame } => {
                self.view.apply_frame(&frame);
                self.hud.estimate_context(self.view.context_chars());
                self.scroll_offset = 0;
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
                self.scroll_offset = 0;
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
                self.view
                    .note(&format!("media ({producer}): {diff_summary}"));
                self.dirty = true;
            }
            ServerMsg::Usage {
                model,
                input_tokens,
                cache_read,
                cost_usd_micros,
                ..
            } => {
                let cache_hit_pct = if input_tokens > 0 {
                    Some(cache_read as f64 * 100.0 / input_tokens as f64)
                } else {
                    None
                };
                self.hud.apply(&HudUpdate {
                    cache_hit_pct,
                    model: model.model.clone(),
                    cost_delta: cost_usd_micros
                        .map_or(0.0, |micros| micros as f64 / MICROS_PER_DOLLAR),
                });
                self.dirty = true;
            }
            ServerMsg::FallbackAsk { ask } => {
                self.fallback_gate = Some(ask);
                self.fallback_pending = None;
                self.dirty = true;
            }
            ServerMsg::FallbackResolved { ask_id, chosen } => {
                if self
                    .fallback_gate
                    .as_ref()
                    .is_some_and(|ask| ask.ask_id == ask_id)
                {
                    self.fallback_gate = None;
                    self.fallback_pending = None;
                }
                self.view.note(&format!(
                    "fallback {ask_id} resolved: next model {}",
                    chosen.model
                ));
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
            ServerMsg::PlanAsk { ask } => {
                self.plan_gate = Some(ask);
                self.plan_pending = None;
                self.dirty = true;
            }
            ServerMsg::PlanResolved {
                ask_id,
                plan_id,
                outcome,
            } => {
                if self
                    .plan_gate
                    .as_ref()
                    .is_some_and(|ask| ask.ask_id == ask_id)
                {
                    self.plan_gate = None;
                    self.plan_pending = None;
                }
                self.view
                    .note(&plan_resolution_note(ask_id, plan_id, &outcome));
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

fn plan_resolution_note(ask_id: u64, plan_id: u64, outcome: &PlanOutcome) -> String {
    match outcome {
        PlanOutcome::Approved { by } => {
            format!("plan ask {ask_id}: plan {plan_id} approved by {by}; writes re-armed")
        }
        PlanOutcome::Rejected { by } => {
            format!("plan ask {ask_id}: plan {plan_id} rejected by {by}; still gated")
        }
        PlanOutcome::Edited { by, .. } => {
            format!(
                "plan ask {ask_id}: plan {plan_id} edited by {by}; re-gated on the new revision"
            )
        }
        PlanOutcome::Cancelled { by } => {
            format!("plan ask {ask_id}: withdrawn by {by}; plan {plan_id} stays pending")
        }
        PlanOutcome::Timeout => format!(
            "plan ask {ask_id} timed out; plan {plan_id} stays pending (never auto-approved)"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::UiEvent;
    use austringer_daemon::protocol::decode_server;

    fn app() -> App {
        App::new(crate::app::DEFAULT_CONTEXT_WINDOW)
    }

    #[test]
    fn hello_ack_records_the_negotiated_version() {
        let mut app = app();
        let ack =
            decode_server(br#"{"type":"hello_ack","version":2,"last_event_id":0}"#.as_slice())
                .expect("fixture ack decodes");
        app.step(UiEvent::Server(ack));
        assert_eq!(app.negotiated_version(), Some(2));
        assert_eq!(app.conn, Conn::Attached);
    }

    #[test]
    fn fallback_and_plan_gates_open_and_resolve() {
        let mut app = app();
        let ask = decode_server(
            br#"{"type":"fallback_ask","ask":{"ask_id":9,"current":{"provider":"p","model":"a"},
                "proposed":{"provider":"p","model":"b"},"reason":"r","deadline_ms":1000}}"#
                .as_slice(),
        )
        .expect("fixture ask decodes");
        app.step(UiEvent::Server(ask));
        assert!(app.fallback_gate().is_some());
        let resolved = decode_server(
            br#"{"type":"fallback_resolved","ask_id":9,"chosen":{"provider":"p","model":"b"}}"#
                .as_slice(),
        )
        .expect("fixture resolution decodes");
        app.step(UiEvent::Server(resolved));
        assert!(app.fallback_gate().is_none());
        assert!(
            app.view.notes().iter().any(|n| n.contains("fallback 9")),
            "notes: {:?}",
            app.view.notes()
        );
    }
}
