//! M12 round-trip gate: both writers generate -> extract -> M5 diff
//! (empty = PASS) over a spec corpus, with the independent re-parse
//! (khronos-ecosystem `gltf` reader) breaking writer/reader circularity,
//! byte-determinism pinned, and the Khronos glTF-Validator tier.
//!
//! ## Validator tiers (recorded decision)
//!
//! The plan's "pure Rust glTF-Validator in CI" premise is factually wrong:
//! the Khronos validator is DART, and the only crates.io `gltf-validator`
//! crate downloads a prebuilt foreign binary at BUILD time and does not
//! support Windows - neither is hermetic-CI-compatible (standing
//! constraint 5). The in-CI gate is therefore the trio below; the real
//! Khronos validator runs as the `#[ignore]` real-tool tier
//! (constraint-5 discipline), pinned errors=0.

#![cfg(feature = "gltf")]

use harness_media::adapter::gltf;
use harness_media::diff::{RoundTripVerdict, Tolerances, structural_diff};
use harness_media::error::MediaError;
use harness_media::executor::OpRequest;
use harness_media::extract::ExtractBudgets;
use harness_media::format::FormatId;
use harness_media::generate::GenerationSpec;
use harness_media::probe::{Confidence, ProbeResult, probe};

/// The M12 tolerance profile: eps 1e-6; materials + name-keyed nodes
/// unordered (the plan's only sanctioned reordering escapes).
fn profile() -> Tolerances {
    gltf::tolerances()
}

/// Spec corpus entry: (name, params). Built independently of the writer;
/// floats are dyadic so f32 round-trips stay exact.
fn corpus() -> Vec<(&'static str, serde_json::Value)> {
    vec![
        (
            "hierarchy-with-materials",
            serde_json::json!({
                "container": "glb",
                "nodes": [
                    {"name": "root", "translation": [1.0, 2.0, 3.0],
                     "children": ["spin", "plain"], "mesh": "tris"},
                    {"name": "spin", "rotation": [0.0, 0.0, 0.5, 0.5], "scale": [2.0, 2.0, 2.0],
                     "mesh": "tris"},
                    {"name": "plain"}
                ],
                "meshes": [
                    {"name": "tris", "primitives": [
                        {"positions": [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                         "normals": [[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]],
                         "indices": [0, 1, 2], "material": "red"},
                        {"positions": [[2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 2.0]],
                         "material": "blue"}
                    ]}
                ],
                "materials": [
                    {"name": "red", "base_color": [1.0, 0.0, 0.0, 0.5], "alpha_mode": "BLEND"},
                    {"name": "blue", "base_color": [0.0, 0.0, 1.0, 1.0]}
                ]
            }),
        ),
        (
            "nodes-only",
            serde_json::json!({
                "container": "gltf",
                "nodes": [{"name": "only", "translation": [0.5, 0.25, 0.125]}]
            }),
        ),
        (
            "mesh-without-indices",
            serde_json::json!({
                "container": "glb",
                "nodes": [{"name": "n", "mesh": "m"}],
                "meshes": [{"name": "m", "primitives": [
                    {"positions": [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0]],
                     "normals": [[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]]}
                ]}]
            }),
        ),
    ]
}

fn spec(params: serde_json::Value) -> GenerationSpec {
    GenerationSpec {
        format: FormatId::Gltf,
        params,
    }
}

fn extract(bytes: &[u8]) -> Result<harness_media::extract::Extracted, MediaError> {
    gltf::extract(bytes, ExtractBudgets::default())
}

#[test]
fn both_writers_round_trip_empty_diff_on_corpus() {
    for (name, params) in corpus() {
        for container in ["glb", "gltf"] {
            let mut params = params.clone();
            params["container"] = serde_json::json!(container);
            let spec = spec(params);
            // Generate via the EXECUTOR path (descriptor sync + disclosure).
            let registry = harness_media::executor::registry();
            let produced = registry
                .dispatch(FormatId::Gltf, OpRequest::Generate { spec: &spec })
                .unwrap_or_else(|error| panic!("{name}/{container}: generate failed: {error}"));
            let bytes = match produced.inner() {
                harness_media::executor::OpOutput::Generate(bytes) => bytes.clone(),
                other => panic!("{name}/{container}: expected generate output, got {other:?}"),
            };
            // Expected tree: the INDEPENDENT spec->tree projection.
            let expected = gltf::spec_tree(&spec.params).expect("spec projects");
            // Verify via the adapter's profile: extract + diff = empty.
            let verdict = gltf::verify(&expected, &bytes)
                .unwrap_or_else(|error| panic!("{name}/{container}: verify failed: {error}"));
            assert!(
                matches!(verdict, RoundTripVerdict::Pass),
                "{name}/{container}: round trip must pass, got:\n{verdict:?}"
            );
        }
    }
}

