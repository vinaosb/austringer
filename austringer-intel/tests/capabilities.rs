//! Per-model media capability flags (plan todo M7, media-formats wave):
//! `Catalog::capabilities` is fail-closed, source-cited, and PER-MODEL —
//! never inferred from family. Every seeded flag cites its source row
//! (the sidecar table in `austringer-intel/src/seed.rs` docs: provider docs
//! verified in harness-research SS33.8 + `.omo/research/model-profiles.md`
//! rows). Absent or unknown support = false — the same fail-closed
//! discipline as the 400-trap table in `traps.rs`. Override semantics
//! live in `capability_overrides.rs`.

use std::collections::BTreeMap;

use austringer_intel::catalog::{Capabilities, Catalog, CatalogEntry, UnknownModel};

/// The pinned capability JSON of the whole seeded table (BTreeMap =
/// sorted model keys; struct field order vision/audio_input/video_input).
/// This snapshot is the drift catcher: any seeded-flag change must
/// consciously update it here.
const SNAPSHOT: &str = r#"{
  "claude-fable-5-1": {
    "vision": true,
    "audio_input": false,
    "video_input": false
  },
  "claude-haiku-5-5": {
    "vision": false,
    "audio_input": false,
    "video_input": false
  },
  "claude-opus-4-6": {
    "vision": true,
    "audio_input": false,
    "video_input": false
  },
  "claude-opus-4-7": {
    "vision": true,
    "audio_input": false,
    "video_input": false
  },
  "claude-opus-5": {
    "vision": true,
    "audio_input": false,
    "video_input": false
  },
  "claude-opus-5-5": {
    "vision": false,
    "audio_input": false,
    "video_input": false
  },
  "claude-sonnet-4-6": {
    "vision": true,
    "audio_input": false,
    "video_input": false
  },
  "claude-sonnet-5-5": {
    "vision": false,
    "audio_input": false,
    "video_input": false
  },
  "deepseek-reasoner": {
    "vision": false,
    "audio_input": false,
    "video_input": false
  },
  "gemini-2-5-pro": {
    "vision": true,
    "audio_input": true,
    "video_input": true
  },
  "gemini-3-1-pro": {
    "vision": true,
    "audio_input": true,
    "video_input": true
  },
  "gemini-3-pro": {
    "vision": true,
    "audio_input": true,
    "video_input": true
  },
  "gemini-4-argon": {
    "vision": false,
    "audio_input": false,
    "video_input": false
  },
  "gpt-5-5": {
    "vision": true,
    "audio_input": false,
    "video_input": false
  },
  "gpt-6-1-sol": {
    "vision": false,
    "audio_input": false,
    "video_input": false
  },
  "gpt-6-astra": {
    "vision": true,
    "audio_input": false,
    "video_input": false
  },
  "gpt-6-luna": {
    "vision": false,
    "audio_input": false,
    "video_input": false
  },
  "gpt-6-sol": {
    "vision": false,
    "audio_input": false,
    "video_input": false
  },
  "grok-4-6": {
    "vision": true,
    "audio_input": false,
    "video_input": false
  },
  "grok-4-7": {
    "vision": false,
    "audio_input": false,
    "video_input": false
  }
}"#;

#[test]
fn per_model_capability_table_is_pinned() {
    // Given: the seeded catalog. When/Then: every model's three flags
    // match this table exactly (the exhaustive per-model matrix).
    let catalog = Catalog::seeded();
    let vision_only = Capabilities {
        vision: true,
        audio_input: false,
        video_input: false,
    };
    let expected: &[(&str, Capabilities)] = &[
        // platform.claude.com image-input docs; Messages API has no
        // audio/video input blocks.
        ("claude-opus-4-6", vision_only),
        ("claude-opus-4-7", vision_only),
        ("claude-sonnet-4-6", vision_only),
        ("claude-fable-5-1", vision_only),
        ("claude-opus-5", vision_only),
        // developers.openai.com Responses image input (GPT-5/6 series).
        ("gpt-5-5", vision_only),
        ("gpt-6-astra", vision_only),
        // ai.google.dev: Gemini Pro accepts image/audio/video input.
        (
            "gemini-2-5-pro",
            Capabilities {
                vision: true,
                audio_input: true,
                video_input: true,
            },
        ),
        (
            "gemini-3-pro",
            Capabilities {
                vision: true,
                audio_input: true,
                video_input: true,
            },
        ),
        (
            "gemini-3-1-pro",
            Capabilities {
                vision: true,
                audio_input: true,
                video_input: true,
            },
        ),
        // docs.x.ai grok-4 image input; no audio/video input documented.
        ("grok-4-6", vision_only),
        // DeepSeek API is text-only (model-profiles.md SS3 "DeepSeek").
        ("deepseek-reasoner", Capabilities::default()),
    ];
    for (model, caps) in expected {
        assert_eq!(
            catalog.capabilities(model),
            Ok(*caps),
            "seeded flags for {model} diverge from the pinned table"
        );
    }
}

