//! The GLB container walk (plan M12 references: "magic + version + chunk
//! walk is trivially parseable"): a 12-byte header, the mandatory JSON
//! chunk, and the optional BIN chunk - with every failure carrying the
//! BYTE OFFSET of the offending element (surfaced through the typed
//! `ParseFailure`).

use crate::error::MediaError;

/// The GLB container split: JSON chunk slice + BIN chunk length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlbChunks<'a> {
    /// The JSON chunk payload.
    pub json: &'a [u8],
    /// The BIN chunk length in bytes (chunk padding included).
    pub bin_len: u64,
}

/// Why a GLB container walk failed, with the byte offset of the offending
/// element (surfaces through [`MediaError::ParseFailure`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum GlbFormatError {
    /// Fewer than 12 bytes: no complete header.
    #[error("glb header truncated at byte 0: fewer than 12 bytes")]
    HeaderTruncated,
    /// The 4-byte magic is not `glTF`.
    #[error("glb magic mismatch at byte {offset}")]
    Magic {
        /// Offset of the magic field.
        offset: usize,
    },
    /// Header version is not 2.
    #[error("glb version {version} at byte {offset}: only 2 is supported")]
    Version {
        /// Offset of the version field.
        offset: usize,
        /// The unsupported version value.
        version: u32,
    },
    /// Declared total length exceeds the bytes present.
    #[error("glb length at byte {offset} declares {declared} bytes but only {actual} present")]
    Length {
        /// Offset of the length field.
        offset: usize,
        /// Declared total length.
        declared: usize,
        /// Bytes actually present.
        actual: usize,
    },
    /// A chunk header is incomplete.
    #[error("glb chunk header truncated at byte {offset}")]
    ChunkHeader {
        /// Offset of the chunk header.
        offset: usize,
    },
    /// The first chunk is not the JSON chunk.
    #[error("glb first chunk at byte {offset} is not the JSON chunk")]
    JsonMissing {
        /// Offset of the first chunk header.
        offset: usize,
    },
    /// A chunk declares more payload than the bytes hold.
    #[error("glb chunk at byte {offset} declares {declared} payload bytes but only {actual} present")]
    ChunkLength {
        /// Offset of the chunk header.
        offset: usize,
        /// Declared payload length.
        declared: usize,
        /// Bytes actually present.
        actual: usize,
    },
}

impl GlbFormatError {
    /// Byte offset of the offending container element.
    #[must_use]
    pub fn offset(self) -> usize {
        match self {
            Self::HeaderTruncated => 0,
            Self::Magic { offset }
            | Self::Version { offset, .. }
            | Self::Length { offset, .. }
            | Self::ChunkHeader { offset }
            | Self::JsonMissing { offset }
            | Self::ChunkLength { offset, .. } => offset,
        }
    }
}

impl From<GlbFormatError> for MediaError {
    fn from(_: GlbFormatError) -> Self {
        MediaError::ParseFailure
    }
}

const CHUNK_JSON: u32 = 0x4E4F_534A;
const CHUNK_BIN: u32 = 0x004E_4942;
const GLB_MAGIC: u32 = 0x4654_6C67;

fn u32le(bytes: &[u8], offset: usize) -> Option<u32> {
    let raw = bytes.get(offset..offset.checked_add(4)?)?;
    Some(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

/// Walk the GLB container: 12-byte header, JSON chunk, optional BIN chunk
/// (unknown chunk types are skipped, as the spec requires of readers).
pub fn glb_chunks(bytes: &[u8]) -> Result<GlbChunks<'_>, GlbFormatError> {
    if bytes.len() < 12 {
        return Err(GlbFormatError::HeaderTruncated);
    }
    if u32le(bytes, 0) != Some(GLB_MAGIC) {
        return Err(GlbFormatError::Magic { offset: 0 });
    }
    match u32le(bytes, 4) {
        Some(2) => {}
        Some(version) => return Err(GlbFormatError::Version { offset: 4, version }),
        None => return Err(GlbFormatError::HeaderTruncated),
    }
    if let Some(declared) = u32le(bytes, 8)
        && declared as usize > bytes.len()
    {
        return Err(GlbFormatError::Length {
            offset: 8,
            declared: declared as usize,
            actual: bytes.len(),
        });
    }
    let json_len = u32le(bytes, 12).ok_or(GlbFormatError::ChunkHeader { offset: 12 })? as usize;
    if u32le(bytes, 16) != Some(CHUNK_JSON) {
        return Err(GlbFormatError::JsonMissing { offset: 12 });
    }
    let available = bytes.len().saturating_sub(20);
    let json_end = 20usize
        .checked_add(json_len)
        .filter(|end| *end <= bytes.len())
        .ok_or(GlbFormatError::ChunkLength {
            offset: 12,
            declared: json_len,
            actual: available,
        })?;
    let json = bytes.get(20..json_end).ok_or(GlbFormatError::ChunkLength {
        offset: 12,
        declared: json_len,
        actual: available,
    })?;
    let mut cursor = json_end;
    let mut bin_len = None;
    while let (Some(length), Some(kind)) = (u32le(bytes, cursor), u32le(bytes, cursor + 4)) {
        let payload = cursor + 8;
        let end = payload
            .checked_add(length as usize)
            .filter(|end| *end <= bytes.len())
            .ok_or(GlbFormatError::ChunkLength {
                offset: cursor,
                declared: length as usize,
                actual: bytes.len().saturating_sub(payload),
            })?;
        if kind == CHUNK_BIN && bin_len.is_none() {
            bin_len = Some(u64::from(length));
        }
        cursor = end;
    }
    Ok(GlbChunks {
        json,
        bin_len: bin_len.unwrap_or(0),
    })
}

