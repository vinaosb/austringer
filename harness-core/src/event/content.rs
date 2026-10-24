//! Multimodal content blocks: text, out-of-line image references, and
//! generalized media references (any `major/subtype` MIME type).

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::media::MediaRef;

/// Maximum number of metadata entries on a media block.
pub const MAX_META_ENTRIES: usize = 16;
/// Maximum byte length of each metadata key and value.
pub const MAX_META_BYTES: usize = 128;

/// Reasons a MIME-type string is not a valid type.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvalidMimeType {
    /// Empty input.
    #[error("MIME type must not be empty")]
    Empty,
    /// Not a `major/subtype` pair of non-empty valid tokens.
    #[error("invalid MIME structure (expected major/subtype): {0:?}")]
    InvalidStructure(String),
    /// Not an `image/<subtype>` string (the [`MimeType::image`] boundary).
    #[error("not an image MIME type: {0:?}")]
    NotImage(String),
    /// Characters outside the RFC token set.
    #[error("invalid characters in MIME type: {0:?}")]
    InvalidCharacters(String),
}

/// A MIME type (`major/subtype`), validated and lowercased at construction
/// so the serialized form is canonical. Any major type is accepted; the
/// image-only rule lives at the [`MimeType::image`] / [`ImageBlock::new`]
/// boundary.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct MimeType(String);

impl MimeType {
    /// Validate (and lowercase) any `major/subtype` string; both parts
    /// must be non-empty tokens over `[a-z0-9+\-.]` with exactly one `/`.
    pub fn new(raw: &str) -> Result<Self, InvalidMimeType> {
        if raw.is_empty() {
            return Err(InvalidMimeType::Empty);
        }
        let lowered = raw.to_ascii_lowercase();
        let valid_chars = lowered.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'/' | b'+' | b'-' | b'.')
        });
        if !valid_chars {
            return Err(InvalidMimeType::InvalidCharacters(raw.to_owned()));
        }
        let Some((major, sub)) = lowered.split_once('/') else {
            return Err(InvalidMimeType::InvalidStructure(raw.to_owned()));
        };
        if major.is_empty() || sub.is_empty() || sub.contains('/') {
            return Err(InvalidMimeType::InvalidStructure(raw.to_owned()));
        }
        Ok(Self(lowered))
    }

    /// Validate (and lowercase) an `image/<subtype>` string, rejecting
    /// every other major type with a typed error so a widened MIME type
    /// can never land inside an [`ImageBlock`].
    pub fn image(raw: &str) -> Result<Self, InvalidMimeType> {
        let mime = Self::new(raw)?;
        if mime.is_image() {
            Ok(mime)
        } else {
            Err(InvalidMimeType::NotImage(raw.to_owned()))
        }
    }

    /// True when the major type is exactly `image`.
    #[must_use]
    pub fn is_image(&self) -> bool {
        self.0.starts_with("image/")
    }

    /// `image/png`.
    #[must_use]
    pub fn png() -> Self {
        Self("image/png".to_owned())
    }

    /// `image/jpeg`.
    #[must_use]
    pub fn jpeg() -> Self {
        Self("image/jpeg".to_owned())
    }

    /// `image/gif`.
    #[must_use]
    pub fn gif() -> Self {
        Self("image/gif".to_owned())
    }

    /// `image/webp`.
    #[must_use]
    pub fn webp() -> Self {
        Self("image/webp".to_owned())
    }

    /// The canonical (lowercase) MIME string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MimeType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Default for MimeType {
    /// `application/octet-stream` — the RFC 2046 default for unknown
    /// bytes (the honest fallback for unidentified media).
    fn default() -> Self {
        Self("application/octet-stream".to_owned())
    }
}

impl From<MimeType> for String {
    fn from(mime: MimeType) -> Self {
        mime.0
    }
}

impl TryFrom<String> for MimeType {
    type Error = InvalidMimeType;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl FromStr for MimeType {
    type Err = InvalidMimeType;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

/// An image block: typed metadata plus an out-of-line content reference.
///
/// The image bytes are never serialized inline (no base64 in the JSONL);
/// they live at `<session>/media/<hash>` and the event carries only
/// [`MediaRef`]. Text-only consumers render [`ImageBlock::text_marker`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageBlock {
    /// Image MIME type (e.g. `image/png`).
    pub mime: MimeType,
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// Out-of-line storage reference (content hash + byte length).
    pub media: MediaRef,
}

impl ImageBlock {
    /// Typed image-only construction: the MIME string must be
    /// `image/<subtype>` ([`MimeType::image`] boundary).
    pub fn new(
        raw_mime: &str,
        width: u32,
        height: u32,
        media: MediaRef,
    ) -> Result<Self, InvalidMimeType> {
        Ok(Self {
            mime: MimeType::image(raw_mime)?,
            width,
            height,
            media,
        })
    }

