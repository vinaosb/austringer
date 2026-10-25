//! WIP-diff context mode + edit-format routing per model family.

use austringer_tools::formats::{EditFormat, ModelFamily, default_format, edit_schema_variant};
use austringer_tools::hunks::Replacement;
use austringer_tools::wip::{WipFileDiff, wip_context};
use std::path::PathBuf;

#[test]
fn wip_context_renders_the_anchoring_block() {
    let modified = vec![
        WipFileDiff {
            path: PathBuf::from("src/main.rs"),
            untracked: false,
            hunks: vec![
                Replacement {
                    old_start: 10,
                    old_end: 12,
                    new_start: 10,
                    new_end: 14,
                },
                Replacement {
                    old_start: 40,
                    old_end: 40,
                    new_start: 42,
                    new_end: 45,
                },
            ],
        },
        WipFileDiff {
            path: PathBuf::from("src/lib.rs"),
            untracked: false,
            hunks: vec![Replacement {
                old_start: 90,
                old_end: 91,
                new_start: 95,
                new_end: 95,
            }],
        },
    ];
    let untracked = vec![(PathBuf::from("notes.md"), 30usize)];
    let context = wip_context(&modified, &untracked);
    let rendered = &context.render;
    assert!(
        rendered.contains("WIP context: 2 modified, 1 untracked"),
        "{rendered}"
    );
    assert!(
        rendered.contains("src/main.rs: 2 hunk(s), +9/-4"),
        "{rendered}"
    );
    assert!(
        rendered.contains("src/lib.rs: 1 hunk(s), +1/-2"),
        "{rendered}"
    );
    assert!(
        rendered.contains("?? notes.md (untracked, 30 lines)"),
        "{rendered}"
    );
    // Typed descriptor stays machine-readable alongside the render.
    assert_eq!(context.files.len(), 2);
    assert_eq!(context.untracked.len(), 1);
}

#[test]
fn wip_context_empty_state_is_explicit() {
    let context = wip_context(&[], &[]);
    assert!(
        context.render.contains("no work in progress"),
        "{}",
        context.render
    );
}

#[test]
fn format_routing_table_matches_plan_families() {
    assert_eq!(
        default_format(ModelFamily::OpenAi),
        EditFormat::UnifiedPatch
    );
    assert_eq!(
        default_format(ModelFamily::Anthropic),
        EditFormat::AnchorReplace
    );
}

#[test]
fn schema_variants_carry_their_trained_shapes() {
    let patch = edit_schema_variant(EditFormat::UnifiedPatch);
    assert_eq!(patch.format, EditFormat::UnifiedPatch);
    let props = patch.parameters["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("missing props"));
    assert!(props.contains_key("path"), "patch variant: {patch:?}");
    assert!(
        props.contains_key("patch") && props["patch"]["type"] == "string",
        "patch variant must be unified-diff flavored: {patch:?}"
    );
    assert_eq!(
        patch.parameters["required"],
        serde_json::json!(["path", "patch"])
    );

    let anchor = edit_schema_variant(EditFormat::AnchorReplace);
    let props = anchor.parameters["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("missing props"));
    assert!(props.contains_key("path"));
    let edits = &props["edits"]["items"]["properties"];
    for key in ["line", "old_text", "new_text"] {
        assert!(
            edits[key].is_object(),
            "anchor variant must keep the core edit_lines shape: {anchor:?}"
        );
    }
    assert!(
        edits["hash"].is_object(),
        "hashline anchor is an optional extension point"
    );
    assert_eq!(
        anchor.parameters["required"],
        serde_json::json!(["path", "edits"])
    );
}

#[test]
fn routing_document_records_the_schema_similarity_rule() {
    // The routing table itself is the exposed, typed artifact (consumed by
    // the T34 profile subsystem); its schema variants must round-trip
    // through serialization for embedding in profiles.
    let schema = edit_schema_variant(default_format(ModelFamily::OpenAi));
    let serialized = serde_json::to_string(&schema).unwrap_or_else(|e| panic!("{e}"));
    assert!(serialized.contains("UnifiedPatch"), "{serialized}");
}