#[test]
fn generated_outputs_probe_certain_as_gltf() {
    // Every generated artifact must sniff back as its container family.
    for (_, params) in corpus() {
        let spec = spec(params);
        let bytes = gltf::generate(&spec).expect("generate");
        match probe(&bytes, None) {
            ProbeResult::Identified {
                format, confidence, ..
            } => {
                assert_eq!(format, FormatId::Gltf);
                assert_eq!(confidence, Confidence::Certain);
            }
            other => panic!("generated bytes must probe gltf, got {other:?}"),
        }
    }
}

#[test]
fn writer_containers_agree_everywhere_but_the_container_node() {
    // Given: one spec generated through BOTH writers.
    let (name, params) = corpus().remove(0);
    let mut glb_params = params.clone();
    glb_params["container"] = serde_json::json!("glb");
    let glb = gltf::generate(&spec(glb_params)).expect("glb generate");
    let mut gltf_params = params;
    gltf_params["container"] = serde_json::json!("gltf");
    let gltf_doc = gltf::generate(&spec(gltf_params)).expect("gltf generate");
    // When: the two extracted trees are diffed ignoring ONLY /container.
    let glb_tree = extract(&glb).expect("glb extract").tree;
    let gltf_tree = extract(&gltf_doc).expect("gltf extract").tree;
    let mut tolerances = profile();
    tolerances.ignored_at.insert("/container".to_string());
    let diff = structural_diff(&gltf_tree, &glb_tree, &tolerances)
        .unwrap_or_else(|error| panic!("{name}: diff failed: {error}"));
    // Then: the trees agree everywhere but the container node.
    assert!(
        diff.is_empty(),
        "{name}: container-agnostic diff not empty:\n{diff}"
    );
}

#[test]
fn generation_is_byte_deterministic_across_runs() {
    // DETERMINISM CONTRACT: same spec in, byte-identical bytes out (no
    // ambient clocks anywhere in the glTF subset).
    for (name, params) in corpus() {
        let spec = spec(params.clone());
        let first = gltf::generate(&spec).expect("first generate");
        let second = gltf::generate(&spec).expect("second generate");
        assert_eq!(
            first, second,
            "{name}: generation must be byte-deterministic"
        );
    }
}

#[test]
fn generated_documents_reparse_through_the_khronos_reader() {
    // Independent third-party parse (the gltf crate's validating reader -
    // a different implementation path than the serde-subset writer) over
    // every generated artifact: breaks writer/reader circularity.
    for (name, params) in corpus() {
        for container in ["glb", "gltf"] {
            let mut params = params.clone();
            params["container"] = serde_json::json!(container);
            let bytes = gltf::generate(&spec(params))
                .unwrap_or_else(|error| panic!("{name}/{container}: generate: {error}"));
            let reparsed = gltf::reparse(&bytes)
                .unwrap_or_else(|error| panic!("{name}/{container}: re-parse: {error}"));
            // The asset version survives the full third-party round trip.
            assert_eq!(reparsed, "2.0", "{name}/{container}: asset version");
        }
    }
}

#[test]
fn u32_indices_fall_back_when_vertices_exceed_u16() {
    // Given: 70_000 vertices referenced by index (max index >= 65536).
    let positions: Vec<[f32; 3]> = (0..70_000).map(|i| [i as f32, 0.0, 0.0]).collect();
    let indices: Vec<u32> = (0..70_000u32).rev().collect();
    let params = serde_json::json!({
        "container": "glb",
        "nodes": [{"name": "big", "mesh": "m"}],
        "meshes": [{"name": "m", "primitives": [{"positions": positions, "indices": indices}]}]
    });
    let spec = spec(params);
    // When: generated and verified against the independent projection
    // (which applies the same u16/u32 width rule from the spec side).
    let bytes = gltf::generate(&spec).expect("u32-index generate");
    let expected = gltf::spec_tree(&spec.params).expect("spec projects");
    let verdict = gltf::verify(&expected, &bytes).expect("u32-index verify");
    assert!(
        matches!(verdict, RoundTripVerdict::Pass),
        "u32 indices: {verdict:?}"
    );
    // And: the independent khronos reader still accepts it.
    assert!(gltf::reparse(&bytes).is_ok(), "u32-index re-parse");
}

