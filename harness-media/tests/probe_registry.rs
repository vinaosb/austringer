//! M2 probe registry wiring: all 34 registered formats carry a descriptor
//! with probe Supported and honest typed-Unsupported reasons for the
//! operations that arrive with the adapter todos (M11+). Hermetic.

use harness_media::descriptor::{OpSupport, Operation};
use harness_media::format::FormatId;
use harness_media::probe;

#[test]
fn registry_covers_all_34_formats_with_probe_supported() {
    // Given: the M2 probe registry.
    let registry = probe::registry();
    // When: every registered format is looked up.
    for id in FormatId::ALL {
        let descriptor = registry
            .lookup(id)
            .unwrap_or_else(|error| panic!("{id}: {error}"));
        // Then: probe is Supported, identity is populated, and the other
        // three operations are honestly Unsupported with non-empty
        // reasons.
        assert!(
            matches!(
                descriptor.ops.get(&Operation::Probe),
                Some(OpSupport::Supported)
            ),
            "{id}: probe must be Supported"
        );
        assert!(!descriptor.mime.is_empty(), "{id}: empty mime");
        assert!(!descriptor.extensions.is_empty(), "{id}: no extensions");
        for op in [Operation::Extract, Operation::Generate, Operation::Verify] {
            // M11: the svg adapter flips its claims to Supported (paired
            // with the executor bindings under the same feature).
            if cfg!(feature = "svg") && *id == FormatId::Svg {
                continue;
            }
            // M12: the gltf adapter flips its claims to Supported (paired
            // with the executor bindings under the same feature).
            if cfg!(feature = "gltf") && *id == FormatId::Gltf {
                continue;
            }
            // M13: the pdf adapter flips its claims to Supported (same
            // feature-paired pattern).
            if cfg!(feature = "pdf") && *id == FormatId::Pdf {
                continue;
            }
            // M14: the audio adapters flip their claims (wav/flac fully;
            // mp3/ogg extract-only - encode redirects to M17, so no
            // writer exists to verify).
            if cfg!(feature = "audio") {
                match (id, op) {
                    (FormatId::Wav | FormatId::Flac, _) => continue,
                    (FormatId::Mp3 | FormatId::Ogg, Operation::Extract) => continue,
                    _ => {}
                }
            }
            // M30: the text-substrate adapters flip their claims to
            // Supported (same feature-paired pattern).
            if cfg!(feature = "text")
                && matches!(
                    id,
                    FormatId::Markdown
                        | FormatId::Html
                        | FormatId::Json
                        | FormatId::Csv
                        | FormatId::Toml
                )
            {
                continue;
            }
            // M28: the image-family adapters flip their claims to
            // Supported (png/jpeg/webp fully; jxl extract-only; avif
            // under its own feature).
            if cfg!(feature = "image")
                && matches!(id, FormatId::Png | FormatId::Jpeg | FormatId::Webp)
            {
                continue;
            }
            if cfg!(feature = "image") && *id == FormatId::Jxl && op == Operation::Extract {
                continue;
            }
            if cfg!(feature = "avif") && *id == FormatId::Avif {
                continue;
            }
            match descriptor.ops.get(&op) {
                Some(OpSupport::Unsupported { reason }) => {
                    assert!(!reason.is_empty(), "{id} {op}: empty reason");
                }
                other => panic!("{id} {op}: expected Unsupported, got {other:?}"),
            }
        }
    }
}

#[test]
fn registry_extensions_are_lowercase_and_dot_free() {
    // Given: the registry.
    let registry = probe::registry();
    // When/Then: every extension is the documented wire shape.
    for id in FormatId::ALL {
        let descriptor = registry.lookup(id).unwrap();
        for extension in &descriptor.extensions {
            assert!(
                extension
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
                "{id}: extension {extension} must be lowercase and dot-free"
            );
        }
    }
}

#[test]
fn registry_rejects_duplicate_registration() {
    // Given: the probe registry and a second descriptor for one format.
    let mut registry = probe::registry();
    let mut duplicate = registry.lookup(&FormatId::Png).unwrap().clone();
    duplicate.mime = "application/x-duplicate".into();
    // When/Then: registration is refused and the original is kept.
    assert!(!registry.register(duplicate));
    assert_eq!(registry.lookup(&FormatId::Png).unwrap().mime, "image/png");
}
