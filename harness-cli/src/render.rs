//! Output rendering for both modes. `--output-json` (the eval adapter
//! contract) emits one JSON object per line: event frames as
//! `{"event": {"id": N, "event": …}}` (Recorded frames unwrap to the
//! core event; daemon-level events pass through), plus a final
//! `{"type": "turn_result", …}` for run/repl turns. Human mode renders
//! streaming text lines. Everything here is pure and unit-tested.

use harness_core::event::{ContentBlock, Event};
use harness_daemon::protocol::{AskPayload, Frame, Resolution, ServerMsg, SessionEvent};

/// One NDJSON envelope line for an event frame.
#[must_use]
pub fn frame_envelope(frame: &Frame) -> String {
    let inner = match &frame.event {
        SessionEvent::Recorded { event, .. } => serde_json::to_value(event),
        other => serde_json::to_value(other),
    };
    let inner = inner.unwrap_or(serde_json::Value::Null);
    serde_json::json!({ "event": { "id": frame.id, "event": inner } }).to_string()
}

/// True when a turn outcome counts as success for exit-code purposes.
#[must_use]
pub fn outcome_ok(outcome: &str) -> bool {
    matches!(outcome, "task_completed" | "turn_complete")
}

/// The final NDJSON line for a finished turn.
#[must_use]
pub fn turn_result(turn: u64, outcome: &str) -> String {
    serde_json::json!({
        "type": "turn_result",
        "ok": outcome_ok(outcome),
        "outcome": outcome,
        "turn": turn,
    })
    .to_string()
}

/// Human-mode line for a server message; `None` prints nothing.
#[must_use]
pub fn human(msg: &ServerMsg) -> Option<String> {
    match msg {
        ServerMsg::Event { frame } => human_event(&frame.event),
        ServerMsg::PermissionAsk { ask } => Some(human_ask(ask)),
        ServerMsg::PermissionResolved { resolution, .. } => Some(human_resolution(resolution)),
        ServerMsg::StateResyncRequired { last_event_id } => Some(format!(
            "— resync: state diverged, replaying from {last_event_id} —"
        )),
        ServerMsg::Snapshot { .. } => Some("— snapshot received —".to_owned()),
        ServerMsg::Bye { reason } => Some(format!("— daemon closed the stream: {reason} —")),
        ServerMsg::HelloAck { .. } | ServerMsg::Pong => None,
        ServerMsg::Error { code, message } => Some(format!("daemon error ({code:?}): {message}")),
        ServerMsg::MediaResult {
            producer,
            diff_summary,
            ..
        } => Some(format!("media ({producer}): {diff_summary}")),
        ServerMsg::Usage {
            model,
            input_tokens,
            output_tokens,
            ..
        } => Some(format!(
            "usage: {} — {input_tokens} in / {output_tokens} out",
            model.model
        )),
        ServerMsg::FallbackAsk { ask } => Some(format!(
            "model fallback proposed: {} -> {} — {} (answer over WS/IPC)",
            ask.current.model, ask.proposed.model, ask.reason
        )),
        ServerMsg::FallbackResolved { ask_id, chosen } => {
            Some(format!("fallback {ask_id} resolved: next model {}", chosen.model))
        }
        ServerMsg::Verdict {
            turn,
            verdict,
            evidence,
        } => Some(format!(
            "turn {turn} verdict: {} ({} evidence events)",
            verdict.as_str(),
            evidence.len()
        )),
    }
}

fn human_event(event: &SessionEvent) -> Option<String> {
    match event {
        SessionEvent::Recorded { event, .. } => human_core_event(event),
        SessionEvent::TurnStarted { turn } => Some(format!("— turn {turn} started —")),
        SessionEvent::TurnFinished { turn, outcome } => {
            Some(format!("— turn {turn} finished: {outcome} —"))
        }
        SessionEvent::Diff { path, .. } => Some(format!("diff: {path}")),
        SessionEvent::Notification { message, .. } => Some(format!("notice: {message}")),
        SessionEvent::ChildStarted { child, task, role, .. } => {
            Some(format!("child started: {child} ({task}, {role})"))
        }
        SessionEvent::ChildProgress { child, phase, detail } => {
            Some(format!("child {child} {phase}: {detail}"))
        }
        SessionEvent::ChildFinished { child, task, state, detail } => {
            Some(format!("child {child} ({task}) finished {state}: {detail}"))
        }
    }
}

