//! Foreign-session import: Claude Code + Codex readers.

use harness_core::event::{ContentBlock, Event};
use harness_sessions::foreign::{ForeignReason, claude, codex};

#[test]
fn claude_code_session_maps_losslessly_where_formats_align() {
    let jsonl = concat!(
        r#"{"type":"user","message":{"role":"user","content":"fix the failing test"},"uuid":"u1"}"#,
        "\n",
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"looking now"},{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"cargo test"}}]},"uuid":"a1"}"#,
        "\n",
        r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_1","content":"1 passed"}]},"uuid":"u2"}"#,
        "\n",
        r#"{"type":"system","content":"hook ran","uuid":"s1"}"#,
        "\n",
        r#"{"type":"summary","summary":"old session","leafUuid":"u1"}"#,
        "\n",
    );
    let read = claude::read(jsonl.as_bytes());
    assert!(
        read.events.len() >= 5,
        "mapped events: {}",
        read.events.len()
    );
    assert_eq!(
        read.events[0],
        Event::User {
            blocks: vec![ContentBlock::Text {
                text: "fix the failing test".to_owned()
            }]
        }
    );
    assert_eq!(
        read.events[1],
        Event::Assistant {
            blocks: vec![ContentBlock::Text {
                text: "looking now".to_owned()
            }]
        }
    );
    assert_eq!(
        read.events[2],
        Event::ToolCall {
            call_id: "toolu_1".to_owned(),
            tool: "Bash".to_owned(),
            arguments: serde_json::json!({"command": "cargo test"}),
        }
    );
    assert_eq!(
        read.events[3],
        Event::ToolResult {
            call_id: "toolu_1".to_owned(),
            blocks: vec![ContentBlock::Text {
                text: "1 passed".to_owned()
            }],
        }
    );
    assert_eq!(
        read.events[4],
        Event::System {
            text: "hook ran".to_owned()
        }
    );
    assert_eq!(read.unknown.len(), 1);
    assert_eq!(read.unknown[0].kind, "summary");
    assert_eq!(read.unknown[0].reason, ForeignReason::UnmappedType);
}

#[test]
fn claude_code_malformed_and_unsupported_lines_are_typed() {
    let jsonl = concat!(
        "not json at all\n",
        r#"{"type":"user","message":{"role":"user","content":[{"type":"image","source":{"data":"base64..."}}]}}"#,
        "\n",
    );
    let read = claude::read(jsonl.as_bytes());
    assert!(read.events.is_empty());
    assert_eq!(read.unknown.len(), 2);
    assert_eq!(read.unknown[0].reason, ForeignReason::MalformedJson);
    assert!(matches!(
        read.unknown[1].reason,
        ForeignReason::UnsupportedShape { .. }
    ));
}

#[test]
fn codex_rollout_maps_messages_and_function_calls() {
    let jsonl = concat!(
        r#"{"timestamp":"2026-01-01T00:00:00Z","type":"session_meta","payload":{"id":"rs1"}}"#,
        "\n",
        r#"{"timestamp":"2026-01-01T00:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"run the suite"}]}}"#,
        "\n",
        r#"{"timestamp":"2026-01-01T00:00:02Z","type":"response_item","payload":{"type":"function_call","call_id":"fc_1","name":"shell","arguments":"{\"command\":[\"cargo\",\"nextest\",\"run\"]}"}}"#,
        "\n",
        r#"{"timestamp":"2026-01-01T00:00:03Z","type":"response_item","payload":{"type":"function_call_output","call_id":"fc_1","output":"12 passed"}}"#,
        "\n",
        r#"{"timestamp":"2026-01-01T00:00:04Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"suite is green"}]}}"#,
        "\n",
        r#"{"timestamp":"2026-01-01T00:00:05Z","type":"response_item","payload":{"type":"reasoning","summary":[]}}"#,
        "\n",
    );
    let read = codex::read(jsonl.as_bytes());
    assert_eq!(read.events.len(), 4);
    assert_eq!(
        read.events[0],
        Event::User {
            blocks: vec![ContentBlock::Text {
                text: "run the suite".to_owned()
            }]
        }
    );
    assert_eq!(
        read.events[1],
        Event::ToolCall {
            call_id: "fc_1".to_owned(),
            tool: "shell".to_owned(),
            arguments: serde_json::json!({"command": ["cargo", "nextest", "run"]}),
        }
    );
    assert_eq!(
        read.events[2],
        Event::ToolResult {
            call_id: "fc_1".to_owned(),
            blocks: vec![ContentBlock::Text {
                text: "12 passed".to_owned()
            }],
        }
    );
    assert_eq!(
        read.events[3],
        Event::Assistant {
            blocks: vec![ContentBlock::Text {
                text: "suite is green".to_owned()
            }]
        }
    );
    assert_eq!(read.unknown.len(), 2);
    assert!(
        read.unknown
            .iter()
            .all(|u| u.reason == ForeignReason::UnmappedType)
    );
}

#[test]
fn foreign_read_imports_into_a_resumable_session() {
    let jsonl = concat!(
        r#"{"type":"user","message":{"role":"user","content":"hello from claude"}}"#,
        "\n",
    );
    let read = claude::read(jsonl.as_bytes());
    assert_eq!(read.events.len(), 1);

    let root = tempfile::tempdir().unwrap();
    let store = harness_sessions::SessionStore::open(root.path()).unwrap();
    let meta = store
        .import_events(
            "imported-claude",
            "claude session",
            &read.events,
            &read.origin("claude-code"),
        )
        .unwrap();
    assert_eq!(meta.event_count, 1);
    assert_eq!(meta.imported_from.as_deref(), Some("claude-code"));
    assert_eq!(meta.imported_unknown, 0);

    // Resume: the imported session continues appending.
    let owner = harness_sessions::LeaseOwner::new("w");
    let h = store.open_session_rw("imported-claude", &owner).unwrap();
    let seq = h
        .append(Event::Assistant {
            blocks: vec![ContentBlock::Text {
                text: "picked up where claude left off".to_owned(),
            }],
        })
        .unwrap();
    assert_eq!(seq, harness_core::event::Sequence(2));
}

#[test]
fn torn_tail_is_reported_not_fatal() {
    let jsonl = concat!(
        r#"{"type":"user","message":{"role":"user","content":"committed line"}}"#,
        "\n",
        r#"{"type":"user","message":{"role":"user","content":"torn par"#,
    );
    let read = claude::read(jsonl.as_bytes());
    assert!(read.torn_tail);
    assert_eq!(read.events.len(), 1);
}
