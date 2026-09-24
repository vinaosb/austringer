//! M12 glTF/GLB adapter tests: extraction to pinned trees, the typed
//! compressed-variant refusal, the offset-precise GLB container walk, the
//! deterministic scene-outline projector, and executor wiring. Hermetic:
//! committed fixtures only, no network, no external tools.

#![cfg(feature = "gltf")]

use std::sync::Arc;

use harness_media::adapter::gltf;
use harness_media::error::MediaError;
use harness_media::executor::OpRequest;
use harness_media::extract::ExtractBudgets;
use harness_media::format::FormatId;
use harness_media::projection::{ProjectionContext, ProjectionProvider, ProjectionRegistry};

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("gltf")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("fixture {name}: {error}"))
}

/// The pinned tree shared by both containers; only `/container` differs.
fn pinned_scene_tree() -> serde_json::Value {
    serde_json::json!({
        "accessor_sizes": [36, 36, 6, 36, 8, 24],
        "animations": 1,
        "asset": {"generator": "harness-fixture", "version": "2.0"},
        "buffer_view_sizes": [36, 36, 6, 36, 8, 24],
        "materials": [
            {"alpha_mode": "BLEND", "base_color_factor": [1.0, 0.0, 0.0, 0.5], "name": "red"},
            {"alpha_mode": "OPAQUE", "base_color_factor": [0.0, 0.0, 1.0, 1.0], "name": "blue"}
        ],
        "nodes": [
            {
                "children": ["child-a", "child-b"],
                "mesh": "tris",
                "name": "root",
                "primitives": [
                    {"attributes": ["NORMAL", "POSITION"], "has_indices": true, "material": "red"},
                    {"attributes": ["POSITION"], "has_indices": false, "material": "blue"}
                ],
                "rotation": null,
                "scale": null,
                "translation": [1.0, 2.0, 3.0]
            },
            {
                "children": [],
                "mesh": null,
                "name": "child-a",
                "primitives": [],
                "rotation": [0.0, 0.0, 0.5, 0.5],
                "scale": [2.0, 2.0, 2.0],
                "translation": null
            },
            {
                "children": [],
                "mesh": null,
                "name": "child-b",
                "primitives": [],
                "rotation": null,
                "scale": null,
                "translation": null
            }
        ],
        "scene": {"name": "Scene", "roots": ["root"]}
    })
}

#[test]
fn gltf_fixture_extracts_to_pinned_tree() {
    // Given: the committed .gltf fixture (data-URI buffer).
    // When: extracted under default budgets.
    let extracted = gltf::extract(&fixture("scene.gltf"), ExtractBudgets::default())
        .expect("scene.gltf must extract");
    // Then: the canonical tree is exactly the pinned tree (dyadic floats:
    // f32 round-trips are exact, no tolerance needed for the pin itself).
    let mut expected = pinned_scene_tree();
    expected["container"] = serde_json::json!({"kind": "gltf"});
    assert_eq!(extracted.tree, expected);
    assert!(extracted.thumbnails.is_empty());
}

#[test]
fn glb_fixture_extracts_to_pinned_tree_with_bin_chunk_size() {
    // Given: the committed .glb fixture (JSON + BIN chunks).
    let extracted = gltf::extract(&fixture("scene.glb"), ExtractBudgets::default())
        .expect("scene.glb must extract");
    // Then: same pinned tree; the container records the GLB BIN chunk size
    // (148 B, the fixture's declared chunk length).
    let mut expected = pinned_scene_tree();
    expected["container"] = serde_json::json!({"bin_bytes": 148, "kind": "glb"});
    assert_eq!(extracted.tree, expected);
}

#[test]
fn text_projection_is_the_bounded_scene_outline() {
    // Given: either fixture.
    let extracted = gltf::extract(&fixture("scene.glb"), ExtractBudgets::default())
        .expect("glb must extract");
    // Then: the projection is a deterministic outline naming the asset,
    // container, hierarchy, and material census - and is bounded.
    assert!(extracted.text_projection.contains("gltf 2.0"), "projection: {}",
        extracted.text_projection);
    assert!(extracted.text_projection.contains("glb"), "projection: {}",
        extracted.text_projection);
    assert!(extracted.text_projection.contains("root"), "projection: {}",
        extracted.text_projection);
    assert!(extracted.text_projection.contains("child-a"), "projection: {}",
        extracted.text_projection);
    assert!(extracted.text_projection.contains("red"), "projection: {}",
        extracted.text_projection);
    assert!(
        extracted.text_projection.chars().count() <= 1200,
        "projection bound: {}",
        extracted.text_projection.chars().count()
    );
    // Deterministic: the same bytes project to the same text.
    let again = gltf::extract(&fixture("scene.glb"), ExtractBudgets::default())
        .expect("glb must extract again");
    assert_eq!(extracted.text_projection, again.text_projection);
}

