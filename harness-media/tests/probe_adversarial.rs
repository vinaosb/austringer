//! M2 adversarial probe corpus: truncated magics, wrong extensions,
//! hint-over-bytes attempts, malformed containers. Every case ends in a
//! TYPED result (`Identified` correct, `LegacyOle2`, or `Unknown`) - never
//! a wrong format, never a panic. Hermetic: no network, no tools.

use harness_media::format::FormatId;
use harness_media::probe::{Confidence, ProbeResult, probe};

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(path).unwrap_or_else(|error| panic!("fixture {name}: {error}"))
}

/// Truncation sweep: every prefix must stay typed and within the legal
/// downgrades (e.g. a truncated geopackage is honestly a bare sqlite).
fn assert_truncations_stay_typed(file: &str, legal: &[FormatId]) {
    let bytes = fixture(file);
    for cut in 0..=bytes.len() {
        let result = probe(&bytes[..cut], None);
        if let ProbeResult::Identified { format, .. } = result {
            assert!(
                legal.contains(&format),
                "{file} truncated to {cut}: illegal {format}"
            );
        }
    }
}

#[test]
fn truncated_magics_stay_typed_or_legally_downgraded() {
    // Given: fixtures whose magic lanes can partially match.
    assert_truncations_stay_typed("png.png", &[FormatId::Png]);
    assert_truncations_stay_typed("jpeg.jpg", &[FormatId::Jpeg]);
    assert_truncations_stay_typed("geopackage.gpkg", &[FormatId::Geopackage, FormatId::Sqlite]);
    assert_truncations_stay_typed("parquet.parquet", &[FormatId::Parquet]);
    assert_truncations_stay_typed("opus.opus", &[FormatId::Opus, FormatId::Ogg]);
    assert_truncations_stay_typed("webm.webm", &[FormatId::Webm]);
    assert_truncations_stay_typed("glb.glb", &[FormatId::Gltf]);
    assert_truncations_stay_typed("docx.docx", &[FormatId::Docx]);
    assert_truncations_stay_typed("svg.svg", &[FormatId::Svg]);
    assert_truncations_stay_typed("kml.kml", &[FormatId::Kml]);
}

#[test]
fn empty_and_one_byte_inputs_probe_typed_unknown() {
    // Given: empty and single-byte inputs across the whole corpus.
    for (file, _, _, _) in [
        ("png.png", "", FormatId::Png, Confidence::Certain),
        ("blend.blend", "", FormatId::Blend, Confidence::Certain),
        ("hdf5.h5", "", FormatId::Hdf5, Confidence::Certain),
    ] {
        let bytes = fixture(file);
        // When/Then: no prefix can identify anything below two bytes.
        assert!(matches!(probe(&[], None), ProbeResult::Unknown { .. }));
        assert!(
            matches!(probe(&bytes[..1], None), ProbeResult::Unknown { .. }),
            "{file}: one byte must not identify"
        );
    }
}

#[test]
fn bytes_win_over_name_hint_when_extensions_lie() {
    // Given: an svg renamed .kml and a png renamed .docx.
    let svg = fixture("svg.svg");
    let png = fixture("png.png");
    // When: probed under the lying hints.
    let svg_result = probe(&svg, Some("photo.kml"));
    let png_result = probe(&png, Some("scan.docx"));
    // Then: bytes decide - svg and png, Certain.
    assert!(matches!(
        svg_result,
        ProbeResult::Identified {
            format: FormatId::Svg,
            confidence: Confidence::Certain,
            ..
        }
    ));
    assert!(matches!(
        png_result,
        ProbeResult::Identified {
            format: FormatId::Png,
            confidence: Confidence::Certain,
            ..
        }
    ));
}

#[test]
fn hint_selects_only_among_fired_candidates_never_adds_one() {
    // Given: markdown whose content fires only the markdown heuristic, and
    // plain JSON without Feature types.
    let md = fixture("markdown.md");
    let json = fixture("json.json");
    // When: hinted as csv / geojson (candidates that did NOT fire).
    let md_result = probe(&md, Some("table.csv"));
    let json_result = probe(&json, Some("map.geojson"));
    // Then: the fired candidates win; the hint cannot create an
    // identification.
    assert!(matches!(
        md_result,
        ProbeResult::Identified {
            format: FormatId::Markdown,
            confidence: Confidence::Likely,
            ..
        }
    ));
    assert!(matches!(
        json_result,
        ProbeResult::Identified {
            format: FormatId::Json,
            confidence: Confidence::Likely,
            ..
        }
    ));
}

