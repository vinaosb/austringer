//! The bounded SSE decoder behind [`super`]: splits WHATWG-conformant
//! events off a byte stream, errors on protocol violations
//! (`sse_stream::Error` taxonomy), and enforces a per-event byte cap.

use futures_util::stream::{BoxStream, Stream, StreamExt};
use sse_stream::{Error as SseError, Sse};

/// Hard cap on one buffered SSE event (defense against a hostile server
/// streaming an endless "event").
pub const MAX_SSE_EVENT_BYTES: usize = 1024 * 1024;

pub(super) fn decode_sse<S, E>(bytes: S) -> BoxStream<'static, Result<Sse, SseError>>
where
    S: Stream<Item = Result<bytes::Bytes, E>> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    let state = SseParseState {
        bytes: Box::pin(bytes),
        buffer: Vec::new(),
    };
    futures_util::stream::unfold(state, |mut state| async move {
        loop {
            if let Some(event) = take_complete_event(&mut state.buffer) {
                return Some((event, state));
            }
            match state.bytes.as_mut().next().await {
                Some(Ok(chunk)) => {
                    state.buffer.extend_from_slice(&chunk);
                    if state.buffer.len() > MAX_SSE_EVENT_BYTES {
                        return Some((
                            Err(SseError::Body("sse event exceeds 1 MiB".into())),
                            state,
                        ));
                    }
                }
                Some(Err(error)) => return Some((Err(SseError::Body(Box::new(error))), state)),
                None => return None,
            }
        }
    })
    .boxed()
}

/// Test-visible alias of the decoder (protocol-fixture assertions).
#[doc(hidden)]
pub fn decode_sse_for_tests<S, E>(bytes: S) -> BoxStream<'static, Result<Sse, SseError>>
where
    S: Stream<Item = Result<bytes::Bytes, E>> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    decode_sse(bytes)
}

struct SseParseState<S> {
    bytes: std::pin::Pin<Box<S>>,
    buffer: Vec<u8>,
}

/// Extracts one fully-framed event (terminated by a blank line) and
/// parses it; comment-only frames yield `Ok(None)` and are skipped by
/// the caller's loop; partial tails are dropped at EOF by the stream.
fn take_complete_event(buffer: &mut Vec<u8>) -> Option<Result<Sse, SseError>> {
    loop {
        let blank = find_blank_line(buffer)?;
        let mut event_bytes: Vec<u8> = buffer.drain(..blank.1).collect();
        event_bytes.truncate(blank.0);
        match parse_event(&event_bytes) {
            Ok(Some(event)) => return Some(Ok(event)),
            Ok(None) => continue,
            Err(error) => return Some(Err(error)),
        }
    }
}

fn find_blank_line(buffer: &[u8]) -> Option<(usize, usize)> {
    let mut start = 0;
    for (index, byte) in buffer.iter().enumerate() {
        if *byte != b'\n' {
            continue;
        }
        let line_len = index - start;
        let line = &buffer[start..index];
        let is_blank = line_len == 0 || (line_len == 1 && line[0] == b'\r');
        if is_blank {
            return Some((start, index + 1));
        }
        start = index + 1;
    }
    None
}

fn parse_event(event_bytes: &[u8]) -> Result<Option<Sse>, SseError> {
    let text = std::str::from_utf8(event_bytes).map_err(SseError::Utf8Parse)?;
    let mut event = Sse::default();
    let mut data: Vec<&str> = Vec::new();
    let mut saw_event = false;
    let mut saw_id = false;
    let mut saw_retry = false;
    let mut any_field = false;
    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        let (field, value) = match line.split_once(':') {
            Some((field, rest)) => (field, rest.strip_prefix(' ').unwrap_or(rest)),
            None => (line, ""),
        };
        any_field = true;
        match field {
            "data" => data.push(value),
            "event" => {
                if saw_event {
                    return Err(SseError::DuplicatedEventLine);
                }
                saw_event = true;
                event.event = Some(value.to_owned());
            }
            "id" => {
                if saw_id {
                    return Err(SseError::DuplicatedIdLine);
                }
                saw_id = true;
                event.id = Some(value.to_owned());
            }
            "retry" => {
                if saw_retry {
                    return Err(SseError::DuplicatedRetry);
                }
                saw_retry = true;
                event.retry = Some(value.parse::<u64>().map_err(SseError::IntParse)?);
            }
            _ => {}
        }
    }
    if !any_field && data.is_empty() {
        return Ok(None);
    }
    if !data.is_empty() {
        event.data = Some(data.join("\n"));
    }
    Ok(Some(event))
}