#[test]
fn draco_required_fixture_is_typed_unsupported_feature() {
    // Given: a GLB whose JSON declares KHR_draco_mesh_compression required.
    // When: extracted.
    let error = gltf::extract(&fixture("draco.glb"), ExtractBudgets::default())
        .expect_err("compressed primitives must be refused typed");
    // Then: UnsupportedFeature naming the format and the extension - never
    // a mangled extraction, never a panic.
    match error {
        MediaError::UnsupportedFeature { format, feature, .. } => {
            assert_eq!(format, FormatId::Gltf);
            assert_eq!(feature, "KHR_draco_mesh_compression");
        }
        other => panic!("expected UnsupportedFeature, got {other:?}"),
    }
}

#[test]
fn meshopt_required_is_typed_unsupported_feature() {
    // Given: a .gltf JSON declaring EXT_meshopt_compression required.
    let bytes = br#"{"asset":{"version":"2.0"},"extensionsRequired":["EXT_meshopt_compression"],
        "scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"name":"n"}]}"#;
    let error = gltf::extract(bytes, ExtractBudgets::default())
        .expect_err("meshopt must be refused typed");
    match error {
        MediaError::UnsupportedFeature { feature, .. } => {
            assert_eq!(feature, "EXT_meshopt_compression");
        }
        other => panic!("expected UnsupportedFeature, got {other:?}"),
    }
}

#[test]
fn truncated_glb_chunk_reports_parse_failure_with_byte_offset() {
    // Given: a GLB whose total length is consistent but whose JSON chunk
    // header declares 100 bytes of payload while the file ends after 2.
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(b"glTF");
    bytes.extend_from_slice(&2u32.to_le_bytes());
    bytes.extend_from_slice(&22u32.to_le_bytes());
    bytes.extend_from_slice(&100u32.to_le_bytes());
    bytes.extend_from_slice(b"JSON");
    bytes.extend_from_slice(b"{}");
    // When: the container is walked.
    let error = gltf::glb_chunks(&bytes).expect_err("truncated chunk must fail");
    // Then: the typed error carries the chunk header's byte offset.
    assert_eq!(error.offset(), 12, "offset points at the JSON chunk header");
    // And: extraction surfaces it as the typed ParseFailure.
    let error = gltf::extract(&bytes, ExtractBudgets::default())
        .expect_err("extract must refuse the truncated GLB");
    assert!(matches!(error, MediaError::ParseFailure), "got {error:?}");
}

#[test]
fn wrong_glb_version_and_magic_report_offsets() {
    // Given: a GLB header claiming version 3.
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(b"glTF");
    bytes.extend_from_slice(&3u32.to_le_bytes());
    bytes.extend_from_slice(&12u32.to_le_bytes());
    let error = gltf::glb_chunks(&bytes).expect_err("version 3 must fail");
    assert_eq!(error.offset(), 4, "version field sits at byte 4");
    // Given: a missing JSON chunk entirely (12-byte file, valid header).
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(b"glTF");
    bytes.extend_from_slice(&2u32.to_le_bytes());
    bytes.extend_from_slice(&12u32.to_le_bytes());
    let error = gltf::glb_chunks(&bytes).expect_err("no chunks must fail");
    assert_eq!(error.offset(), 12, "first chunk header would sit at byte 12");
}

#[test]
fn anonymous_nodes_are_index_keyed() {
    // Given: a minimal unnamed-node scene.
    let bytes = br#"{"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0,1]}],
        "nodes":[{},{"name":"leaf"}]}"#;
    let extracted = gltf::extract(bytes, ExtractBudgets::default())
        .expect("unnamed nodes must extract");
    // Then: anonymous nodes are keyed "#<index>" in the name-keyed view.
    let nodes = &extracted.tree["nodes"];
    assert_eq!(nodes[0]["name"], "#0");
    assert_eq!(nodes[1]["name"], "leaf");
    let roots = extracted.tree["scene"]["roots"].as_array().unwrap();
    assert_eq!(roots[0], "#0");
    assert_eq!(roots[1], "leaf");
}

