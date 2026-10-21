//! The v1 wire downgrade (plan M9): per-connection rewrite applied at
//! each carrier's outbound funnel when — and only when — the peer
//! negotiated protocol v1. Rust serde cannot decode unknown enum
//! variants, so a pre-built v0.1 frontend receiving a
//! `ContentBlock::Media` or a `ServerMsg::MediaResult` would break;
//! instead the daemon rewrites media blocks to their deterministic
//! text markers inside `Recorded` frames and withholds v2-only server
//! messages entirely. The durable log and the hub keep the TYPED form
//! — this is a wire-only transformation, applied after the hub
//! fan-out, per connection, at the pump where every carrier's
//! outbound passes with the negotiated version in hand (the encode
//! sites — ws writer, ipc line writer, sse unfold — all sit downstream
//! of exactly one pump).

use harness_core::event::{ContentBlock, Event};

use crate::protocol::{Frame, ServerMsg, SessionEvent};

/// Rewrite one outbound message for a v1 connection. `None` means
/// withhold entirely (v2-only variants). Frame ids and every
/// v1-decodable field are preserved byte-for-byte; only media content
/// blocks change (marker text) — image and text blocks pass through
/// untouched.
#[must_use]
pub fn downgrade_for_v1(msg: ServerMsg) -> Option<ServerMsg> {
    match msg {
        // v2-only variants: withheld entirely. Usage/Verdict are
        // accounted nowhere a v1 client can act on them; FallbackAsk /
        // FallbackResolved are replaced on v1 wires by the
        // v1-decodable `Notification` pair the session host publishes
        // alongside (no v1 client silently misses an ask).
        ServerMsg::MediaResult { .. }
        | ServerMsg::Usage { .. }
        | ServerMsg::FallbackAsk { .. }
        | ServerMsg::FallbackResolved { .. }
        | ServerMsg::Verdict { .. } => None,
        ServerMsg::Event { frame } => downgrade_frame(frame).map(|frame| ServerMsg::Event { frame }),
        ServerMsg::Snapshot { frames } => Some(ServerMsg::Snapshot {
            frames: frames.into_iter().filter_map(downgrade_frame).collect(),
        }),
        // Every v1-native message passes through untouched; new
        // variants MUST be listed here (or withheld above) so a future
        // v2-only addition cannot silently reach a v1 wire.
        ServerMsg::HelloAck { .. }
        | ServerMsg::PermissionAsk { .. }
        | ServerMsg::PermissionResolved { .. }
        | ServerMsg::StateResyncRequired { .. }
        | ServerMsg::Error { .. }
        | ServerMsg::Pong
        | ServerMsg::Bye { .. } => Some(msg),
    }
}

/// The SessionEvent gate: an EXHAUSTIVE match over every variant, so
/// a future `SessionEvent` addition fails compilation here until it
/// is classified for v1 wires (the same fail-closed construction the
/// `ServerMsg` match above enforces — previously non-`Recorded`
/// events passed through a let-else with no compiler check).
///
/// `Recorded` payloads get the media-block rewrite; the team-event
/// payloads (`Child*`, todo 8) are DOWNGRADED onto the v1-decodable
/// `Notification` lane with their frame id preserved (v1 clients keep
/// a gap-free id sequence and still see team activity — the string
/// channel the orchestration crate's event vocabulary was designed
/// for); every v1-native daemon event passes through untouched.
fn downgrade_frame(frame: Frame) -> Option<Frame> {
    let Frame { id, event } = frame;
    let event = match event {
        SessionEvent::Recorded { seq, event } => SessionEvent::Recorded {
            seq,
            event: downgrade_event(event),
        },
        event
        @ (SessionEvent::TurnStarted { .. }
        | SessionEvent::TurnFinished { .. }
        | SessionEvent::Diff { .. }
        | SessionEvent::Notification { .. }) => event,
        SessionEvent::ChildStarted { child, task, role, model } => {
            SessionEvent::Notification {
                kind: "child_started".to_owned(),
                message: format!("{child} ({task}) started as {role} on {model}"),
            }
        }
        SessionEvent::ChildProgress { child, phase, detail } => SessionEvent::Notification {
            kind: "child_progress".to_owned(),
            message: format!("{child} {phase}: {detail}"),
        },
        SessionEvent::ChildFinished { child, task, state, detail } => {
            SessionEvent::Notification {
                kind: "child_finished".to_owned(),
                message: format!("{child} ({task}) finished {state}: {detail}"),
            }
        }
        SessionEvent::CostWarning { consumed_micros, cap_micros } => {
            SessionEvent::Notification {
                kind: "cost_warning".to_owned(),
                message: format!(
                    "session spend {consumed_micros} of {cap_micros} micro-USD (80% of cap)"
                ),
            }
        }
        SessionEvent::CostStop { consumed_micros, cap_micros } => SessionEvent::Notification {
            kind: "cost_stop".to_owned(),
            message: format!(
                "session spend {consumed_micros} of {cap_micros} micro-USD (cap reached); \
                 further turns refused"
            ),
        },
    };
    Some(Frame { id, event })
}

