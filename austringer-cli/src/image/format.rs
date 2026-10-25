//! Client-side image-format sniffing: magic bytes plus dimension
//! parsing for the four formats `--image` accepts (PNG, JPEG, GIF,
//! WebP). Pure, allocation-free on the error path, no image decoding
//! — only the header fields the typed marker contract needs.

use austringer_core::event::MimeType;

/// A successfully sniffed header.
pub(super) struct Sniffed {
    /// Sniffed image MIME type.
    pub(super) mime: MimeType,
    /// Pixel width.
    pub(super) width: u32,
    /// Pixel height.
    pub(super) height: u32,
}

/// Sniff magic bytes and parse dimensions.
pub(super) fn sniff(bytes: &[u8]) -> Option<Sniffed> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return sniff_png(bytes);
    }
    if bytes.starts_with(b"GIF8") {
        return sniff_gif(bytes);
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        return sniff_jpeg(bytes);
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return sniff_webp(bytes);
    }
    None
}

fn sniff_png(bytes: &[u8]) -> Option<Sniffed> {
    // IHDR is the mandatory first chunk: length(4) + "IHDR" then
    // BE width/height at offset 16.
    if bytes.len() < 24 || &bytes[12..16] != b"IHDR" {
        return None;
    }
    Some(Sniffed {
        mime: MimeType::png(),
        width: u32::from_be_bytes(bytes[16..20].try_into().ok()?),
        height: u32::from_be_bytes(bytes[20..24].try_into().ok()?),
    })
}

fn sniff_gif(bytes: &[u8]) -> Option<Sniffed> {
    // "GIF87a"/"GIF89a" then LE logical screen size.
    if bytes.len() < 10 {
        return None;
    }
    Some(Sniffed {
        mime: MimeType::gif(),
        width: u16::from_le_bytes(bytes[6..8].try_into().ok()?).into(),
        height: u16::from_le_bytes(bytes[8..10].try_into().ok()?).into(),
    })
}

fn sniff_jpeg(bytes: &[u8]) -> Option<Sniffed> {
    // Walk segments to the first SOFn (frame header): dimensions are
    // BE at segment start + 5..9.
    let mut at = 2usize;
    while at + 9 < bytes.len() {
        if bytes[at] != 0xFF {
            at += 1;
            continue;
        }
        let marker = bytes[at + 1];
        if (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
            let (height, width) = (
                u16::from_be_bytes(bytes[at + 5..at + 7].try_into().ok()?),
                u16::from_be_bytes(bytes[at + 7..at + 9].try_into().ok()?),
            );
            return Some(Sniffed {
                mime: MimeType::jpeg(),
                width: width.into(),
                height: height.into(),
            });
        }
        if matches!(marker, 0xD8 | 0x01 | 0xD0..=0xD7) {
            at += 2;
            continue;
        }
        let length = u16::from_be_bytes(bytes[at + 2..at + 4].try_into().ok()?);
        at += 2 + usize::from(length);
    }
    None
}

fn sniff_webp(bytes: &[u8]) -> Option<Sniffed> {
    match bytes.get(12..16)? {
        b"VP8X" => {
            // Extended: 24-bit LE canvas size (14 bits used per
            // dimension, stored as value-1) at chunk start + 8.
            let at = 20usize;
            if bytes.len() < at + 6 {
                return None;
            }
            let b = |i: usize| u32::from(bytes[at + i]);
            let width = 1 + (((b(0) << 16) | (b(1) << 8) | b(2)) & 0x003F_FFFF);
            let height = 1 + (((b(3) << 16) | (b(4) << 8) | b(5)) & 0x003F_FFFF);
            Some(Sniffed {
                mime: MimeType::webp(),
                width,
                height,
            })
        }
        b"VP8L" => {
            // Lossless: signature 0x2F then 14-bit dimensions packed
            // in the next 4 bytes (stored as value-1).
            let at = 21usize;
            if bytes.len() < at + 4 || bytes[20] != 0x2F {
                return None;
            }
            let bits = u32::from_le_bytes(bytes[at..at + 4].try_into().ok()?);
            Some(Sniffed {
                mime: MimeType::webp(),
                width: (bits & 0x3FFF) + 1,
                height: ((bits >> 14) & 0x3FFF) + 1,
            })
        }
        b"VP8 " => {
            // Lossy: 3-byte frame tag then sync code 0x9D 0x01 0x2A,
            // then LE14 dimensions.
            let at = 26usize;
            if bytes.len() < at + 4 {
                return None;
            }
            let bits = u16::from_le_bytes(bytes[at..at + 2].try_into().ok()?);
            Some(Sniffed {
                mime: MimeType::webp(),
                width: u32::from(bits & 0x3FFF),
                height: u32::from(bits >> 14),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_dimensions_and_mime_are_parsed() {
        let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        bytes.extend_from_slice(&13u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&3u32.to_be_bytes());
        bytes.extend_from_slice(&2u32.to_be_bytes());
        let sniffed = sniff(&bytes).unwrap_or_else(|| panic!("png must sniff"));
        assert_eq!(sniffed.width, 3);
        assert_eq!(sniffed.height, 2);
        assert_eq!(sniffed.mime.as_str(), "image/png");
    }

    #[test]
    fn gif_dimensions_are_little_endian() {
        let mut bytes = b"GIF89a".to_vec();
        bytes.extend_from_slice(&7u16.to_le_bytes());
        bytes.extend_from_slice(&5u16.to_le_bytes());
        let sniffed = sniff(&bytes).unwrap_or_else(|| panic!("gif must sniff"));
        assert_eq!(sniffed.width, 7);
        assert_eq!(sniffed.height, 5);
    }

    #[test]
    fn jpeg_frame_header_is_found() {
        let mut bytes = vec![0xFF, 0xD8];
        bytes.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x0B, 0x08]);
        bytes.extend_from_slice(&9u16.to_be_bytes());
        bytes.extend_from_slice(&4u16.to_be_bytes());
        bytes.extend_from_slice(&[0x03, 0x01, 0x11, 0x00]);
        let sniffed = sniff(&bytes).unwrap_or_else(|| panic!("jpeg must sniff"));
        assert_eq!(sniffed.width, 4);
        assert_eq!(sniffed.height, 9);
        assert_eq!(sniffed.mime.as_str(), "image/jpeg");
    }

    #[test]
    fn webp_lossless_dimensions_are_parsed() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&24u32.to_le_bytes());
        bytes.extend_from_slice(b"WEBPVP8L");
        bytes.extend_from_slice(&5u32.to_le_bytes());
        bytes.push(0x2F);
        // width-1 = 9, height-1 = 6 packed as 14-bit LE pairs.
        let bits: u32 = 9 | (6 << 14);
        bytes.extend_from_slice(&bits.to_le_bytes());
        let sniffed = sniff(&bytes).unwrap_or_else(|| panic!("webp must sniff"));
        assert_eq!(sniffed.width, 10);
        assert_eq!(sniffed.height, 7);
    }

    #[test]
    fn unknown_bytes_do_not_sniff() {
        assert!(sniff(b"plain text, not an image").is_none());
        assert!(sniff(&[]).is_none());
    }
}
