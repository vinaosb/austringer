//! OAuth endpoint plumbing: form-encoded POSTs over the injected
//! [`Transport`], bounded body reads, and the OAuth error taxonomy
//! (pending / slow_down / invalid_grant) regardless of HTTP status.

use serde_json::Value;

use super::AuthError;
use crate::transport::{Body, ByteStream, Transport, TransportRequest};

/// POSTs `form` to `url` and parses the JSON reply. OAuth error payloads win
/// whenever present — some endpoints answer 200 with an error body behind
/// proxies.
pub(super) async fn auth_post(
    transport: &dyn Transport,
    url: &str,
    form: &[(&str, String)],
) -> Result<Value, AuthError> {
    let request = TransportRequest::new(url, form_urlencoded(form).into_bytes())
        .with_header("content-type", "application/x-www-form-urlencoded")
        .with_header("accept", "application/json");
    let response = transport
        .send(request)
        .await
        .map_err(AuthError::Transport)?;
    let bytes = match response.body {
        Body::Full(bytes) => bytes,
        Body::Stream(mut stream) => read_bounded(&mut stream).await?,
    };
    let parsed: Value = serde_json::from_slice(&bytes)
        .map_err(|err| AuthError::InvalidResponse(format!("non-JSON auth body: {err}")))?;
    if let Some(error) = field_str(&parsed, "error") {
        return Err(match error {
            "authorization_pending" => AuthError::AuthorizationPending,
            "slow_down" => AuthError::SlowDown,
            "invalid_grant" | "expired_token" => {
                AuthError::ReauthRequired(format!("{error}: relogin needed"))
            }
            other => AuthError::Http {
                status: response.status,
                error: other.to_owned(),
                description: field_str(&parsed, "error_description")
                    .unwrap_or_default()
                    .to_owned(),
            },
        });
    }
    if response.status >= 400 {
        return Err(AuthError::Http {
            status: response.status,
            error: "unknown_error".to_owned(),
            description: String::from_utf8_lossy(&bytes).to_string(),
        });
    }
    Ok(parsed)
}

/// Provider payloads are untrusted: never buffer more than 64 KiB of an auth
/// body.
async fn read_bounded(stream: &mut ByteStream) -> Result<Vec<u8>, AuthError> {
    let mut out = Vec::new();
    while let Some(chunk) = stream.next_chunk().await {
        if out.len() + chunk.len() > 64 * 1024 {
            return Err(AuthError::InvalidResponse(
                "auth body exceeds 64 KiB".to_owned(),
            ));
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

/// Minimal `application/x-www-form-urlencoded` encoder: every byte outside
/// the RFC 3986 unreserved set becomes `%XX` (spaces as `%20`, not `+`).
fn form_urlencoded(pairs: &[(&str, String)]) -> String {
    pairs
        .iter()
        .map(|(key, value)| format!("{}={}", encode(key), encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn encode(component: &str) -> String {
    let mut out = String::with_capacity(component.len());
    for byte in component.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

pub(super) fn field_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

pub(super) fn field_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}
