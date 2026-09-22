//! M1 contract tests: serde/display round-trips, typed-error discipline,
//! empty-registry lookup, determinism constants, and malformed-input
//! behavior for the adapter contract types. Hermetic: no network, no
//! filesystem writes, no external tools.

use std::collections::BTreeMap;
use std::time::Duration;

use harness_core::event::{ContentHash, MediaRef};
use harness_media::descriptor::{FormatDescriptor, FormatRegistry, Operation, OpSupport};
use harness_media::error::MediaError;
use harness_media::extract::{ExtractBudgets, Extracted, TEXT_PROJECTION_MAX_CHARS};
use harness_media::format::FormatId;
use harness_media::generate::{GenerationSpec, DETERMINISM_EPOCH, DETERMINISM_EPOCH_UNIX};
use serde_json::json;

/// The 34-format slate in plan declaration order.
const SLATE: &[&str] = &[
    "png", "jpeg", "gif", "webp", "avif", "jxl", "svg", "gltf", "blend", "pdf", "wav", "flac",
    "opus", "mp3", "ogg", "webm", "mkv", "docx", "xlsx", "pptx", "markdown", "html", "json",
    "csv", "toml", "step", "geotiff", "geopackage", "geojson", "kml", "sqlite", "hdf5",
    "parquet", "arrow",
];

/// Every MediaError variant, at least one instance each.
fn every_error_variant() -> Vec<MediaError> {
    vec![
        MediaError::UnsupportedFormat,
        MediaError::UnsupportedOperation {
            format: FormatId::Pptx,
            op: Operation::Generate,
            reason: "no maintained pure-Rust pptx writer; sandboxed pandoc tier required".into(),
        },
        MediaError::ToolUnavailable {
            tool: "pandoc".into(),
            install_hint: "enable the office format pack and run the pinned installer".into(),
        },
        MediaError::InstallFailed,
        MediaError::SandboxDenied,
        MediaError::Timeout,
        MediaError::InvalidSpec,
        MediaError::ParseFailure,
        MediaError::EncryptedUnsupported,
        MediaError::NoProjector,
        MediaError::ExtractTooLarge,
    ]
}

/// A minimal well-formed descriptor for the given format.
fn descriptor_for(id: FormatId) -> FormatDescriptor {
    let mut ops = BTreeMap::new();
    ops.insert(Operation::Probe, OpSupport::Supported);
    ops.insert(
        Operation::Extract,
        OpSupport::Unsupported {
            reason: "extract arrives with the adapter todos".into(),
        },
    );
    ops.insert(
        Operation::Generate,
        OpSupport::Unsupported {
            reason: "decode-only lane".into(),
        },
    );
    ops.insert(
        Operation::Verify,
        OpSupport::Unsupported {
            reason: "verify arrives with the M5 diff engine".into(),
        },
    );
    FormatDescriptor {
        id,
        mime: format!("application/x-{id}"),
        extensions: vec![id.to_string()],
        ops,
    }
}

#[test]
fn format_id_serde_roundtrips_when_canonical_snake_case_name_used() {
    // Given: the declared 34-format slate.
    assert_eq!(FormatId::ALL.len(), SLATE.len());
    let mut seen: Vec<&str> = Vec::new();
    for (id, name) in FormatId::ALL.iter().zip(SLATE) {
        // When: serialized, rendered, and parsed back.
        let wire = serde_json::to_value(id).unwrap();
        // Then: the wire form is the exact snake_case name, Display agrees
        // with serde, and deserialize is the identity.
        assert_eq!(wire, json!(name), "wire form mismatch for {name}");
        assert_eq!(id.to_string(), *name, "Display disagrees with wire form");
        let back: FormatId = serde_json::from_value(json!(name)).unwrap();
        assert_eq!(back, *id);
        seen.push(name);
    }
    seen.sort_unstable();
    let mut deduped = seen.clone();
    deduped.dedup();
    assert_eq!(seen, deduped, "FormatId::ALL contains duplicates");
}

