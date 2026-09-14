//! OSC 777/9 terminal notification emission: an escape-sequence bus
//! any listening terminal receives. Text is sanitized before framing —
//! a notification must never smuggle a second escape sequence.

use std::io::Write;
use std::sync::Mutex;

use crate::notify::{Notification, NotificationSink, NotifyError};

/// Bound on emitted text per frame (bytes, char-boundary safe).
pub const OSC_MAX_TEXT_BYTES: usize = 200;

/// Strip control characters and bound length. ESC/BEL inside payload
/// text would allow terminal-sequence injection; every C0 control
/// (plus DEL) is replaced by a space.
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
        frame.push(b'\n');
        Ok(frame)
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
        // Exactly the two framing ESCs we emitted ourselves.
        assert_eq!(
            bytes.iter().filter(|b| **b == 0x1b).count(),
            2,
            "injected escape: {bytes:?}"
        );
        assert_eq!(bytes.iter().filter(|b| **b == 0x07).count(), 2);
        Ok(())
    }
}
