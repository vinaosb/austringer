//! `--image` attachment handling (plan todo 39, multimodal turns):
//! client-side validation — the file must exist, be a regular file, and
//! sniff as a supported raster format (PNG/JPEG/GIF/WebP) with parseable
//! dimensions (see [`format`]). Validated images render into the
//! session as the typed harness-core image marker (out-of-line media
//! reference: mime, dimensions, blake3 hash — never the bytes); when
//! the daemon's data dir is reachable the bytes are also stored
//! content-addressed into the session's media store so the session
//! owns the content.

mod format;

use std::path::{Path, PathBuf};

use harness_core::event::{ContentHash, ImageBlock, MediaRef, MediaStore, MimeType};

use crate::error::CliError;

/// One validated attachment, bytes resident (bounded by the CLI's own
/// argument surface — these are files the human explicitly named).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedImage {
    /// Sniffed image MIME type.
    pub mime: MimeType,
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// The image bytes.
    pub bytes: Vec<u8>,
    /// Where they came from (error reporting only).
    pub source: PathBuf,
}

impl PreparedImage {
    /// The harness-core typed marker line for this image.
    #[must_use]
    pub fn marker(&self) -> String {
        image_block(self).text_marker()
    }
}

/// Validate every `--image` path. `skip_missing` silently drops paths
/// that do not exist (REPL `/image` convenience); every path that DOES
/// exist must be a regular, sniffable image or the whole call fails
/// typed before any daemon contact.
pub fn attachments(paths: &[PathBuf], skip_missing: bool) -> Result<Vec<PreparedImage>, CliError> {
    let mut out = Vec::new();
    for path in paths {
        let meta = match std::fs::metadata(path) {
            Ok(meta) => meta,
            Err(_) if skip_missing => continue,
            Err(error) => {
                return Err(CliError::Image {
                    detail: format!("{}: {error}", path.display()),
                });
            }
        };
        if !meta.is_file() {
            return Err(CliError::Image {
                detail: format!("{} is not a regular image file", path.display()),
            });
        }
        let bytes = std::fs::read(path).map_err(|error| CliError::Image {
            detail: format!("{}: {error}", path.display()),
        })?;
        let sniffed = format::sniff(&bytes).ok_or_else(|| CliError::Image {
            detail: format!(
                "{} is not a recognized image (expected png, jpeg, gif, or webp)",
                path.display()
            ),
        })?;
        out.push(PreparedImage {
            mime: sniffed.mime,
            width: sniffed.width,
            height: sniffed.height,
            bytes,
            source: path.clone(),
        });
    }
    Ok(out)
}

/// Compose the submit text: the prompt plus one typed marker line per
/// image (the deterministic text-only rendering harness-core defines
/// for image blocks).
#[must_use]
pub fn submit_text(prompt: &str, images: &[PreparedImage]) -> String {
    if images.is_empty() {
        return prompt.to_owned();
    }
    let mut text = String::from(prompt);
    for image in images {
        text.push('\n');
        text.push_str(&image.marker());
    }
    text
}

/// Store validated images content-addressed into a session's media dir
/// (`{data}/sessions/{sid}/media/`). Failure is typed but non-fatal to
/// turn submission — the marker already references the content hash.
pub fn store_into(session_dir: &Path, images: &[PreparedImage]) -> Result<(), CliError> {
    let store = MediaStore::open(session_dir).map_err(|error| CliError::Io {
        detail: error.to_string(),
    })?;
    for image in images {
        store
            .store(&image.mime, image.width, image.height, &image.bytes)
            .map_err(|error| CliError::Io {
                detail: error.to_string(),
            })?;
    }
    Ok(())
}

fn image_block(image: &PreparedImage) -> ImageBlock {
    ImageBlock {
        mime: image.mime.clone(),
        width: image.width,
        height: image.height,
        media: MediaRef {
            hash: ContentHash::from_bytes(&image.bytes),
            bytes: image.bytes.len() as u64,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_temp(name: &str, bytes: &[u8]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap_or_default();
        (dir, path)
    }

    /// Minimal PNG header: signature + IHDR declaring `width`x`height`.
    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        bytes.extend_from_slice(&13u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 0, 0, 0, 0]);
        bytes
    }

    #[test]
    fn validated_png_carries_dimensions_and_mime() {
        let (_dir, path) = write_temp("x.png", &png_header(3, 2));
        let images = attachments(std::slice::from_ref(&path), false).unwrap_or_default();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].width, 3);
        assert_eq!(images[0].height, 2);
        assert_eq!(images[0].mime.as_str(), "image/png");
    }

    #[test]
    fn non_image_files_are_rejected_typed() {
        let (_dir, path) = write_temp("notes.txt", b"plain text, not an image");
        let error = attachments(&[path], false).unwrap_err();
        assert!(error.to_string().contains("image"), "{error}");
    }

    #[test]
    fn directories_are_rejected_typed() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
        let error = attachments(&[dir.path().to_path_buf()], false).unwrap_err();
        assert!(error.to_string().contains("image"), "{error}");
    }

    #[test]
    fn missing_files_error_unless_skipped() {
        let error = attachments(&[PathBuf::from("definitely-not-here.png")], false).unwrap_err();
        assert!(error.to_string().contains("image"), "{error}");
        assert!(
            attachments(&[PathBuf::from("definitely-not-here.png")], true)
                .unwrap_or_default()
                .is_empty()
        );
    }

    #[test]
    fn marker_line_is_the_typed_harness_core_form() {
        let images = attachments(&[PathBuf::from("x")], true).unwrap_or_default();
        assert_eq!(submit_text("hi", &images), "hi");
        let (_dir, path) = write_temp("x.png", &png_header(2, 3));
        let images = attachments(&[path], false).unwrap_or_default();
        let text = submit_text("hi", &images);
        assert!(
            text.starts_with("hi\n[image: image/png 2x3 blake3:"),
            "{text}"
        );
    }

    #[test]
    fn store_into_writes_the_content_addressed_bytes() {
        let session = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
        let (_dir, path) = write_temp("x.png", &png_header(1, 1));
        let images = attachments(&[path], false).unwrap_or_default();
        store_into(session.path(), &images).unwrap_or_default();
        let store = MediaStore::open(session.path()).unwrap_or_else(|e| panic!("open: {e}"));
        let hash = ContentHash::from_bytes(&images[0].bytes);
        assert!(store.load(&hash).unwrap_or_default().is_some());
    }
}
