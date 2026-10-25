//! M2 probe fixture corpus: every one of the 34 registered formats is
//! identified from its committed minimal header fixture (each <=64 KB).
//! Magic-lane fixtures probe Certain from bytes alone; heuristic-lane
//! fixtures (markdown/html/csv/toml/json/geojson) probe Likely with the
//! CORRECT format - never a guess. Hermetic: no network, no tools.

use austringer_media::format::FormatId;
use austringer_media::probe::{Confidence, ProbeResult, probe};

/// Reads a committed fixture (binary or text) into memory.
fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(path).unwrap_or_else(|error| panic!("fixture {name}: {error}"))
}

/// (file, name hint, expected format, expected confidence).
const CORPUS: &[(&str, &str, FormatId, Confidence)] = &[
    ("png.png", "png.png", FormatId::Png, Confidence::Certain),
    ("jpeg.jpg", "jpeg.jpg", FormatId::Jpeg, Confidence::Certain),
    ("gif.gif", "gif.gif", FormatId::Gif, Confidence::Certain),
    (
        "webp.webp",
        "webp.webp",
        FormatId::Webp,
        Confidence::Certain,
    ),
    (
        "avif.avif",
        "avif.avif",
        FormatId::Avif,
        Confidence::Certain,
    ),
    ("jxl.jxl", "jxl.jxl", FormatId::Jxl, Confidence::Certain),
    ("svg.svg", "svg.svg", FormatId::Svg, Confidence::Certain),
    (
        "gltf.gltf",
        "gltf.gltf",
        FormatId::Gltf,
        Confidence::Certain,
    ),
    ("glb.glb", "glb.glb", FormatId::Gltf, Confidence::Certain),
    (
        "blend.blend",
        "blend.blend",
        FormatId::Blend,
        Confidence::Certain,
    ),
    ("pdf.pdf", "pdf.pdf", FormatId::Pdf, Confidence::Certain),
    ("wav.wav", "wav.wav", FormatId::Wav, Confidence::Certain),
    (
        "flac.flac",
        "flac.flac",
        FormatId::Flac,
        Confidence::Certain,
    ),
    (
        "opus.opus",
        "opus.opus",
        FormatId::Opus,
        Confidence::Certain,
    ),
    ("mp3.mp3", "mp3.mp3", FormatId::Mp3, Confidence::Certain),
    ("ogg.ogg", "ogg.ogg", FormatId::Ogg, Confidence::Certain),
    (
        "webm.webm",
        "webm.webm",
        FormatId::Webm,
        Confidence::Certain,
    ),
    ("mkv.mkv", "mkv.mkv", FormatId::Mkv, Confidence::Certain),
    (
        "docx.docx",
        "docx.docx",
        FormatId::Docx,
        Confidence::Certain,
    ),
    (
        "xlsx.xlsx",
        "xlsx.xlsx",
        FormatId::Xlsx,
        Confidence::Certain,
    ),
    (
        "pptx.pptx",
        "pptx.pptx",
        FormatId::Pptx,
        Confidence::Certain,
    ),
    (
        "markdown.md",
        "markdown.md",
        FormatId::Markdown,
        Confidence::Likely,
    ),
    ("html.html", "html.html", FormatId::Html, Confidence::Likely),
    ("json.json", "json.json", FormatId::Json, Confidence::Likely),
    ("csv.csv", "csv.csv", FormatId::Csv, Confidence::Likely),
    ("toml.toml", "toml.toml", FormatId::Toml, Confidence::Likely),
    ("step.stp", "step.stp", FormatId::Step, Confidence::Certain),
    (
        "geotiff.tif",
        "geotiff.tif",
        FormatId::Geotiff,
        Confidence::Certain,
    ),
    (
        "geopackage.gpkg",
        "geopackage.gpkg",
        FormatId::Geopackage,
        Confidence::Certain,
    ),
    (
        "geojson.geojson",
        "geojson.geojson",
        FormatId::Geojson,
        Confidence::Likely,
    ),
    ("kml.kml", "kml.kml", FormatId::Kml, Confidence::Certain),
    (
        "sqlite.sqlite",
        "sqlite.sqlite",
        FormatId::Sqlite,
        Confidence::Certain,
    ),
    ("hdf5.h5", "hdf5.h5", FormatId::Hdf5, Confidence::Certain),
    (
        "parquet.parquet",
        "parquet.parquet",
        FormatId::Parquet,
        Confidence::Certain,
    ),
    (
        "arrow.arrow",
        "arrow.arrow",
        FormatId::Arrow,
        Confidence::Certain,
    ),
];

