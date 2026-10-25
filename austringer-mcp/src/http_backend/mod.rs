//! Streamable HTTP client backend over the WORKSPACE reqwest 0.12 (the
//! one the provider adapters already ship, rustls/ring TLS). rmcp's
//! optional reqwest 0.13 backend is deliberately not enabled: this keeps
//! one HTTP stack in the tree and honors the workspace's no-native-tls
//! policy. Implements rmcp's transport-agnostic `StreamableHttpClient`
//! trait; the SSE decoder lives in [`sse`].

mod sse;

pub use sse::decode_sse_for_tests;

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use futures_util::stream::BoxStream;
use http::{HeaderName, HeaderValue};
use rmcp::model::{ClientJsonRpcMessage, ServerJsonRpcMessage};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClient, StreamableHttpError, StreamableHttpPostResponse,
};
use sse_stream::{Error as SseError, Sse};

const EVENT_STREAM_MIME: &str = "text/event-stream";
const JSON_MIME: &str = "application/json";
const HEADER_SESSION_ID: HeaderName = HeaderName::from_static("mcp-session-id");
const HEADER_LAST_EVENT_ID: HeaderName = HeaderName::from_static("last-event-id");

/// reqwest-backed transport client. Redirects are disabled (custom headers
/// must never replay to a redirect target) and idle pooling is off
/// (mirrors rmcp's own backend hardening against delayed-ACK stalls).
#[derive(Debug, Clone)]
pub struct ReqwestSseClient {
    http: reqwest::Client,
}

impl Default for ReqwestSseClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ReqwestSseClient {
    #[must_use]
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .pool_max_idle_per_host(0)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap_or_default();
        Self { http }
    }
}

fn check_custom_headers(
    custom_headers: &HashMap<HeaderName, HeaderValue>,
) -> Result<(), StreamableHttpError<reqwest::Error>> {
    for name in custom_headers.keys() {
        if name == HEADER_SESSION_ID
            || name == HEADER_LAST_EVENT_ID
            || name == http::header::AUTHORIZATION
            || name == http::header::ACCEPT
            || name == http::header::CONTENT_TYPE
        {
            return Err(StreamableHttpError::ReservedHeaderConflict(
                name.to_string(),
            ));
        }
    }
    Ok(())
}

fn unexpected_status(
    status: reqwest::StatusCode,
    body: String,
) -> StreamableHttpError<reqwest::Error> {
    StreamableHttpError::UnexpectedServerResponse(Cow::Owned(format!("HTTP {status}: {body}")))
}

impl StreamableHttpClient for ReqwestSseClient {
    type Error = reqwest::Error;