#[test]
fn over_deep_hierarchy_trips_the_typed_budget() {
    // Given: a 65-node deep chain (the walk depth cap is 64); the last
    // node carries no children so the document stays index-valid.
    let mut nodes = String::from("{\"asset\":{\"version\":\"2.0\"},\"scene\":0,\"scenes\":[{\"nodes\":[0]}],\"nodes\":[");
    for index in 0..65 {
        if index > 0 {
            nodes.push(',');
        }
        if index == 64 {
            nodes.push_str("{}");
        } else {
            nodes.push_str(&format!("{{\"children\":[{}]}}", index + 1));
        }
    }
    nodes.push_str("]}");
    let error = gltf::extract(nodes.as_bytes(), ExtractBudgets::default())
        .expect_err("the 65-deep chain must trip the cap");
    assert!(matches!(error, MediaError::ExtractTooLarge), "got {error:?}");
}

#[test]
fn zero_deadline_trips_the_typed_budget() {
    // Given: any parseable input and a zero wall-clock budget.
    let bytes = br#"{"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],"nodes":[{}]}"#;
    let budgets = ExtractBudgets {
        deadline: std::time::Duration::ZERO,
        max_total_bytes: 1 << 20,
    };
    let error = gltf::extract(bytes, budgets).expect_err("zero deadline must trip");
    assert!(matches!(error, MediaError::ExtractTooLarge), "got {error:?}");
}

#[test]
fn oversized_input_is_refused_before_parsing() {
    // Given: a total-size cap smaller than the input.
    let bytes = fixture("scene.glb");
    let budgets = ExtractBudgets {
        deadline: std::time::Duration::from_secs(10),
        max_total_bytes: 8,
    };
    let error = gltf::extract(&bytes, budgets).expect_err("cap must trip");
    assert!(matches!(error, MediaError::ExtractTooLarge), "got {error:?}");
}

#[test]
fn scene_outline_projector_registers_as_text_projection() {
    // Given: the adapter's projector registered alongside it.
    let mut registry = ProjectionRegistry::new();
    gltf::register_projection(&mut registry).expect("gltf projector registers");
    // When: the committed GLB is projected.
    let context = ProjectionContext {
        budgets: ExtractBudgets::default(),
        name_hint: None,
    };
    let projected = registry
        .project(&FormatId::Gltf, harness_core::event::ProjectionKind::TextProjection,
            &fixture("scene.glb"), &context)
        .expect("text projection resolves");
    // Then: it equals the extraction's text projection.
    let extracted = gltf::extract(&fixture("scene.glb"), ExtractBudgets::default())
        .expect("glb must extract");
    assert_eq!(projected, extracted.text_projection);
    // And: a different kind is honestly typed NoProjector (one format, one
    // projector - the registry contract).
    let error = registry
        .project(&FormatId::Gltf, harness_core::event::ProjectionKind::Transcript,
            &fixture("scene.glb"), &context)
        .expect_err("transcript is not this projector's kind");
    assert!(matches!(error, MediaError::NoProjector), "got {error:?}");
}

#[test]
fn executor_dispatches_gltf_extract_with_full_disclosure() {
    // Given: the day-one registry with the gltf adapter wired.
    let registry = harness_media::executor::registry();
    // When: extract is dispatched on the committed fixture.
    let bytes = fixture("scene.gltf");
    let produced = registry
        .dispatch(FormatId::Gltf, OpRequest::Extract {
            bytes: &bytes,
            budgets: ExtractBudgets::default(),
        })
        .expect("gltf extract dispatches");
    // Then: pure tier, named engine, crate version, no command.
    let producer = produced.producer();
    assert_eq!(producer.tier, harness_media::executor::AdapterTier::Pure);
    assert_eq!(producer.engine, "harness-media::gltf");
    assert_eq!(producer.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(producer.command, None);
}

#[test]
fn descriptor_claims_match_the_bound_handlers() {
    // Given: the M2 descriptor table with the M12 claim flip.
    let registry = harness_media::probe::registry();
    let descriptor = registry.lookup(&FormatId::Gltf).expect("gltf descriptor");
    // Then: extract/generate/verify are claimed Supported (the builder's
    // descriptor/binding sync rule is backed by real handlers).
    use harness_media::descriptor::{OpSupport, Operation};
    for op in [Operation::Extract, Operation::Generate, Operation::Verify] {
        assert!(
            matches!(descriptor.ops.get(&op), Some(OpSupport::Supported)),
            "{op} must be claimed supported"
        );
    }
}

#[test]
fn projection_provider_is_usable_directly() {
    // Given: the provider type itself (the daemon wiring composes it).
    let provider = Arc::new(gltf::GltfOutline);
    assert_eq!(provider.kind(), harness_core::event::ProjectionKind::TextProjection);
}
