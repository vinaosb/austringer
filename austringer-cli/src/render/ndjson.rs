//! The NDJSON eval-adapter contract: one JSON object per line — event
//! frames as `{"event": {"id": N, "event": …}}` envelopes (Recorded
//! frames unwrap to the core event; daemon-level events pass through),
//! every other v2 frame verbatim, plus the final `turn_result` line
//! (the todo-22-named extraction from the render facade).

use austringer_daemon::protocol::{Frame, ServerMsg, SessionEvent};

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

/// One NDJSON line for ANY server message: event frames take the
/// envelope shape; every other frame — usage, fallback ask/resolved,
/// verdict, plan, snapshot, bye — passes through verbatim (the eval
/// driver contract requires the full v2 frame set, never a filtered
/// subset).
#[must_use]
pub fn ndjson_line(msg: &ServerMsg) -> String {
    match msg {
        ServerMsg::Event { frame } => frame_envelope(frame),
        other => serde_json::to_string(other).unwrap_or_else(|_| "{}".to_owned()),
    }
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

#[cfg(test)]
mod tests {
    use super::{ServerMsg, frame_envelope, ndjson_line, outcome_ok, turn_result};
    use austringer_core::event::{ContentBlock, Event};
    use austringer_daemon::protocol::{Frame, SessionEvent, VerdictKind};

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
    fn ndjson_lines_pass_all_v2_frames_through() {
        let usage: ServerMsg = serde_json::from_value(serde_json::json!({
            "type": "usage",
            "turn": 3,
            "model": { "provider": "mock", "model": "mock-1" },
            "effort": null,
            "input_tokens": 10,
            "output_tokens": 20,
            "cache_read": 1,
            "cache_write": 2,
            "cost_usd_micros": null
        }))
        .unwrap();
        let value: serde_json::Value =
            serde_json::from_str(&ndjson_line(&usage)).unwrap_or_else(|e| panic!("json: {e}"));
        assert_eq!(value["type"], serde_json::json!("usage"));
        assert_eq!(value["input_tokens"], serde_json::json!(10));
        assert_eq!(value["model"]["model"], serde_json::json!("mock-1"));

        let fallback: ServerMsg = serde_json::from_value(serde_json::json!({
            "type": "fallback_ask",
            "ask": {
                "ask_id": 7,
                "current": { "provider": "mock", "model": "big" },
                "proposed": { "provider": "mock", "model": "small" },
                "reason": "cap pressure",
                "deadline_ms": 5000
            }
        }))
        .unwrap();
        assert!(
            ndjson_line(&fallback).contains("\"fallback_ask\""),
            "fallback ask must pass through for eval drivers"
        );

        let verdict = ndjson_line(&ServerMsg::Verdict {
            turn: 2,
            verdict: VerdictKind::Confirmed,
            evidence: vec![11, 12],
        });
        assert!(
            verdict.contains("\"verdict\":\"confirmed\""),
            "verdict frame must pass through: {verdict}"
        );

        let team = ndjson_line(&ServerMsg::Event {
            frame: Frame {
                id: 4,
                event: SessionEvent::TeamBudget {
                    spawned: 1,
                    tokens_spent: 5,
                    fan_out_cap: 4,
                    token_budget: 100,
                },
            },
        });
        assert!(team.contains("\"team_budget\""), "{team}");
    }
}
