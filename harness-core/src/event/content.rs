//! Multimodal content blocks: text and out-of-line image references.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::media::MediaRef;

/// Reasons a MIME-type string is not a valid image type.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvalidMimeType {
    /// Empty input.
    #[error("MIME type must not be empty")]
    Empty,
    /// Not an `image/<subtype>` string.
    #[error("not an image MIME type: {0:?}")]
    NotAnImage(String),
    /// Characters outside the RFC token set.
    #[error("invalid characters in MIME type: {0:?}")]
    InvalidCharacters(String),
}

/// An image MIME type (`image/<subtype>`), validated and lowercased at
/// construction so the serialized form is canonical.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct MimeType(String);

impl MimeType {
    /// Validate (and lowercase) an `image/<subtype>` string.
    pub fn new(raw: &str) -> Result<Self, InvalidMimeType> {
        if raw.is_empty() {
            return Err(InvalidMimeType::Empty);
        }
        let lowered = raw.to_ascii_lowercase();
        let valid = lowered.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'/' | b'+' | b'-' | b'.')
        });
        if !valid {
            return Err(InvalidMimeType::InvalidCharacters(raw.to_owned()));
        }
        let (major, sub) = lowered
            .split_once('/')
            .ok_or_else(|| InvalidMimeType::NotAnImage(raw.to_owned()))?;
        if major != "image" || sub.is_empty() {
            return Err(InvalidMimeType::NotAnImage(raw.to_owned()));
        }
        Ok(Self(lowered))
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
}

impl ContentBlock {
    /// Deterministic text-only rendering of this block. Image blocks render
    /// as their typed placeholder marker.
    #[must_use]
    pub fn text_marker(&self) -> String {
        match self {
            Self::Text { text } => text.clone(),
            Self::Image(block) => block.text_marker(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_type_rejects_when_not_image() {
        assert_eq!(MimeType::new("image/png"), Ok(MimeType::png()));
        assert_eq!(MimeType::new("IMAGE/PNG"), Ok(MimeType::png()));
        assert_eq!(MimeType::new(""), Err(InvalidMimeType::Empty));
        assert!(matches!(
            MimeType::new("text/plain"),
            Err(InvalidMimeType::NotAnImage(_))
        ));
        assert!(matches!(
            MimeType::new("image/p ng"),
            Err(InvalidMimeType::InvalidCharacters(_))
        ));
        assert!(matches!(
            MimeType::new("image/"),
            Err(InvalidMimeType::NotAnImage(_))
        ));
    }
}