#[test]
fn format_id_rejects_malformed_strings_when_deserialized() {
    // Given: strings that are not canonical variant names.
    let malformed = [
        "", "PNG", "png ", " png", "png\n", "not-a-format", "geotiff2", "hdf-5", "parquet/",
        "\u{1f980}", "0", "p n g",
    ];
    for name in malformed {
        // When/Then: deserialization is a typed error, never a panic.
        let parsed: Result<FormatId, _> = serde_json::from_value(json!(name));
        assert!(parsed.is_err(), "{name:?} must not deserialize as a FormatId");
    }
}

#[test]
fn format_id_never_panics_when_fed_garbage_bytes() {
    // Given: 256 deterministic pseudo-random byte strings (fuzz-ish loop).
    let mut state = 0x5eed_1234_u64;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        state
    };
    for _ in 0..256 {
        let length = (next() >> 33) as usize % 16;
        let bytes: Vec<u8> = (0..length)
            .map(|i| ((next() >> ((i % 8) * 8)) & 0xff) as u8)
            .collect();
        // When/Then: from_slice is Err (or an accidental valid name); the
        // loop completing is the zero-panics proof.
        let _ = serde_json::from_slice::<FormatId>(&bytes);
    }
}

#[test]
fn media_error_serde_roundtrips_when_any_variant_serialized() {
    for error in every_error_variant() {
        let wire = serde_json::to_string(&error).unwrap();
        let back: MediaError = serde_json::from_str(&wire).unwrap();
        assert_eq!(back, error, "round-trip broke for {wire}");
    }
}

#[test]
fn media_error_display_carries_actionable_hint_when_rendered() {
    const ACTION_MARKERS: &[&str] = &[
        "probe", "install", "re-run", "check", "fix", "decrypt", "register", "raise", "split",
        "request", "use", "resubmit", "narrow", "re-probe",
    ];
    for error in &every_error_variant() {
        let text = error.to_string();
        assert!(!text.is_empty(), "empty Display for {error:?}");
        assert!(
            ACTION_MARKERS.iter().any(|marker| text.contains(marker)),
            "Display names no next action: {text:?}"
        );
    }
}

#[test]
fn media_error_display_surfaces_its_fields_when_variant_carries_them() {
    // Given: field-bearing variants.
    let unsupported = MediaError::UnsupportedOperation {
        format: FormatId::Hdf5,
        op: Operation::Extract,
        reason: "deferred: single-vendor stewardship".into(),
    }
    .to_string();
    // Then: format, op, and reason are all named.
    assert!(unsupported.contains("hdf5"));
    assert!(unsupported.contains("extract"));
    assert!(unsupported.contains("deferred"));

    let tool = MediaError::ToolUnavailable {
        tool: "gdal".into(),
        install_hint: "enable the georaster pack".into(),
    }
    .to_string();
    assert!(tool.contains("gdal"));
    assert!(tool.contains("georaster pack"));
}

#[test]
fn empty_registry_lookup_fails_typed_when_nothing_registered() {
    // Given: a registry with zero descriptors.
    let registry = FormatRegistry::new();
    // When: any format is looked up.
    // Then: the result is exactly UnsupportedFormat - the RIGHT variant,
    // not a generic error or a panic.
    assert!(matches!(
        registry.lookup(&FormatId::Png),
        Err(MediaError::UnsupportedFormat)
    ));
}

#[test]
fn registry_lookup_succeeds_when_descriptor_registered() {
    // Given: one registered descriptor.
    let mut registry = FormatRegistry::new();
    assert!(registry.register(descriptor_for(FormatId::Png)));
    // When: it is looked up again.
    let found = registry.lookup(&FormatId::Png);
    // Then: the same descriptor comes back.
    assert_eq!(found, Ok(&descriptor_for(FormatId::Png)));
    // And: any other format is still the typed UnsupportedFormat.
    assert!(matches!(
        registry.lookup(&FormatId::Jpeg),
        Err(MediaError::UnsupportedFormat)
    ));
    // And: a duplicate registration is refused without overwriting.
    assert!(!registry.register(descriptor_for(FormatId::Png)));
}