    /// Typed placeholder marker for text-only consumers: deterministic,
    /// carries the MIME type, dimensions, and a short content-hash prefix —
    /// never the image bytes.
    #[must_use]
    pub fn text_marker(&self) -> String {
        format!(
            "[image: {} {}x{} {}]",
            self.mime,
            self.width,
            self.height,
            self.media.hash.short_ref()
        )
    }
}

/// Reasons media-block metadata fails the construction bounds.
///
/// Construction is the trust boundary: serde stays permissive so old
/// session logs always replay; oversized metadata is rejected here.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvalidMediaMeta {
    /// More than [`MAX_META_ENTRIES`] entries.
    #[error("media block metadata must have at most {MAX_META_ENTRIES} entries: {0}")]
    TooManyEntries(usize),
    /// A key longer than [`MAX_META_BYTES`] bytes.
    #[error("media block metadata key must be at most {MAX_META_BYTES} bytes: {0:?}")]
    KeyTooLong(String),
    /// A value longer than [`MAX_META_BYTES`] bytes.
    #[error("media block metadata value must be at most {MAX_META_BYTES} bytes: {0:?}")]
    ValueTooLong(String),
}

/// A generalized media block: any MIME type plus an out-of-line content
/// reference and bounded metadata (`format` and similar plain string
/// facts; the `FormatId` mapping lives in harness-media, never here).
///
/// Like image blocks, the bytes are never serialized inline; the `meta`
/// map is a `BTreeMap` so keys serialize sorted (serde stability).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaBlock {
    /// Media MIME type (e.g. `audio/wav`, `model/gltf+json`).
    pub mime: MimeType,
    /// Out-of-line storage reference (content hash + byte length).
    pub media: MediaRef,
    /// Bounded metadata; keys serialize sorted.
    pub meta: BTreeMap<String, String>,
}

impl MediaBlock {
    /// Validate the metadata bounds: at most [`MAX_META_ENTRIES`] entries,
    /// each key and value at most [`MAX_META_BYTES`] bytes.
    pub fn new(
        mime: MimeType,
        media: MediaRef,
        meta: BTreeMap<String, String>,
    ) -> Result<Self, InvalidMediaMeta> {
        if meta.len() > MAX_META_ENTRIES {
            return Err(InvalidMediaMeta::TooManyEntries(meta.len()));
        }
        for (key, value) in &meta {
            if key.len() > MAX_META_BYTES {
                return Err(InvalidMediaMeta::KeyTooLong(key.clone()));
            }
            if value.len() > MAX_META_BYTES {
                return Err(InvalidMediaMeta::ValueTooLong(value.clone()));
            }
        }
        Ok(Self { mime, media, meta })
    }

    /// Typed placeholder marker for text-only consumers: deterministic,
    /// carries the MIME type, a short content-hash prefix, and the sorted
    /// `k=v` metadata — never the media bytes.
    #[must_use]
    pub fn text_marker(&self) -> String {
        let mut marker = format!("[media: {} {}", self.mime, self.media.hash.short_ref());
        for (key, value) in &self.meta {
            marker.push_str(&format!(" {key}={value}"));
        }
        marker.push(']');
        marker
    }
}

/// One block of multimodal content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text.
    Text {
        /// The text.
        text: String,
    },
    /// An image, stored out-of-line under `<session>/media/`.
    Image(ImageBlock),
    /// Any other media, stored out-of-line under `<session>/media/`.
    Media(MediaBlock),
}

impl ContentBlock {
    /// Deterministic text-only rendering of this block. Image and media
    /// blocks render as their typed placeholder markers.
    #[must_use]
    pub fn text_marker(&self) -> String {
        match self {
            Self::Text { text } => text.clone(),
            Self::Image(block) => block.text_marker(),
            Self::Media(block) => block.text_marker(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_type_validates_any_major_but_only_canonical_tokens() {
        assert_eq!(MimeType::new("image/png"), Ok(MimeType::png()));
        assert_eq!(
            MimeType::new("AUDIO/WAV").as_ref().map(MimeType::as_str),
            Ok("audio/wav")
        );
        assert_eq!(MimeType::new(""), Err(InvalidMimeType::Empty));
        assert_eq!(
            MimeType::new("text/plain").as_ref().map(MimeType::as_str),
            Ok("text/plain")
        );
        assert!(matches!(
            MimeType::new("image/"),
            Err(InvalidMimeType::InvalidStructure(_))
        ));
        assert!(matches!(
            MimeType::new("a/b/c"),
            Err(InvalidMimeType::InvalidStructure(_))
        ));
        assert!(matches!(
            MimeType::new("image/p ng"),
            Err(InvalidMimeType::InvalidCharacters(_))
        ));
    }
}