fn human_core_event(event: &Event) -> Option<String> {
    match event {
        Event::User { blocks } => Some(format!("you: {}", blocks_text(blocks))),
        Event::Assistant { blocks } => Some(format!("assistant: {}", blocks_text(blocks))),
        Event::ToolCall { call_id, tool, .. } => Some(format!("tool call {call_id}: {tool}")),
        Event::ToolResult { call_id, blocks } => {
            Some(format!("tool result {call_id}: {}", blocks_text(blocks)))
        }
        Event::System { text } => Some(format!("system: {text}")),
        Event::Condensation { .. } | Event::Snapshot { .. } => None,
    }
}

fn blocks_text(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .map(ContentBlock::text_marker)
        .collect::<Vec<_>>()
        .join("\n")
}

fn human_ask(ask: &AskPayload) -> String {
    format!(
        "permission requested: {} — {} [y]allow once [s]allow session [n]deny",
        ask.tool, ask.reason
    )
}

fn human_resolution(resolution: &Resolution) -> String {
    match resolution {
        Resolution::Allowed { by } => format!("permission allowed by {by}"),
        Resolution::Denied { by } => format!("permission denied by {by}"),
        Resolution::Cancelled { by } => format!("permission cancelled by {by}"),
        Resolution::Timeout => "permission ask timed out".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_core::event::ContentBlock;

    fn user_frame(id: u64, text: &str) -> Frame {
        Frame {
            id,
            event: SessionEvent::Recorded {
                seq: id,
                event: Event::User {
                    blocks: vec![ContentBlock::Text {
                        text: text.to_owned(),
                    }],
                },
            },
        }
    }

    #[test]
    fn recorded_frames_unwrap_to_the_core_event() {
        let line = frame_envelope(&user_frame(3, "hello there"));
        let value: serde_json::Value =
            serde_json::from_str(&line).unwrap_or_else(|e| panic!("json: {e}"));
        assert_eq!(value["event"]["id"], serde_json::json!(3));
        assert_eq!(value["event"]["event"]["type"], serde_json::json!("user"));
        assert_eq!(
            value["event"]["event"]["blocks"][0]["text"],
            serde_json::json!("hello there")
        );
    }

    #[test]
    fn daemon_level_frames_pass_through() {
        let frame = Frame {
            id: 9,
            event: SessionEvent::TurnFinished {
                turn: 2,
                outcome: "failed::InferenceExhausted".to_owned(),
            },
        };
        let line = frame_envelope(&frame);
        let value: serde_json::Value =
            serde_json::from_str(&line).unwrap_or_else(|e| panic!("json: {e}"));
        assert_eq!(
            value["event"]["event"]["type"],
            serde_json::json!("turn_finished")
        );
        assert_eq!(
            value["event"]["event"]["outcome"],
            serde_json::json!("failed::InferenceExhausted")
        );
    }

    #[test]
    fn turn_result_marks_success_and_failure() {
        let ok: serde_json::Value = serde_json::from_str(&turn_result(1, "task_completed"))
            .unwrap_or_else(|e| panic!("json: {e}"));
        assert_eq!(ok["type"], serde_json::json!("turn_result"));
        assert_eq!(ok["ok"], serde_json::json!(true));
        assert_eq!(ok["outcome"], serde_json::json!("task_completed"));
        let bad: serde_json::Value = serde_json::from_str(&turn_result(2, "failed::Boom"))
            .unwrap_or_else(|e| panic!("json: {e}"));
        assert_eq!(bad["ok"], serde_json::json!(false));
        assert!(!outcome_ok("cancelled@4"));
        assert!(outcome_ok("turn_complete"));
    }

    #[test]
    fn human_lines_carry_the_pinned_keywords() {
        let ask = human(&ServerMsg::PermissionAsk {
            ask: AskPayload {
                ask_id: 1,
                tool: "bash".to_owned(),
                target: None,
                reason: "runs a command".to_owned(),
                deadline_ms: 1000,
            },
        })
        .unwrap_or_default();
        assert!(ask.contains("permission"), "{ask}");
        let denied = human(&ServerMsg::PermissionResolved {
            ask_id: 1,
            resolution: Resolution::Denied {
                by: "cli".to_owned(),
            },
        })
        .unwrap_or_default();
        assert!(denied.contains("denied"), "{denied}");
        let finished = human_event(&SessionEvent::TurnFinished {
            turn: 1,
            outcome: "task_completed".to_owned(),
        })
        .unwrap_or_default();
        assert!(finished.contains("task_completed"), "{finished}");
        let user = human(&ServerMsg::Event {
            frame: user_frame(1, "say something"),
        })
        .unwrap_or_default();
        assert!(user.contains("say something"), "{user}");
    }
}