#[test]
fn descriptor_serializes_ops_with_sorted_keys_when_roundtripped() {
    // Given: a descriptor whose ops are inserted in non-sorted order.
    let descriptor = descriptor_for(FormatId::Avif);
    // When: serialized and re-parsed.
    let wire = serde_json::to_string(&descriptor).unwrap();
    let value: serde_json::Value = serde_json::from_str(&wire).unwrap();
    let keys: Vec<&str> = value["ops"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    // Then: op keys are byte-sorted (serde_json BTreeMap ordering).
    assert_eq!(keys, ["extract", "generate", "probe", "verify"]);
    let back: FormatDescriptor = serde_json::from_str(&wire).unwrap();
    assert_eq!(back, descriptor);
}

#[test]
fn descriptor_rejects_unknown_operation_names_when_deserialized() {
    // Given: a structurally valid descriptor JSON naming an op outside the
    // four-operation contract.
    let wire = r#"{"id":"png","mime":"image/png","extensions":["png"],"ops":{"transcode":"Supported"}}"#;
    // When/Then: deserialization is a typed serde error.
    assert!(serde_json::from_str::<FormatDescriptor>(wire).is_err());
}

#[test]
fn generation_spec_roundtrips_and_epoch_constants_are_pinned() {
    // Given: the determinism contract constants.
    assert_eq!(DETERMINISM_EPOCH, "2026-01-01T00:00:00Z");
    assert_eq!(DETERMINISM_EPOCH_UNIX, 1_767_225_600);
    // When: a spec round-trips.
    let spec = GenerationSpec {
        format: FormatId::Pdf,
        params: json!({"pages": 1, "title": "deterministic"}),
    };
    let wire = serde_json::to_string(&spec).unwrap();
    let back: GenerationSpec = serde_json::from_str(&wire).unwrap();
    // Then: identity.
    assert_eq!(back, spec);
}

#[test]
fn generation_spec_rejects_malformed_json_when_deserialized() {
    // Given: broken JSON and an unknown format name.
    for wire in ["{ nope", r#"{"format":"definitely-not","params":{}}"#, "null_x"] {
        // When/Then: typed serde errors, never panics.
        assert!(serde_json::from_str::<GenerationSpec>(wire).is_err());
    }
}

#[test]
fn extracted_roundtrips_and_projection_bound_is_chars() {
    // Given: the bound is expressed in CHARS (never tokens).
    assert_eq!(TEXT_PROJECTION_MAX_CHARS, 2000);
    let thumbnail = MediaRef {
        hash: ContentHash::from_bytes(b"thumbnail"),
        bytes: 9,
    };
    let extracted = Extracted {
        tree: json!({"root": {"kind": "svg", "children": []}}),
        text_projection: "[svg] 1 root element".into(),
        thumbnails: vec![thumbnail],
    };
    // When: round-tripped.
    let wire = serde_json::to_string(&extracted).unwrap();
    let back: Extracted = serde_json::from_str(&wire).unwrap();
    // Then: identity.
    assert_eq!(back, extracted);
}

#[test]
fn extract_budgets_default_is_fixture_scoped_and_overridable() {
    // Given: the documented default budgets.
    let default = ExtractBudgets::default();
    assert_eq!(default.deadline, Duration::from_secs(10));
    assert_eq!(default.max_total_bytes, 64 * 1024 * 1024);
    // When: a caller passes narrower fixture-scoped budgets.
    let custom = ExtractBudgets {
        deadline: Duration::from_millis(250),
        max_total_bytes: 4096,
    };
    // Then: the override is distinct and honored by value.
    assert_ne!(custom, default);
}