#[test]
fn namespace_less_kml_root_needs_hint_to_identify() {
    // Given: a kml root element without the 2.2 namespace declaration.
    let bytes = b"<kml><Document/></kml>";
    // When: probed with and without a .kml name hint.
    let hinted = probe(bytes, Some("map.kml"));
    let unhinted = probe(bytes, None);
    // Then: the hint breaks the tie at Likely; without it, typed Unknown.
    assert!(matches!(
        hinted,
        ProbeResult::Identified {
            format: FormatId::Kml,
            confidence: Confidence::Likely,
            ..
        }
    ));
    assert!(matches!(unhinted, ProbeResult::Unknown { .. }));
}

#[test]
fn ebml_without_doctype_needs_hint_to_disambiguate() {
    // Given: the EBML magic with no resolvable DocType string.
    let bytes = [0x1A, 0x45, 0xDF, 0xA3, 0x9F, 0x00, 0x00];
    // When: probed with and without a .webm name hint.
    let hinted = probe(&bytes, Some("clip.webm"));
    let unhinted = probe(&bytes, None);
    // Then: webm at Likely via hint; typed Unknown without one.
    assert!(matches!(
        hinted,
        ProbeResult::Identified {
            format: FormatId::Webm,
            confidence: Confidence::Likely,
            ..
        }
    ));
    assert!(matches!(unhinted, ProbeResult::Unknown { .. }));
}

#[test]
fn riff_with_unregistered_form_probes_typed_unknown() {
    // Given: a RIFF container whose form is neither WEBP nor WAVE.
    let bytes = b"RIFF\x00\x00\x00\x00AVI ";
    // When/Then: typed Unknown (never a guess, never a panic).
    assert!(matches!(probe(bytes, None), ProbeResult::Unknown { .. }));
}

#[test]
fn zip_without_ooxml_content_types_probes_typed_unknown() {
    // Given: a committed plain zip (no [Content_Types].xml) and a
    // PK-prefixed blob that is not a readable zip at all.
    let plain = fixture("zip-plain.zip");
    let garbage = b"PK\x03\x04not-really-a-zip";
    // When/Then: both stay typed Unknown.
    assert!(matches!(
        probe(&plain, Some("a.zip")),
        ProbeResult::Unknown { .. }
    ));
    assert!(matches!(probe(garbage, None), ProbeResult::Unknown { .. }));
}

#[test]
fn utf16_bom_is_not_an_mp3_sync_word() {
    // Given: UTF-16LE text (FF FE passes the raw MPEG sync mask).
    let bytes = [0xFF, 0xFE, 0x68, 0x00, 0x69, 0x00];
    // When/Then: typed Unknown - the BOM guard outranks the sync word.
    assert!(matches!(probe(&bytes, None), ProbeResult::Unknown { .. }));
}

#[test]
fn all_ff_bytes_fail_the_bitrate_sanity_guard() {
    // Given: eight 0xFF bytes (sync mask passes, bitrate index reserved).
    let bytes = [0xFFu8; 8];
    // When/Then: typed Unknown.
    assert!(matches!(probe(&bytes, None), ProbeResult::Unknown { .. }));
}

#[test]
fn garbage_inputs_never_panic_and_stay_typed() {
    // Given: 256 deterministic pseudo-random byte strings.
    let mut state = 0x5eed_cafe_f00d_d00d_u64;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        state
    };
    for _ in 0..256 {
        let length = (next() >> 33) as usize % 64;
        let bytes: Vec<u8> = (0..length)
            .map(|i| ((next() >> ((i % 8) * 8)) & 0xff) as u8)
            .collect();
        // When/Then: every probe returns a typed variant; the loop
        // completing is the zero-panics proof.
        let _ = probe(&bytes, Some(&format!("blob{length}.bin")));
    }
}
