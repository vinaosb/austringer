//! Shared kit for delivery-planner tests (plan M8): fixture blocks whose
//! expectations derive from the fixture inputs, the four capability
//! presets, and the map-backed projection cache.

#![allow(dead_code)]

use std::collections::BTreeMap;

use austringer_core::event::content::{ContentBlock, ImageBlock, MediaBlock, MimeType};
use austringer_core::event::derived::ProjectionKind;
use austringer_core::event::media::{ContentHash, MediaRef};
use austringer_core::media_delivery::{DeliveryCapabilities, ProjectionCache};

/// Synthetic image fixture bytes (expectations key off these).
pub const IMAGE_BYTES: &[u8] = b"planner-image-fixture-bytes-0001";
/// Synthetic audio fixture bytes.
pub const AUDIO_BYTES: &[u8] = b"planner-audio-fixture-bytes-0001";
/// Synthetic gltf fixture bytes.
pub const GLTF_BYTES: &[u8] = b"planner-gltf-fixture-bytes-0001";
/// Synthetic pdf fixture bytes.
pub const PDF_BYTES: &[u8] = b"planner-pdf-fixture-bytes-00001";

/// A content-addressed reference to fixture bytes.
pub fn media_of(bytes: &[u8]) -> MediaRef {
    MediaRef {
        hash: ContentHash::from_bytes(bytes),
        bytes: bytes.len() as u64,
    }
}

/// A stored-image content block (4x4 png over [`IMAGE_BYTES`]).
pub fn image_block() -> ContentBlock {
    ContentBlock::Image(
        ImageBlock::new("image/png", 8, 6, media_of(IMAGE_BYTES))
            .expect("typed image construction"),
    )
}

/// A generalized media block over `bytes` with optional sorted meta.
pub fn media_block(raw_mime: &str, bytes: &[u8], meta: &[(&str, &str)]) -> ContentBlock {
    let meta: BTreeMap<String, String> = meta
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    ContentBlock::Media(
        MediaBlock::new(
            MimeType::new(raw_mime).expect("valid mime"),
            media_of(bytes),
            meta,
        )
        .expect("bounded meta"),
    )
}

/// A plain text block.
pub fn text_block(text: &str) -> ContentBlock {
    ContentBlock::Text {
        text: text.to_owned(),
    }
}

/// All-native-inputs-off (the fail-closed default).
pub fn text_only() -> DeliveryCapabilities {
    DeliveryCapabilities::default()
}

/// Vision on, audio off.
pub fn vision() -> DeliveryCapabilities {
    DeliveryCapabilities {
        vision: true,
        audio_input: false,
    }
}

/// Audio on, vision off.
pub fn audio() -> DeliveryCapabilities {
    DeliveryCapabilities {
        vision: false,
        audio_input: true,
    }
}

/// Both native inputs on.
pub fn vision_audio() -> DeliveryCapabilities {
    DeliveryCapabilities {
        vision: true,
        audio_input: true,
    }
}

/// Map-backed projection cache: (source hash, kind) -> text.
pub struct MapCache(pub Vec<(ContentHash, ProjectionKind, String)>);

impl ProjectionCache for MapCache {
    fn cached_projection(&self, source: &ContentHash, kind: ProjectionKind) -> Option<String> {
        self.0
            .iter()
            .find(|(hash, cached, _)| hash == source && *cached == kind)
            .map(|(_, _, text)| text.clone())
    }
}

/// The always-missing cache.
pub fn no_cache() -> MapCache {
    MapCache(Vec::new())
}

/// Expected image marker, derived from the fixture input (M3 format).
pub fn image_marker() -> String {
    format!(
        "[image: image/png 8x6 {}]",
        ContentHash::from_bytes(IMAGE_BYTES).short_ref()
    )
}

/// Expected media marker, derived from the fixture input (M3 format).
pub fn media_marker(raw_mime: &str, bytes: &[u8], meta: &[(&str, &str)]) -> String {
    let mut marker = format!(
        "[media: {} {}",
        MimeType::new(raw_mime).expect("valid mime"),
        ContentHash::from_bytes(bytes).short_ref()
    );
    for (key, value) in meta {
        marker.push_str(&format!(" {key}={value}"));
    }
    marker.push(']');
    marker
}