#[test]
fn invalid_specs_fail_typed_before_any_bytes_are_emitted() {
    let cases: Vec<(&str, serde_json::Value)> = vec![
        (
            "duplicate node names",
            serde_json::json!({
                "nodes": [{"name": "a"}, {"name": "a"}]
            }),
        ),
        (
            "dangling child ref",
            serde_json::json!({
                "nodes": [{"name": "a", "children": ["ghost"]}]
            }),
        ),
        (
            "unknown mesh ref",
            serde_json::json!({
                "nodes": [{"name": "a", "mesh": "ghost"}]
            }),
        ),
        (
            "unknown material ref",
            serde_json::json!({
                "nodes": [{"name": "a"}],
                "meshes": [{"name": "m", "primitives": [
                    {"positions": [[0.0, 0.0, 0.0]], "material": "ghost"}]}]
            }),
        ),
        (
            "normal count mismatch",
            serde_json::json!({
                "nodes": [{"name": "a"}],
                "meshes": [{"name": "m", "primitives": [
                    {"positions": [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
                     "normals": [[0.0, 1.0, 0.0]]}]}]
            }),
        ),
        (
            "empty positions",
            serde_json::json!({
                "nodes": [{"name": "a"}],
                "meshes": [{"name": "m", "primitives": [{"positions": []}]}]
            }),
        ),
        (
            "bad container kind",
            serde_json::json!({"container": "obj"}),
        ),
        ("empty nodes", serde_json::json!({"nodes": []})),
        (
            "index out of vertex range",
            serde_json::json!({
                "nodes": [{"name": "a"}],
                "meshes": [{"name": "m", "primitives": [
                    {"positions": [[0.0, 0.0, 0.0]], "indices": [7]}]}]
            }),
        ),
    ];
    for (name, params) in cases {
        let error = gltf::generate(&spec(params)).expect_err(&format!("{name}: must be refused"));
        assert!(
            matches!(error, MediaError::InvalidSpec),
            "{name}: expected InvalidSpec, got {error:?}"
        );
    }
}

#[test]
fn verify_reports_exact_pointers_on_a_broken_round_trip() {
    // Given: an expected tree that disagrees with the produced bytes.
    let params = corpus().remove(1).1; // nodes-only, container gltf
    let spec = spec(params);
    let bytes = gltf::generate(&spec).expect("generate");
    let mut expected = gltf::spec_tree(&spec.params).expect("spec projects");
    expected["nodes"][0]["translation"] = serde_json::json!([9.0, 9.0, 9.0]);
    // When: verified.
    let verdict = gltf::verify(&expected, &bytes).expect("verify runs");
    // Then: the failure names the exact changed pointer (within eps).
    match verdict {
        RoundTripVerdict::Fail(diff) => {
            assert!(
                diff.changed
                    .iter()
                    .any(|change| change.path.contains("/translation")),
                "diff must name /translation, got:\n{diff}"
            );
        }
        RoundTripVerdict::Pass => panic!("mutated tree must not pass"),
    }
}

/// The real-tool tier: the Khronos glTF-Validator (Dart CLI binary) over
/// every generated GLB, errors=0 pinned. Never runs in CI (hermetic
/// constraint); run at-todo-time / on the M25 cadence with:
/// `GLTF_VALIDATOR_BIN=<path to gltf_validator.exe> cargo nextest run
///   -p harness-media --run-ignored only --test gltf_roundtrip`
#[test]
#[ignore = "real-tool tier: needs the Khronos glTF-Validator CLI via GLTF_VALIDATOR_BIN"]
fn khronos_validator_reports_zero_errors_on_generated_glbs() {
    let bin = std::env::var_os("GLTF_VALIDATOR_BIN").unwrap_or_else(|| {
        panic!(
            "set GLTF_VALIDATOR_BIN to the Khronos glTF-Validator CLI binary \
             (github.com/KhronosGroup/glTF-Validator releases; not vendored - \
             hermetic CI never downloads tools)"
        )
    });
    for (name, params) in corpus() {
        let mut params = params.clone();
        params["container"] = serde_json::json!("glb");
        let bytes = gltf::generate(&spec(params)).expect("generate");
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("artifact.glb");
        std::fs::write(&file, &bytes).expect("write glb");
        let output = std::process::Command::new(&bin)
            .arg("-o")
            .arg(&file)
            .output()
            .unwrap_or_else(|error| panic!("{name}: validator spawn: {error}"));
        let report: serde_json::Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| panic!("{name}: report parse: {error}"));
        let errors = report["issues"]["numErrors"].as_u64().unwrap_or(u64::MAX);
        assert_eq!(
            errors, 0,
            "{name}: Khronos validator errors=0 is pinned; report: {report}"
        );
    }
}
