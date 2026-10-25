//! Media delivery plumbing for provider requests (plan M8): the
//! session-backed projection cache + byte fetcher the adapter consults at
//! request-build time, the native-media payload type carried on
//! [`crate::identity::CompletionRequest`], and the per-family native-MIME
//! allowlist (a DATA table — extending per model never touches the
//! planner).
//!
//! Base64 exists only inside rendered wire requests. The resolved bytes on
//! [`NativeMedia`] are serde-skipped and Debug-redacted so they can never
//! reach snapshots, events, or logs.

use std::fmt;
use std::io;
use std::path::Path;

use austringer_core::event::derived::{DerivedCorrupt, DerivedStore, ProjectionKind};
use austringer_core::event::media::{ContentHash, MediaStore};
use austringer_core::media_delivery::ProjectionCache;
use serde::{Deserialize, Serialize};

/// Producer id under which delivery-inlined projections are cached. The
/// M10 projection pipeline stores under producer-specific ids; the
/// delivery cache consults this canonical one (extend to fallback lists
/// there, not here).
pub const DELIVERY_PROJECTION_PRODUCER: &str = "delivery";

/// Session media context for the delivery planner: the content-addressed
/// media store (byte fetches at request-build time) and the derived
/// projection cache (text inlining for incapable models).
pub struct MediaDelivery {
    store: MediaStore,
    derived: DerivedStore,
}

impl MediaDelivery {
    /// Opens the stores for a session directory
    /// (`<session>/media/` + `<session>/media/derived/`).
    pub fn open(session_dir: &Path) -> io::Result<Self> {
        Ok(Self {
            store: MediaStore::open(session_dir)?,
            derived: DerivedStore::open(session_dir)?,
        })
    }

    /// Loads stored bytes by content hash (`None` when unknown).
    pub fn load_bytes(&self, hash: &ContentHash) -> io::Result<Option<Vec<u8>>> {
        self.store.load(hash)
    }
}

impl ProjectionCache for MediaDelivery {
    fn cached_projection(&self, source: &ContentHash, kind: ProjectionKind) -> Option<String> {
        match self
            .derived
            .load_derived(source, DELIVERY_PROJECTION_PRODUCER, kind)
        {
            Ok(Some(bytes)) => String::from_utf8(bytes).ok(),
            Ok(None) => None,
            Err(DerivedCorrupt::Io(error)) => {
                tracing::warn!(source = %source.to_ref(), kind = %kind, %error, "derived store i/o failure");
                None
            }
            Err(corrupt) => {
                // A corrupt entry degrades to a miss: the block renders
                // its marker and the projection stays queued for repair.
                tracing::warn!(
                    source = %source.to_ref(),
                    kind = %kind,
                    error = %corrupt,
                    "derived projection cache corrupt; queuing re-projection"
                );
                None
            }
        }
    }
}

/// Which native wire shape a media payload takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeKind {
    /// Rendered as the family's image part (image source / image_url /
    /// inline_data).
    Image,
    /// Rendered as the family's audio part (input_audio / inline_data).
    Audio,
}

/// One natively-delivered media payload on a completion request: the
/// typed reference plus the bytes resolved from the media store at
/// request-build time. Serialized snapshots keep the reference fields
/// only — bytes never serialize, and [`fmt::Debug`] prints their length,
/// never their content.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeMedia {
    /// The family wire shape to render.
    pub kind: NativeKind,
    /// Canonical MIME type (`image/png`, `audio/wav`, ...).
    pub mime: String,
    /// Content hash of the resolved bytes (identity + provenance).
    pub hash: ContentHash,
    /// Resolved media bytes. Serde-skipped: they exist ONLY inside the
    /// rendered wire request, never in snapshots, events, or logs.
    #[serde(skip)]
    pub bytes: Vec<u8>,
}

impl fmt::Debug for NativeMedia {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeMedia")
            .field("kind", &self.kind)
            .field("mime", &self.mime)
            .field("hash", &self.hash.to_ref())
            .field("bytes", &format_args!("{}B (redacted)", self.bytes.len()))
            .finish()
    }
}

/// Native-MIME allowlist per provider family (plan M8 DATA table).
/// Extendable per model without planner changes: a model-specific surface
/// widens its family's table, the planner keeps planning capabilities,
/// not dialects. Unknown families (including `compat`) get nothing —
/// fail-closed.
///
/// * anthropic — Messages API `image` source types (no audio input in
///   the M7-cited docs).
/// * openai — `image_url` images + `input_audio` (wav/mp3 only).
/// * google — `inline_data` (broad MIME acceptor; the PDF-via-inline_data
///   row is a NAMED FOLLOW-UP, deliberately not v0.2).
/// * xai — `image_url` images only.
#[must_use]
pub fn native_mimes(family: &str) -> &'static [&'static str] {
    match family {
        "anthropic" => &["image/gif", "image/jpeg", "image/png", "image/webp"],
        "openai" => &[
            "image/gif",
            "image/jpeg",
            "image/png",
            "image/webp",
            "audio/mpeg",
            "audio/wav",
        ],
        "google" => &[
            "image/jpeg",
            "image/png",
            "image/webp",
            "audio/flac",
            "audio/mpeg",
            "audio/ogg",
            "audio/wav",
        ],
        "xai" => &["image/gif", "image/jpeg", "image/png", "image/webp"],
        _ => &[],
    }
}