fn downgrade_event(mut event: Event) -> Event {
    let blocks = match &mut event {
        Event::User { blocks } | Event::Assistant { blocks } | Event::ToolResult { blocks, .. } => {
            blocks
        }
        Event::ToolCall { .. }
        | Event::System { .. }
        | Event::Condensation { .. }
        | Event::Snapshot { .. } => return event,
    };
    for block in blocks.iter_mut() {
        if let ContentBlock::Media(media) = block {
            *block = ContentBlock::Text {
                text: media.text_marker(),
            };
        }
    }
    event
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_core::event::{ContentHash, MediaBlock, MediaRef, MimeType};
    use std::collections::BTreeMap;

    fn media_block(mime: &str) -> MediaBlock {
        let mut meta = BTreeMap::new();
        meta.insert("format".to_owned(), "png".to_owned());
        MediaBlock::new(
            MimeType::new(mime).unwrap_or_else(|e| panic!("mime: {e}")),
            MediaRef {
                hash: ContentHash::from_bytes(b"x"),
                bytes: 1,
            },
            meta,
        )
        .unwrap_or_else(|e| panic!("meta: {e}"))
    }

    #[test]
    fn media_blocks_become_text_markers_and_ids_survive() {
        let frame = Frame {
            id: 42,
            event: SessionEvent::Recorded {
                seq: 9,
                event: Event::User {
                    blocks: vec![
                        ContentBlock::Text { text: "a".into() },
                        ContentBlock::Media(media_block("image/png")),
                    ],
                },
            },
        };
        let downgraded = downgrade_for_v1(ServerMsg::Event { frame }).expect("kept");
        let ServerMsg::Event { frame } = downgraded else {
            panic!("shape");
        };
        assert_eq!(frame.id, 42);
        let SessionEvent::Recorded { seq, event } = frame.event else {
            panic!("recorded");
        };
        assert_eq!(seq, 9);
        let Event::User { blocks } = event else {
            panic!("user");
        };
        assert_eq!(blocks[0], ContentBlock::Text { text: "a".into() });
        match &blocks[1] {
            ContentBlock::Text { text } => {
                assert!(text.starts_with("[media: image/png blake3:"));
            }
            other => panic!("expected marker text, got {other:?}"),
        }
    }

    #[test]
    fn v2_only_variants_are_withheld() {
        assert_eq!(
            downgrade_for_v1(ServerMsg::MediaResult {
                producer: "p".into(),
                artifact: MediaRef {
                    hash: ContentHash::from_bytes(b"x"),
                    bytes: 1,
                },
                diff_summary: "s".into(),
            }),
            None
        );
    }

    #[test]
    fn non_recorded_frames_and_v1_messages_pass_through() {
        let msg = ServerMsg::Pong;
        assert_eq!(downgrade_for_v1(msg.clone()), Some(msg));
        let frame = Frame {
            id: 1,
            event: SessionEvent::TurnStarted { turn: 3 },
        };
        let msg = ServerMsg::Event { frame };
        assert_eq!(downgrade_for_v1(msg.clone()), Some(msg));
    }

    #[test]
    fn snapshot_frames_are_downgraded_too() {
        let frame = Frame {
            id: 2,
            event: SessionEvent::Recorded {
                seq: 1,
                event: Event::User {
                    blocks: vec![ContentBlock::Media(media_block("audio/wav"))],
                },
            },
        };
        let downgraded = downgrade_for_v1(ServerMsg::Snapshot { frames: vec![frame] })
            .expect("snapshot kept");
        let ServerMsg::Snapshot { frames } = downgraded else {
            panic!("shape");
        };
        let encoded = serde_json::to_string(&frames[0]).unwrap_or_default();
        assert!(!encoded.contains("\"media\""));
        assert!(encoded.contains("[media: audio/wav blake3:"));
    }

    #[test]
    fn cost_events_downgrade_to_v1_notifications() {
        for (event, expected_kind) in [
            (
                SessionEvent::CostWarning {
                    consumed_micros: 850_000,
                    cap_micros: 1_000_000,
                },
                "cost_warning",
            ),
            (
                SessionEvent::CostStop {
                    consumed_micros: 1_100_000,
                    cap_micros: 1_000_000,
                },
                "cost_stop",
            ),
        ] {
            let frame = Frame { id: 9, event };
            let downgraded =
                downgrade_for_v1(ServerMsg::Event { frame }).expect("downgraded, not withheld");
            let ServerMsg::Event { frame } = downgraded else {
                panic!("shape");
            };
            assert_eq!(frame.id, 9, "frame id survives the downgrade");
            match frame.event {
                SessionEvent::Notification { kind, message } => {
                    assert_eq!(kind, expected_kind);
                    assert!(message.contains("1000000"), "message names the cap: {message}");
                }
                other => panic!("expected notification downgrade, got {other:?}"),
            }
        }
    }

    #[test]
    fn media_meta_marker_stays_deterministic() {
        let marker = media_block("model/gltf+json").text_marker();
        let again = media_block("model/gltf+json").text_marker();
        assert_eq!(marker, again);
        assert!(marker.contains("format=png"));
    }
}
