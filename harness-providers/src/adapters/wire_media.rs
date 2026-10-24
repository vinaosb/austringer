//! Per-family native-media wire encoding (plan M8). Base64 appears ONLY
//! in the values built here — inside the rendered request body, never in
//! events, logs, or snapshots. Families without native media (or kinds a
//! family cannot carry) keep the plain text-only request form.

use base64ct::{Base64, Encoding as _};
use serde_json::{Value, json};

use crate::identity::CompletionRequest;
use crate::media::{NativeKind, NativeMedia};

fn b64(media: &NativeMedia) -> String {
    Base64::encode_string(&media.bytes)
}

/// Anthropic Messages content: a plain string for text-only turns
/// (byte-identical to the pre-media form), else a block array with
/// base64 image sources after the text.
pub(crate) fn anthropic_content(request: &CompletionRequest) -> Value {
    if request.media.is_empty() {
        return Value::String(request.prompt.clone());
    }
    let mut blocks = vec![json!({ "type": "text", "text": request.prompt })];
    for media in &request.media {
        match media.kind {
            NativeKind::Image => blocks.push(json!({
                "type": "image",
                "source": { "type": "base64", "media_type": media.mime, "data": b64(media) }
            })),
            NativeKind::Audio => tracing::warn!(
                mime = &media.mime,
                "anthropic wire has no native audio part; dropping from render"
            ),
        }
    }
    Value::Array(blocks)
}

/// Chat-completions content (OpenAI + xAI + compat): plain string for
/// text-only turns, else `image_url` data URLs and `input_audio` blocks.
pub(crate) fn chat_content(request: &CompletionRequest) -> Value {
    if request.media.is_empty() {
        return Value::String(request.prompt.clone());
    }
    let mut blocks = vec![json!({ "type": "text", "text": request.prompt })];
    for media in &request.media {
        match media.kind {
            NativeKind::Image => blocks.push(json!({
                "type": "image_url",
                "image_url": { "url": format!("data:{};base64,{}", media.mime, b64(media)) }
            })),
            NativeKind::Audio => match audio_format(&media.mime) {
                Some(format) => blocks.push(json!({
                    "type": "input_audio",
                    "input_audio": { "data": b64(media), "format": format }
                })),
                None => tracing::warn!(
                    mime = &media.mime,
                    "chat-completions input_audio accepts wav/mp3 only; dropping from render"
                ),
            },
        }
    }
    Value::Array(blocks)
}

/// `input_audio` accepts exactly `wav` or `mp3` formats.
fn audio_format(mime: &str) -> Option<&'static str> {
    match mime {
        "audio/wav" => Some("wav"),
        "audio/mpeg" => Some("mp3"),
        _ => None,
    }
}

/// Gemini `parts`: the text part plus `inline_data` parts (Gemini's
/// acceptor is MIME-typed, images and audio share the shape).
pub(crate) fn google_parts(request: &CompletionRequest) -> Value {
    let mut parts = vec![json!({ "text": request.prompt })];
    for media in &request.media {
        parts.push(json!({
            "inline_data": { "mime_type": media.mime, "data": b64(media) }
        }));
    }
    Value::Array(parts)
}

/// Responses API `input`: a plain string for text-only turns, else a
/// user message carrying `input_text` + `input_image`/`input_audio`
/// data-URL parts.
pub(crate) fn responses_input(request: &CompletionRequest) -> Value {
    if request.media.is_empty() {
        return Value::String(request.prompt.clone());
    }
    let mut parts = vec![json!({ "type": "input_text", "text": request.prompt })];
    for media in &request.media {
        let data_url = format!("data:{};base64,{}", media.mime, b64(media));
        match media.kind {
            NativeKind::Image => parts.push(json!({
                "type": "input_image", "image_url": data_url
            })),
            NativeKind::Audio => parts.push(json!({
                "type": "input_audio", "input_audio": data_url
            })),
        }
    }
    json!([{ "role": "user", "content": parts }])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::ModelIdentity;
    use harness_core::event::media::ContentHash;

    fn request_with(media: Vec<NativeMedia>) -> CompletionRequest {
        let mut request = CompletionRequest::new(ModelIdentity::new("openai", "m"), "prompt");
        if !media.is_empty() {
            request = request.with_native_media(media);
        }
        request
    }

    fn image(bytes: &[u8]) -> NativeMedia {
        NativeMedia {
            kind: NativeKind::Image,
            mime: "image/png".to_owned(),
            hash: ContentHash::from_bytes(bytes),
            bytes: bytes.to_vec(),
        }
    }

    #[test]
    fn text_only_forms_stay_plain_strings() {
        let request = request_with(Vec::new());
        assert_eq!(anthropic_content(&request), Value::String("prompt".into()));
        assert_eq!(chat_content(&request), Value::String("prompt".into()));
        assert_eq!(responses_input(&request), Value::String("prompt".into()));
        assert_eq!(
            google_parts(&request),
            json!([{ "text": "prompt" }]),
            "gemini parts were always an array"
        );
    }

    #[test]
    fn debug_never_prints_bytes() {
        let media = image(b"SECRET-BYTES");
        let rendered = format!("{media:?}");
        assert!(!rendered.contains("SECRET-BYTES"));
        assert!(rendered.contains("12B (redacted)"));
    }
}
