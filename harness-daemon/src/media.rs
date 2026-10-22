//! Media HTTP surface (plan M9): the size-capped streamed upload route
//! and the token-authed fetch route. Media bytes NEVER ride protocol
//! frames — this is the only client-facing byte path in and out of the
//! daemon. Both routes sit behind the shared auth middleware like
//! every `/s/{sid}/…` sibling.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;
use serde::Deserialize;
use tokio::io::AsyncWriteExt;

use harness_core::event::{ContentHash, MediaStore, MimeType};
use harness_media::ProbeResult;
use harness_media::probe::{probe, registry};

use crate::server::DaemonState;

#[derive(Deserialize)]
pub(crate) struct MediaQuery {
    filename: Option<String>,
}

fn typed_error(status: StatusCode, code: &str, extra: serde_json::Value) -> Response {
    let mut error = serde_json::json!({ "code": code });
    if let serde_json::Value::Object(fields) = extra {
        for (key, value) in fields {
            error[key] = value;
        }
    }
    (status, Json(serde_json::json!({ "error": error }))).into_response()
}

/// Probe stored/uploaded bytes and derive the canonical mime plus a
/// short format note for media-block metadata. Unknown bytes stay
/// honest: octet-stream with an `unknown` note.
pub fn probe_mime(bytes: &[u8]) -> (MimeType, String) {
    let result = probe(bytes, None);
    match &result {
        ProbeResult::Identified { format, .. } => {
            let mime = registry()
                .lookup(format)
                .ok()
                .and_then(|descriptor| MimeType::new(&descriptor.mime).ok())
                .unwrap_or_default();
            (mime, format.to_string())
        }
        ProbeResult::LegacyOle2 { .. } => (MimeType::default(), "legacy-ole2".into()),
        ProbeResult::Unknown { .. } => (MimeType::default(), "unknown".into()),
    }
}

/// `POST /s/{sid}/media` — streamed body to a temp file inside the
/// session media store directory (same volume as the
/// content-addressed targets), cap enforced mid-stream, then one
/// read-back for the content hash ([`MediaStore::store_media`]
/// content-addresses + dedups), the M2 probe, and the response.
pub(crate) async fn upload_media(
    State(state): State<Arc<DaemonState>>,
    Path(sid): Path<String>,
    Query(query): Query<MediaQuery>,
    request: Request,
) -> Response {
    let _ = state.session(&sid);
    let session_dir = state.sessions_root.join(&sid);
    let store = match MediaStore::open(&session_dir) {
        Ok(store) => store,
        Err(error) => {
            return typed_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "media_store_open",
                serde_json::json!({ "detail": error.to_string() }),
            );
        }
    };
    let temp = session_dir
        .join(MediaStore::DIR_NAME)
        .join(format!(".upload-{}", std::process::id()));
    let mut file = match tokio::fs::File::create(&temp).await {
        Ok(file) => file,
        Err(error) => {
            return typed_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "media_temp_create",
                serde_json::json!({ "detail": error.to_string() }),
            );
        }
    };
    let cap = state.config.media_upload_cap_bytes;
    let mut streamed: u64 = 0;
    let mut aborted = false;
    let mut stream = request.into_body().into_data_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(bytes) => bytes,
            Err(_) => {
                aborted = true;
                break;
            }
        };
        streamed += chunk.len() as u64;
        if streamed > cap {
            let _ = tokio::fs::remove_file(&temp).await;
            return typed_error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "media_too_large",
                serde_json::json!({ "limit": cap }),
            );
        }
        if file.write_all(&chunk).await.is_err() {
            aborted = true;
            break;
        }
    }
    if aborted {
        let _ = tokio::fs::remove_file(&temp).await;
        return typed_error(
            StatusCode::BAD_REQUEST,
            "upload_aborted",
            serde_json::json!({}),
        );
    }
    if file.flush().await.is_err() || file.sync_all().await.is_err() {
        let _ = tokio::fs::remove_file(&temp).await;
        return typed_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "media_temp_write",
            serde_json::json!({}),
        );
    }
    drop(file);
    let bytes = match tokio::fs::read(&temp).await {
        Ok(bytes) => bytes,
        Err(error) => {
            let _ = tokio::fs::remove_file(&temp).await;
            return typed_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "media_temp_read",
                serde_json::json!({ "detail": error.to_string() }),
            );
        }
    };
    let dedup = store.path_for(&ContentHash::from_bytes(&bytes)).exists();
    let reference = match store.store_media(&bytes) {
        Ok(reference) => reference,
        Err(error) => {
            let _ = tokio::fs::remove_file(&temp).await;
            return typed_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "media_store",
                serde_json::json!({ "detail": error.to_string() }),
            );
        }
    };
    let _ = tokio::fs::remove_file(&temp).await;
    let probed = probe(&bytes, query.filename.as_deref());
    let body = serde_json::json!({
        "media": reference,
        "probe": probed,
        "dedup": dedup,
    });
    (StatusCode::CREATED, Json(body)).into_response()
}

/// `GET /s/{sid}/media/{hash}` — serve stored bytes (token auth via
/// the shared middleware; size-capped; Content-Type from the probed
/// mime) so daemon-served previews never put media into frames.
pub(crate) async fn get_media(
    State(state): State<Arc<DaemonState>>,
    Path((sid, hash)): Path<(String, String)>,
) -> Response {
    let hash = match ContentHash::parse(&hash) {
        Ok(hash) => hash,
        Err(_) => return typed_error(StatusCode::BAD_REQUEST, "bad_hash", serde_json::json!({})),
    };
    let _ = state.session(&sid);
    let session_dir = state.sessions_root.join(&sid);
    let store = match MediaStore::open(&session_dir) {
        Ok(store) => store,
        Err(_) => {
            return typed_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "media_store_open",
                serde_json::json!({}),
            );
        }
    };
    let bytes = match store.load(&hash) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => {
            return typed_error(
                StatusCode::NOT_FOUND,
                "unknown_media",
                serde_json::json!({}),
            );
        }
        Err(error) => {
            return typed_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "media_load",
                serde_json::json!({ "detail": error.to_string() }),
            );
        }
    };
    let cap = state.config.media_upload_cap_bytes;
    if bytes.len() as u64 > cap {
        return typed_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "media_too_large",
            serde_json::json!({ "limit": cap }),
        );
    }
    let (mime, _) = probe_mime(&bytes);
    let content_type = match axum::http::HeaderValue::from_str(mime.as_str()) {
        Ok(value) => value,
        Err(_) => axum::http::HeaderValue::from_static("application/octet-stream"),
    };
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, content_type)],
        bytes,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_mime_maps_png_magic_to_image_png() {
        let (mime, note) = probe_mime(b"\x89PNG\r\n\x1a\nrest");
        assert_eq!(mime.as_str(), "image/png");
        assert_eq!(note, "png");
    }

    #[test]
    fn probe_mime_unknown_bytes_stay_octet_stream() {
        let (mime, note) = probe_mime(b"\x00\x01\x02 nothing known");
        assert_eq!(mime.as_str(), "application/octet-stream");
        assert_eq!(note, "unknown");
    }

    #[test]
    fn probe_mime_legacy_ole2_is_identified_not_parsed() {
        let (mime, note) = probe_mime(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]);
        assert_eq!(note, "legacy-ole2");
        assert_eq!(mime.as_str(), "application/octet-stream");
    }
}
