//! v0.1 byte-stability characterization (plan M3, acceptance 1).
//!
//! `tests/fixtures/v01_image_event.jsonl` is a verbatim session-log line
//! recorded from the UNCHANGED v0.1 code (scratch recorder deleted after
//! the capture; the bytes below are frozen, never regenerated). The M3
//! widening of `MimeType` and the additive `ContentBlock::Media` variant
//! must not move a single byte of that serialized form, and the v0.1
//! image-block marker format must stay untouched.

use std::fs;

use harness_core::event::content::ContentBlock;
use harness_core::event::{RecordedEvent, Sequence};

/// Verbatim recorded v0.1 line (trimmed of the JSONL newline).
const V01_LINE: &str = concat!(
    r#"{"seq":1,"event":{"type":"user","blocks":[{"type":"text","text":"describe this diagram""#,
    r#"},{"type":"image","mime":"image/png","width":800,"height":600,"#,
    r#""media":{"hash":"blake3:636ca83a67253e2cab7738d7b7ed32e3400acd794298c8c08633212b5d31be7a","#,
    r#""bytes":32}}]}}"#
);

/// The v0.1 image marker format, frozen byte-for-byte.
const V01_IMAGE_MARKER: &str = "[image: image/png 800x600 blake3:636ca83a67253e2c…]";

#[test]
fn fixture_file_matches_embedded_frozen_line() {
    let raw = fs::read_to_string("tests/fixtures/v01_image_event.jsonl")
        .expect("committed fixture must be readable");
    assert_eq!(
        raw.trim_end(),
        V01_LINE,
        "fixture file drifted from the frozen v0.1 line"
    );
}

#[test]
fn v01_image_event_deserializes_and_reserializes_byte_stably() {
    let recorded: RecordedEvent =
        serde_json::from_str(V01_LINE).expect("v0.1 line must deserialize");
    assert_eq!(recorded.seq, Sequence(1));
    let reencoded = serde_json::to_string(&recorded).expect("re-serialize");
    assert_eq!(
        reencoded, V01_LINE,
        "re-serialization must be byte-identical to the recorded v0.1 form"
    );
}

#[test]
fn v01_image_marker_format_is_untouched() {
    let recorded: RecordedEvent =
        serde_json::from_str(V01_LINE).expect("v0.1 line must deserialize");
    let RecordedEvent {
        event: harness_core::event::Event::User { blocks },
        ..
    } = &recorded
    else {
        panic!("fixture must decode to a user event");
    };
    let image = blocks
        .iter()
        .find_map(|block| match block {
            ContentBlock::Image(image) => Some(image),
            _ => None,
        })
        .expect("fixture carries an image block");
    assert_eq!(image.text_marker(), V01_IMAGE_MARKER);
    assert_eq!(
        blocks[1].text_marker(),
        V01_IMAGE_MARKER,
        "ContentBlock-level rendering must match the block-level marker"
    );
}