#[test]
fn seeded_rows_match_their_cited_source_rows() {
    // Spot-checks tie the seed DATA to its cited sources — snapshot
    // stability alone cannot catch wrong seed data (plan M7 acceptance).
    let catalog = Catalog::seeded();

    // Spot-check 1: ai.google.dev Gemini input docs (harness-research
    // SS33.8 verified) document image AND audio AND video input for
    // Gemini Pro models; plan M8 names Google inline_data native wire.
    assert_eq!(
        catalog.capabilities("gemini-3-pro"),
        Ok(Capabilities {
            vision: true,
            audio_input: true,
            video_input: true,
        })
    );

    // Spot-check 2: platform.claude.com Messages API documents image
    // input; no audio/video input content blocks exist (M8 Anthropic
    // native wire = image source only).
    assert_eq!(
        catalog.capabilities("claude-sonnet-4-6"),
        Ok(Capabilities {
            vision: true,
            audio_input: false,
            video_input: false,
        })
    );

    // Spot-check 3: DeepSeek's API is text-only — no image input
    // endpoint for deepseek-reasoner; model-profiles.md SS3 "DeepSeek"
    // describes a text-editing profile only.
    assert_eq!(
        catalog.capabilities("deepseek-reasoner"),
        Ok(Capabilities::default())
    );

    // Spot-check 4: docs.x.ai documents grok-4 image input; the M8 xAI
    // native wire is image_url only — no audio/video input documented.
    assert_eq!(
        catalog.capabilities("grok-4-6"),
        Ok(Capabilities {
            vision: true,
            audio_input: false,
            video_input: false,
        })
    );
}

#[test]
fn text_only_model_has_all_three_flags_false() {
    // Plan M7 acceptance: at least one seeded text-only model keeps all
    // three flags false (fail-closed; delivery plans text projections).
    let caps = Catalog::seeded()
        .capabilities("deepseek-reasoner")
        .expect("seeded model");
    assert_eq!(caps, Capabilities::default());
}

#[test]
fn unknown_or_misspelled_model_is_a_typed_error_zero_panic() {
    // Given: ids that are not exact seeded model ids. When: looked up.
    // Then: typed UnknownModel naming the offender — never a guess,
    // never a panic (malformed-input channel).
    let catalog = Catalog::seeded();
    for bogus in ["gpt-5.6-turbo", "claude-opus-4-6 ", "GPT-5-5", "", "sonnet"] {
        let Err(err) = catalog.capabilities(bogus) else {
            panic!("{bogus:?} must miss the catalog");
        };
        assert_eq!(
            err,
            UnknownModel {
                model: bogus.to_string()
            }
        );
        assert!(!err.to_string().is_empty());
    }
}

#[test]
fn capabilities_json_snapshot_is_stable() {
    // Given: the whole seeded table serialized model-sorted. Then: the
    // JSON is byte-identical to the pinned snapshot — any drift in
    // seeded flags surfaces here consciously (stale-state catcher).
    let catalog = Catalog::seeded();
    let table: BTreeMap<&str, Capabilities> = catalog
        .entries()
        .map(|e| (e.model.as_str(), e.capabilities))
        .collect();
    let json = serde_json::to_string_pretty(&table).expect("serialize");
    assert_eq!(json, SNAPSHOT);
}

#[test]
fn entry_json_without_capabilities_field_fails_closed() {
    // Given: an entry serialized WITHOUT the capabilities key (older
    // data / partial writer). When: deserialized. Then: every flag is
    // false — absent support never upgrades to true.
    let seeded_catalog = Catalog::seeded();
    let original = seeded_catalog.entry("gemini-3-pro").expect("seeded");
    assert!(
        original.capabilities.vision,
        "fixture must carry a true flag for the strip to mean anything"
    );
    let mut value = serde_json::to_value(original).expect("serialize");
    value
        .as_object_mut()
        .expect("entries serialize to JSON objects")
        .remove("capabilities");
    let stripped: CatalogEntry =
        serde_json::from_str(&value.to_string()).expect("entry parses without capabilities");
    assert_eq!(stripped.capabilities, Capabilities::default());
}

#[test]
fn capability_table_printout_for_five_flagship_models() {
    // MANUAL-QA surface (plan M7): the human-readable capability table.
    // Run with --nocapture; the output is recorded in the task evidence.
    let catalog = Catalog::seeded();
    let flagship = [
        "claude-opus-4-7",
        "gpt-6-astra",
        "gemini-3-pro",
        "grok-4-6",
        "deepseek-reasoner",
    ];
    let mut table = String::new();
    for model in flagship {
        let caps = catalog
            .capabilities(model)
            .expect("flagship model is seeded");
        table.push_str(&format!(
            "{model}: vision={} audio_input={} video_input={}\n",
            caps.vision, caps.audio_input, caps.video_input
        ));
    }
    print!("{table}");
    assert_eq!(table.lines().count(), 5, "five flagship rows printed");
    for model in flagship {
        assert!(table.contains(model), "table must list {model}");
    }
}