#[test]
fn fixture_corpus_probes_expected_format_and_confidence() {
    // Given: the corpus covers every registered format exactly (35 files:
    // gltf appears twice - the .gltf JSON and the GLB container).
    let covered: std::collections::BTreeSet<_> = CORPUS.iter().map(|(_, _, f, _)| *f).collect();
    let slate: std::collections::BTreeSet<_> = FormatId::ALL.iter().copied().collect();
    assert_eq!(covered, slate, "corpus must cover every registered format");
    for (file, hint, format, confidence) in CORPUS {
        // Given: the committed minimal fixture.
        let bytes = fixture(file);
        // When: probed with its own file name as the hint.
        let result = probe(&bytes, Some(hint));
        // Then: identified as exactly the expected format at the expected
        // confidence, with a non-empty note.
        match result {
            ProbeResult::Identified {
                format: found,
                confidence: level,
                ref note,
            } => {
                assert_eq!(found, *format, "{file}: wrong format");
                assert_eq!(level, *confidence, "{file}: wrong confidence");
                assert!(!note.is_empty(), "{file}: empty note");
            }
            other => panic!("{file}: expected Identified, got {other:?}"),
        }
    }
}

#[test]
fn fixtures_probe_identically_without_any_name_hint() {
    // Bytes win over names: dropping the hint must not change any result
    // (hints only break ties; the corpus contains no ties).
    for (file, _hint, format, confidence) in CORPUS {
        let result = probe(&fixture(file), None);
        match result {
            ProbeResult::Identified {
                format: found,
                confidence: level,
                ..
            } => {
                assert_eq!(found, *format, "{file}: hint changed the format");
                assert_eq!(level, *confidence, "{file}: hint changed confidence");
            }
            other => panic!("{file}: expected Identified, got {other:?}"),
        }
    }
}

#[test]
fn bigtiff_magic_probes_geotiff_certain_with_bigtiff_note() {
    // Given: a BigTIFF header (II + version 0x2B).
    let bytes = [0x49, 0x49, 0x2B, 0x00, 0x08, 0x00, 0x00, 0x00];
    // When: probed.
    let result = probe(&bytes, Some("raster.tif"));
    // Then: the geotiff lane claims it and the note names BigTIFF.
    match result {
        ProbeResult::Identified {
            format: FormatId::Geotiff,
            confidence: Confidence::Certain,
            note,
        } => assert!(note.to_lowercase().contains("bigtiff"), "note: {note}"),
        other => panic!("expected geotiff, got {other:?}"),
    }
}

#[test]
fn jxl_container_magic_probes_jxl_certain() {
    // Given: the JXL container signature (not the bare codestream).
    let bytes = [
        0x00, 0x00, 0x00, 0x0C, 0x4A, 0x58, 0x4C, 0x20, 0x0D, 0x0A, 0x87, 0x0A,
    ];
    // When: probed.
    let result = probe(&bytes, Some("photo.jxl"));
    // Then: jxl, Certain.
    match result {
        ProbeResult::Identified {
            format: FormatId::Jxl,
            confidence: Confidence::Certain,
            ..
        } => {}
        other => panic!("expected jxl, got {other:?}"),
    }
}

#[test]
fn legacy_ole2_signature_probes_typed_red_never_parsed() {
    // Given: the 8-byte OLE2 compound-document signature.
    let mut bytes = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    bytes.extend_from_slice(&[0u8; 8]);
    // When: probed.
    let result = probe(&bytes, Some("legacy.doc"));
    // Then: the typed RED identification, never a parsed format.
    match result {
        ProbeResult::LegacyOle2 { note } => assert!(!note.is_empty()),
        other => panic!("expected LegacyOle2, got {other:?}"),
    }
}

#[test]
fn jsonl_lines_probe_json_likely_with_jsonl_note() {
    // Given: two JSON lines (a JSONL stream - not a single document).
    let bytes = b"{\"a\": 1}\n{\"a\": 2}\n";
    // When: probed.
    let result = probe(bytes, Some("events.jsonl"));
    // Then: json at Likely with a note naming the JSONL lane.
    match result {
        ProbeResult::Identified {
            format: FormatId::Json,
            confidence: Confidence::Likely,
            note,
        } => assert!(note.to_lowercase().contains("jsonl"), "note: {note}"),
        other => panic!("expected json, got {other:?}"),
    }
}

#[test]
fn id3_tag_probes_mp3_certain() {
    // Given: a bare ID3v2 tag header.
    let bytes = b"ID3\x04\x00\x00\x00\x00\x00\x00";
    // When: probed.
    let result = probe(bytes, Some("song.mp3"));
    // Then: mp3, Certain.
    match result {
        ProbeResult::Identified {
            format: FormatId::Mp3,
            confidence: Confidence::Certain,
            ..
        } => {}
        other => panic!("expected mp3, got {other:?}"),
    }
}