    async fn post_message(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        _auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        check_custom_headers(&custom_headers)?;
        let body = serde_json::to_vec(&message).map_err(|error| {
            StreamableHttpError::UnexpectedServerResponse(Cow::Owned(format!(
                "request serialization failed: {error}"
            )))
        })?;
        let mut request = self
            .http
            .post(uri.as_ref())
            .header(
                http::header::ACCEPT,
                format!("{EVENT_STREAM_MIME}, {JSON_MIME}"),
            )
            .header(http::header::CONTENT_TYPE, JSON_MIME)
            .body(body);
        if let Some(session_id) = &session_id {
            request = request.header(HEADER_SESSION_ID, session_id.as_ref());
        }
        for (name, value) in custom_headers {
            request = request.header(name, value);
        }
        let response = request.send().await.map_err(StreamableHttpError::Client)?;
        let status = response.status();
        if status == reqwest::StatusCode::ACCEPTED || status == reqwest::StatusCode::NO_CONTENT {
            return Ok(StreamableHttpPostResponse::Accepted);
        }
        if status == reqwest::StatusCode::NOT_FOUND && session_id.is_some() {
            return Err(StreamableHttpError::SessionExpired);
        }
        let content_type = response
            .headers()
            .get(http::header::CONTENT_TYPE)
            .map(|ct| String::from_utf8_lossy(ct.as_bytes()).into_owned());
        let new_session = response
            .headers()
            .get(HEADER_SESSION_ID)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            if content_type
                .as_deref()
                .is_some_and(|ct| ct.starts_with(JSON_MIME))
                && let Ok(parsed) = serde_json::from_str::<ServerJsonRpcMessage>(&body)
            {
                return Ok(StreamableHttpPostResponse::Json(parsed, new_session));
            }
            return Err(unexpected_status(status, body));
        }
        match content_type.as_deref() {
            Some(ct) if ct.starts_with(EVENT_STREAM_MIME) => Ok(StreamableHttpPostResponse::Sse(
                sse::decode_sse(response.bytes_stream()),
                new_session,
            )),
            Some(ct) if ct.starts_with(JSON_MIME) => {
                let body = response
                    .bytes()
                    .await
                    .map_err(StreamableHttpError::Client)?;
                match serde_json::from_slice::<ServerJsonRpcMessage>(&body) {
                    Ok(parsed) => Ok(StreamableHttpPostResponse::Json(parsed, new_session)),
                    Err(_) if !message_expects_reply(&message) => {
                        Ok(StreamableHttpPostResponse::Accepted)
                    }
                    Err(error) => Err(StreamableHttpError::UnexpectedServerResponse(Cow::Owned(
                        format!("invalid JSON reply: {error}"),
                    ))),
                }
            }
            other => Err(StreamableHttpError::UnexpectedContentType(
                other.map(str::to_owned),
            )),
        }
    }

    async fn delete_session(
        &self,
        uri: Arc<str>,
        session: Arc<str>,
        _auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<(), StreamableHttpError<Self::Error>> {
        check_custom_headers(&custom_headers)?;
        let mut request = self
            .http
            .delete(uri.as_ref())
            .header(HEADER_SESSION_ID, session.as_ref());
        for (name, value) in custom_headers {
            request = request.header(name, value);
        }
        let response = request.send().await.map_err(StreamableHttpError::Client)?;
        if response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED {
            return Ok(());
        }
        response
            .error_for_status()
            .map(|_| ())
            .map_err(StreamableHttpError::Client)
    }

    async fn get_stream(
        &self,
        uri: Arc<str>,
        session_id: Option<Arc<str>>,
        last_event_id: Option<String>,
        _auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<BoxStream<'static, Result<Sse, SseError>>, StreamableHttpError<Self::Error>> {
        check_custom_headers(&custom_headers)?;
        let mut request = self.http.get(uri.as_ref()).header(
            http::header::ACCEPT,
            format!("{EVENT_STREAM_MIME}, {JSON_MIME}"),
        );
        if let Some(session_id) = &session_id {
            request = request.header(HEADER_SESSION_ID, session_id.as_ref());
        }
        if let Some(last_event_id) = last_event_id {
            request = request.header(HEADER_LAST_EVENT_ID, last_event_id);
        }
        for (name, value) in custom_headers {
            request = request.header(name, value);
        }
        let response = request.send().await.map_err(StreamableHttpError::Client)?;
        if response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED {
            return Err(StreamableHttpError::ServerDoesNotSupportSse);
        }
        let response = response
            .error_for_status()
            .map_err(StreamableHttpError::Client)?;
        match response
            .headers()
            .get(http::header::CONTENT_TYPE)
            .map(|ct| String::from_utf8_lossy(ct.as_bytes()).into_owned())
        {
            Some(ct) if ct.starts_with(EVENT_STREAM_MIME) => {}
            other => return Err(StreamableHttpError::UnexpectedContentType(other)),
        }
        Ok(sse::decode_sse(response.bytes_stream()))
    }
}

fn message_expects_reply(message: &ClientJsonRpcMessage) -> bool {
    !matches!(
        message,
        ClientJsonRpcMessage::Notification(_)
            | ClientJsonRpcMessage::Response(_)
            | ClientJsonRpcMessage::Error(_)
    )
}
