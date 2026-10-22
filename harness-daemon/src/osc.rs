//! OSC 777/9 terminal notification emission plus todo 24's program
//! status lanes: the structured OSC 777 `warp://cli-agent` lifecycle
//! frame and the OSC 7501 Program Status Protocol report (Warp issue
//! #16407 — Crush emits, cmux receives; emit-only, the daemon receives
//! neither). Text is sanitized before framing — a payload must never
//! smuggle a second escape sequence.

use std::io::Write;
use std::sync::Mutex;

use base64ct::{Base64, Encoding};

use crate::notify::{Notification, NotificationSink, NotifyError, ProgramStatus};

/// Bound on emitted text per frame (bytes, char-boundary safe).
pub const OSC_MAX_TEXT_BYTES: usize = 200;

/// Warp's structured cli-agent OSC 777 title sentinel: the frame title
/// its listener intercepts (`view.rs`) before parsing the JSON body.
const WARP_CLI_AGENT_TITLE: &str = "warp://cli-agent";

/// Stable program identifier for the OSC 7501 `app` key (spec grammar
/// `[A-Za-z0-9_.+-]{1,32}`).
const OSC7501_APP: &str = "harness";

/// Strip control characters and bound length. ESC/BEL inside payload
/// text would allow terminal-sequence injection; every C0 control
/// (plus DEL) is replaced by a space — and C1 (U+0080–U+009F), which
/// `char::is_control` also covers, so both OSC lanes inherit the same
/// guarantee.
#[must_use]
pub fn sanitize_osc_text(text: &str, max_bytes: usize) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if cleaned.len() <= max_bytes {
        return cleaned;
    }
    let mut cut = max_bytes;
    while !cleaned.is_char_boundary(cut) {
        cut -= 1;
    }
    cleaned[..cut].to_owned()
}

/// Sink emitting OSC 777 (title + body) and OSC 9 (body) frames to any
/// `Write` handle — stdout in the daemon, pipes in tests. The
/// [`NotificationSink`] trait is the skills/MCP extension point.
pub struct OscSink<W: Write + Send> {
    out: Mutex<W>,
    title: String,
}

impl<W: Write + Send> OscSink<W> {
    /// Sink writing both frames with the given window title.
    #[must_use]
    pub fn new(out: W, title: &str) -> Self {
        Self {
            out: Mutex::new(out),
            title: sanitize_osc_text(title, OSC_MAX_TEXT_BYTES),
        }
    }

    fn frame(&self, notification: &Notification) -> Result<Vec<u8>, NotifyError> {
        let body = sanitize_osc_text(&notification.message, OSC_MAX_TEXT_BYTES);
        let mut frame = format!(
            "\x1b]777;notify;{};{}\x07\x1b]9;{}\x07",
            self.title, body, body
        )
        .into_bytes();
        if let Some(status) = notification.kind.program_status() {
            frame.extend(Self::status_frames(
                Some(status),
                notification.session_id.as_deref(),
                &notification.message,
            ));
        }
        frame.push(b'\n');
        Ok(frame)
    }

    /// Todo 24 status frames for one program-status transition, appended
    /// to every mapped notification's plain notify frames:
    ///
    /// - the structured OSC 777 lifecycle frame — title
    ///   `warp://cli-agent`, body Warp's compact cli-agent JSON
    ///   (`{"v":1,"agent":"harness","event":…,"session_id":…}`);
    /// - the OSC 7501 Program Status Protocol report —
    ///   `state=…[:kind=permission]:app=harness:msg=<base64>` closed
    ///   with ST, every free-text field sanitized first.
    ///
    /// A kind with no terminal status (`None`) emits NOTHING — unknown
    /// kinds must never produce a garbage sequence.
    fn status_frames(
        status: Option<ProgramStatus>,
        session: Option<&str>,
        message: &str,
    ) -> Vec<u8> {
        let Some(status) = status else {
            return Vec::new();
        };
        let mut lifecycle = serde_json::json!({
            "v": 1,
            "agent": OSC7501_APP,
            "event": status.warp_event(),
        });
        if let Some(session) = session {
            lifecycle["session_id"] =
                serde_json::Value::String(sanitize_osc_text(session, OSC_MAX_TEXT_BYTES));
        }
        let mut report = format!(
            "\x1b]777;notify;{WARP_CLI_AGENT_TITLE};{lifecycle}\x07\x1b]7501;state={}",
            status.osc7501_state()
        );
        if matches!(status, ProgramStatus::Blocked) {
            report.push_str(":kind=permission");
        }
        // The 7501 spec bans control characters in the DECODED msg;
        // sanitizing before the base64 makes that guaranteed by
        // construction.
        let msg = Base64::encode_string(sanitize_osc_text(message, OSC_MAX_TEXT_BYTES).as_bytes());
        report.push_str(&format!(":app={OSC7501_APP}:msg={msg}\x1b\\"));
        report.into_bytes()
    }
}

