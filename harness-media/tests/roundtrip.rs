//! M5 round-trip verifier + expected-tree provenance tests:
//! `verify_roundtrip` composition (pass, exact-pointer fail, typed error
//! propagation) and the spec->tree projection contract (independent of any
//! extractor, deterministic, canonical). Hermetic throughout.

use harness_media::diff::{
    RoundTripVerdict, Tolerances, spec_tree, structural_diff, verify_roundtrip,
};
use harness_media::error::MediaError;
use harness_media::extract::Extracted;
use harness_media::format::FormatId;
use harness_media::generate::GenerationSpec;
use serde_json::{Value, json};

/// Honest generic extractor: parses produced bytes as canonical JSON, the
/// shape a real adapter's extract has (bytes -> canonical tree).
fn json_extract(bytes: &[u8]) -> Result<Extracted, MediaError> {
    let tree: Value = serde_json::from_slice(bytes).map_err(|_| MediaError::ParseFailure)?;
    Ok(Extracted {
        tree,
        text_projection: "[json]".into(),
        thumbnails: vec![],
    })
}

#[test]
fn verify_roundtrip_passes_when_extracted_tree_matches_expected() {
    // Given: an expected tree and bytes that serialize it.
    let expected = json!({"asset": {"version": "2.0"}, "meshes": 2});
    let produced = serde_json::to_vec(&expected).unwrap();
    // When: verified through the composition helper.
    let verdict =
        verify_roundtrip(&expected, &produced, json_extract, &Tolerances::default()).unwrap();
    // Then: PASS - empty diff is the round-trip gate (semantic equivalence).
    assert_eq!(verdict, RoundTripVerdict::Pass);
    assert!(verdict.is_pass());
}

#[test]
fn verify_roundtrip_fails_with_exact_pointer_on_one_token_change() {
    // Given: produced bytes whose tree differs by one token.
    let expected = json!({"pages": [{"text": "hello"}], "title": "doc"});
    let mut actual = expected.clone();
    actual["pages"][0]["text"] = json!("hallo");
    let produced = serde_json::to_vec(&actual).unwrap();
    // When: verified.
    let verdict =
        verify_roundtrip(&expected, &produced, json_extract, &Tolerances::default()).unwrap();
    // Then: Fail carrying the exact pointer (never a bare boolean).
    match verdict {
        RoundTripVerdict::Fail(diff) => {
            assert_eq!(diff.changed.len(), 1);
            assert_eq!(diff.changed[0].path, "/pages/0/text");
            assert_eq!(diff.changed[0].expected, json!("hello"));
            assert_eq!(diff.changed[0].actual, json!("hallo"));
        }
        RoundTripVerdict::Pass => panic!("a changed token must not pass"),
    }
}

#[test]
fn extract_failure_propagates_typed_never_a_fake_pass() {
    // Given: produced bytes that do not parse.
    // When: verified.
    let verdict = verify_roundtrip(
        &json!({}),
        b"not json at all",
        json_extract,
        &Tolerances::default(),
    );
    // Then: the extractor's typed error propagates - no silent Pass.
    assert!(matches!(verdict, Err(MediaError::ParseFailure)));
}

#[test]
fn spec_tree_projection_is_deterministic_canonical_and_spec_faithful() {
    // Given: one generation spec, projected twice.
    let spec = GenerationSpec {
        format: FormatId::Svg,
        params: json!({"viewBox": "0 0 10 10", "shapes": 3}),
    };
    let first = spec_tree(&spec);
    let second = spec_tree(&spec);
    // Then: deterministic (same tree every call).
    assert_eq!(first, second);
    // And canonical: object keys serialize sorted (format < params).
    let wire = serde_json::to_string(&first).unwrap();
    assert!(
        wire.starts_with("{\"format\":\"svg\",\"params\":"),
        "got: {wire}"
    );
    // And spec-faithful: format identity and params carry through.
    assert_eq!(first["format"], json!("svg"));
    assert_eq!(first["params"], spec.params);
    // And distinct formats project to distinct trees (no format-blind fold).
    let other = spec_tree(&GenerationSpec {
        format: FormatId::Gltf,
        params: json!({"viewBox": "0 0 10 10", "shapes": 3}),
    });
    assert_ne!(first, other);
}

#[test]
fn provenance_rule_catches_extractor_watermark_a_circle_would_hide() {
    // PROVENANCE CONTRACT (plan M5): expected trees come from an INDEPENDENT
    // spec->tree projection - never from the adapter's own extractor run on
    // its own output. An extractor that watermarks its output would be
    // invisible to a circular expected tree; the independent projection
    // catches the extra key at its exact pointer.
    let spec = GenerationSpec {
        format: FormatId::Json,
        params: json!({"rows": 2}),
    };
    let expected = spec_tree(&spec);
    let watermarking_extract = |bytes: &[u8]| -> Result<Extracted, MediaError> {
        let mut tree: Value =
            serde_json::from_slice(bytes).map_err(|_| MediaError::ParseFailure)?;
        tree["extractor_watermark"] = json!("acme-v1");
        Ok(Extracted {
            tree,
            text_projection: String::new(),
            thumbnails: vec![],
        })
    };
    let produced = serde_json::to_vec(&json!({"format": "json", "params": {"rows": 2}})).unwrap();
    let verdict = verify_roundtrip(
        &expected,
        &produced,
        watermarking_extract,
        &Tolerances::default(),
    )
    .unwrap();
    match verdict {
        RoundTripVerdict::Fail(diff) => {
            assert_eq!(diff.added.len(), 1);
            assert_eq!(diff.added[0].path, "/extractor_watermark");
        }
        RoundTripVerdict::Pass => {
            panic!("the watermark must be caught by the independent projection")
        }
    }
}

#[test]
fn default_tolerances_are_exact_comparison() {
    // Given: the default profile.
    let t = Tolerances::default();
    // Then: exact floats, no reordering escapes, no ignored subtrees -
    // tolerance is a per-format profile decision (M25 table), never ambient.
    assert_eq!(t.float_epsilon, 0.0);
    assert!(t.unordered_at.is_empty());
    assert!(t.ignored_at.is_empty());
    let out = structural_diff(&json!({"f": 0.3}), &json!({"f": 0.1 + 0.2}), &t).unwrap();
    assert_eq!(out.changed.len(), 1);
}