impl<W: Write + Send> NotificationSink for OscSink<W> {
    fn deliver(&self, notification: &Notification) -> Result<(), NotifyError> {
        let frame = self.frame(notification)?;
        let mut out = match self.out.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        out.write_all(&frame)
            .and_then(|_| out.flush())
            .map_err(|e| NotifyError {
                message: format!("osc write: {e}"),
            })
    }
}

/// Stdout sink for the daemon binary.
#[must_use]
pub fn stdout_sink(title: &str) -> OscSink<std::io::Stdout> {
    OscSink::new(std::io::stdout(), title)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notify::{Notification, NotificationKind, NotificationSink};
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct SharedBuf(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedBuf {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            match self.0.lock() {
                Ok(mut guard) => {
                    guard.extend_from_slice(bytes);
                    Ok(bytes.len())
                }
                Err(poisoned) => {
                    poisoned.into_inner().extend_from_slice(bytes);
                    Ok(bytes.len())
                }
            }
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn notification(message: &str) -> Notification {
        Notification {
            kind: NotificationKind::TurnComplete,
            session_id: Some("s-1".into()),
            message: message.into(),
        }
    }

    #[test]
    fn emits_both_osc_777_and_osc_9_frames() -> Result<(), Box<dyn std::error::Error>> {
        let buf = SharedBuf::default();
        let sink = OscSink::new(buf.clone(), "harness");
        sink.deliver(&notification("run done"))?;
        let out = String::from_utf8_lossy(&buf.0.lock().map(|g| g.clone()).unwrap_or_default())
            .into_owned();
        assert!(
            out.contains("\x1b]777;notify;harness;run done\x07"),
            "got {out:?}"
        );
        assert!(out.contains("\x1b]9;run done\x07"), "got {out:?}");
        Ok(())
    }

    #[test]
    fn control_characters_are_stripped_before_framing() {
        let clean = sanitize_osc_text("a\x1b]777;evil\x07b\x00c\r\nd", 200);
        assert!(!clean.contains('\x1b'), "ESC survived: {clean:?}");
        assert!(!clean.contains('\x07'), "BEL survived: {clean:?}");
        assert!(!clean.contains('\0'), "NUL survived: {clean:?}");
        assert!(clean.contains('a') && clean.contains('b') && clean.contains('d'));
    }

    #[test]
    fn sanitizer_bounds_length_on_char_boundary() {
        let text = "é".repeat(400);
        let clean = sanitize_osc_text(&text, 200);
        assert!(clean.len() <= 200);
        assert!(text.starts_with(&clean));
    }

    #[test]
    fn delivered_notifications_never_inject_sequences() -> Result<(), Box<dyn std::error::Error>> {
        let buf = SharedBuf::default();
        let sink = OscSink::new(buf.clone(), "harness");
        sink.deliver(&notification("ok\x1b]9;injected\x07"))?;
        let bytes = buf.0.lock().map(|g| g.clone()).unwrap_or_default();
        // TurnComplete is status-mapped (todo 24), so one deliver emits
        // four sequences: notify-777, OSC 9, structured lifecycle 777
        // (one ESC + one BEL each) and the 7501 report (one ESC for the
        // OSC introducer, one for the ST terminator, no BEL) — exactly
        // five framing ESCs and three framing BELs, nothing injected.
        assert_eq!(
            bytes.iter().filter(|b| **b == 0x1b).count(),
            5,
            "injected escape: {bytes:?}"
        );
        assert_eq!(bytes.iter().filter(|b| **b == 0x07).count(), 3);
        Ok(())
    }

    #[test]
    fn status_frames_for_unmapped_kind_emit_nothing() {
        // The None arm is what a future kind without a terminal status
        // produces: zero status bytes, never a garbage sequence.
        let frames = OscSink::<SharedBuf>::status_frames(None, Some("s-1"), "m");
        assert!(frames.is_empty(), "unmapped kind emitted: {frames:?}");
    }

    #[test]
    fn status_frames_pair_values_stay_in_the_7501_charset() {
        for kind in [
            NotificationKind::ScheduledFired,
            NotificationKind::ApprovalNeeded,
            NotificationKind::TurnComplete,
            NotificationKind::WorkerLost,
        ] {
            let Some(status) = kind.program_status() else {
                panic!("{kind:?} must map");
            };
            let frames = OscSink::<SharedBuf>::status_frames(
                Some(status),
                Some("s-\x1b]777;evil\x07"),
                "m\x1b]7501;evil\x1b\\",
            );
            let text = String::from_utf8_lossy(&frames).into_owned();
            let report = text
                .split("\x1b]7501;")
                .nth(1)
                .unwrap_or_else(|| panic!("no 7501 report for {kind:?}: {text:?}"));
            let body = &report[..report.find("\x1b\\").expect("ST terminator")];
            for value in body.split(':').filter_map(|p| p.split_once('=')) {
                assert!(
                    value
                        .1
                        .chars()
                        .all(|c| matches!(c, 'A'..='Z' | 'a'..='z' | '0'..='9' | '_' | '.' | ',' | '+' | '/' | '=' | '-')),
                    "value outside the 7501 charset: {:?} in {body:?}",
                    value.1
                );
            }
        }
    }
}
