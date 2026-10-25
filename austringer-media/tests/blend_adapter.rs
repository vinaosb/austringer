//! M16 .blend adapter tests: the hermetic mock-blender lane (builder
//! script -> mock .blend fixture -> dump JSON -> structural diff), the
//! default-disabled typed refusals, dispatch producer disclosure, the
//! versioned dump-schema pin (mock-drift law), the mock's python-syntax
//! gate, and the `#[ignore]` real-Blender tier. Hermetic: no network,
//! no real blender, mocktool children only.

use std::collections::BTreeMap;
use std::path::Path;

use austringer_media::adapter::blend::{
    self, BlendSpec, MaterialSpec, ObjectSpec, Primitive, Transform,
};
use austringer_media::error::MediaError;
use austringer_media::executor::{OpOutput, OpRequest, Produced};
use austringer_media::format::FormatId;
use austringer_media::generate::GenerationSpec;
use austringer_media::tool::{ToolCall, ToolConfig, ToolExecutor, ToolOverride};

/// T40-style re-exec entry: when AUSTRINGER_MOCKTOOL_ARGV is set, THIS test
/// becomes a mocktool child; the executor spawns this binary as the
/// substituted blender program.
#[test]
fn mocktool_child_entry() {
    if std::env::var("AUSTRINGER_MOCKTOOL_ARGV").is_err() {
        return; // normal suite run: nothing to do
    }
    std::process::exit(austringer_test_support::mocktool::run_from_env());
}

fn mocktool_override() -> ToolOverride {
    ToolOverride {
        program: Some(
            std::env::current_exe()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
        ),
        args: vec![
            "--exact".into(),
            "mocktool_child_entry".into(),
            "--nocapture".into(),
        ],
        timeout_ms: None,
    }
}

/// The mock blender executor for one mode (`blendbuild` / `blenddump`).
fn blend_exec(mode: &str, temp: &Path) -> ToolExecutor {
    ToolExecutor::new(ToolConfig {
        enabled: [("blender".to_owned(), mocktool_override())].into(),
        install_enabled: false,
        session_temp: temp.to_path_buf(),
        env: [("AUSTRINGER_MOCKTOOL_ARGV".to_owned(), mode.to_owned())].into(),
    })
}

fn transform(location: [f64; 3], rotation: [f64; 3], scale: [f64; 3]) -> Transform {
    Transform {
        location,
        rotation_euler: rotation,
        scale,
    }
}

fn material(base_color: [f64; 4], metallic: f64, roughness: f64) -> MaterialSpec {
    MaterialSpec {
        base_color,
        metallic,
        roughness,
    }
}

fn object(
    name: &str,
    primitive: Primitive,
    transform: Transform,
    material: Option<MaterialSpec>,
) -> ObjectSpec {
    ObjectSpec {
        name: name.to_owned(),
        primitive,
        transform,
        material,
    }
}

fn spec(objects: Vec<ObjectSpec>, world: Option<[f64; 4]>, collection: &str) -> BlendSpec {
    BlendSpec {
        objects,
        world_background: world,
        collection: collection.to_owned(),
    }
}

/// The plan's acceptance corpus: >=8 specs covering every primitive,
/// materials, transforms, world backgrounds, collection layout, and the
/// unordered-by-name objects property (reverse-order twin).
fn corpus() -> Vec<(&'static str, BlendSpec)> {
    let identity = Transform {
        location: [0.0; 3],
        rotation_euler: [0.0; 3],
        scale: [1.0; 3],
    };
    let all_primitives = vec![
        object("Cube", Primitive::Cube, identity, None),
        object("Sphere", Primitive::Sphere, identity, None),
        object("Plane", Primitive::Plane, identity, None),
        object("Cylinder", Primitive::Cylinder, identity, None),
        object("Icosphere", Primitive::Icosphere, identity, None),
        object("Cone", Primitive::Cone, identity, None),
    ];
    let moved = transform([0.5, -1.25, 2.0], [0.1, -0.2, 0.3], [1.0, 2.0, 0.5]);
    vec![
        (
            "single-cube",
            spec(
                vec![object("Cube", Primitive::Cube, identity, None)],
                None,
                "Collection",
            ),
        ),
        (
            "plane-with-material",
            spec(
                vec![object(
                    "Ground",
                    Primitive::Plane,
                    transform([0.0, 0.0, -1.0], [0.0; 3], [4.0, 4.0, 1.0]),
                    Some(material([0.8, 0.2, 0.2, 1.0], 0.0, 0.9)),
                )],
                None,
                "Collection",
            ),
        ),
        (
            "all-primitives",
            spec(all_primitives.clone(), None, "Collection"),
        ),
        (
            "transformed-sphere",
            spec(
                vec![object(
                    "Ball",
                    Primitive::Sphere,
                    moved,
                    Some(material([0.1, 0.3, 0.9, 1.0], 0.9, 0.15)),
                )],
                None,
                "Collection",
            ),
        ),
        ("empty-scene", spec(Vec::new(), None, "Collection")),
        (
            "world-only",
            spec(Vec::new(), Some([0.05, 0.02, 0.08, 1.0]), "Collection"),
        ),
        (
            "cylinder-cone-rotations",
            spec(
                vec![
                    object(
                        "Pillar",
                        Primitive::Cylinder,
                        transform(
                            [1.0, 0.0, 0.0],
                            [0.0, 0.0, std::f64::consts::FRAC_PI_2],
                            [1.0; 3],
                        ),
                        Some(material([0.5, 0.5, 0.5, 1.0], 1.0, 0.4)),
                    ),
                    object(
                        "Tip",
                        Primitive::Cone,
                        transform(
                            [-1.0, 0.5, 0.25],
                            [std::f64::consts::FRAC_PI_4, 0.0, 0.0],
                            [0.5, 0.5, 1.5],
                        ),
                        None,
                    ),
                ],
                Some([0.2, 0.25, 0.3, 1.0]),
                "Collection",
            ),
        ),
        (
            "icosphere-custom-collection",
            spec(
                vec![object(
                    "Gem",
                    Primitive::Icosphere,
                    transform([0.0, 3.0, 1.0], [0.0, 0.4, 0.0], [2.0, 2.0, 2.0]),
                    Some(material([0.9, 0.8, 0.1, 1.0], 0.2, 0.05)),
                )],
                None,
                "Props",
            ),
        ),
        (
            "reversed-order-twin",
            spec(
                all_primitives.into_iter().rev().collect(),
                None,
                "Collection",
            ),
        ),
        (
            "kitchen-sink",
            spec(
                vec![
                    object(
                        "Floor",
                        Primitive::Plane,
                        transform([0.0, 0.0, -2.0], [0.0; 3], [10.0, 10.0, 1.0]),
                        Some(material([0.9, 0.9, 0.85, 1.0], 0.0, 0.95)),
                    ),
                    object(
                        "Table",
                        Primitive::Cube,
                        transform(
                            [0.0, 0.0, 0.5],
                            [0.0, 0.0, std::f64::consts::FRAC_PI_6],
                            [2.0, 1.0, 0.1],
                        ),
                        Some(material([0.4, 0.25, 0.1, 1.0], 0.0, 0.6)),
                    ),
                    object(
                        "Globe",
                        Primitive::Sphere,
                        transform([0.4, 0.0, 1.1], [0.0; 3], [0.5, 0.5, 0.5]),
                        Some(material([0.0, 0.4, 0.8, 1.0], 0.1, 0.3)),
                    ),
                    object(
                        "Cap",
                        Primitive::Cone,
                        transform([-0.6, 0.3, 0.75], [0.0, 0.0, -0.2], [0.3, 0.3, 0.4]),
                        None,
                    ),
                    object(
                        "Bit",
                        Primitive::Icosphere,
                        transform([1.2, -0.8, 0.6], [0.3, 0.3, 0.3], [0.2, 0.2, 0.2]),
                        Some(material([1.0, 1.0, 1.0, 1.0], 0.95, 0.02)),
                    ),
                    object(
                        "Bar",
                        Primitive::Cylinder,
                        transform(
                            [1.5, 1.5, 0.0],
                            [
                                std::f64::consts::FRAC_PI_2,
                                0.0,
                                std::f64::consts::FRAC_PI_4,
                            ],
                            [0.1, 0.1, 3.0],
                        ),
                        None,
                    ),
                ],
                Some([0.03, 0.05, 0.07, 1.0]),
                "Stage",
            ),
        ),
    ]
}

fn generation(spec: &BlendSpec) -> GenerationSpec {
    GenerationSpec {
        format: FormatId::Blend,
        params: serde_json::to_value(spec).unwrap_or_default(),
    }
}

/// ROUND-TRIP (the M16 acceptance gate): spec corpus >= 8 ->
/// generate -> (mock) .blend -> probe Certain -> extract -> empty
/// structural diff against the INDEPENDENT canonical tree.
#[test]
fn spec_corpus_roundtrips_through_the_mock_lane() {
    for (name, blend_spec) in corpus() {
        let temp = tempfile::tempdir().expect("tempdir");
        let builder = blend_exec("blendbuild", temp.path());
        let dumper = blend_exec("blenddump", temp.path());
        let produced = blend::generate(&builder, &generation(&blend_spec))
            .unwrap_or_else(|error| panic!("{name}: generate: {error:?}"));
        let bytes = generate_bytes(&produced);
        assert!(
            bytes.starts_with(b"BLENDER"),
            "{name}: generated artifact carries the blend magic"
        );
        let probe = austringer_media::probe::probe(&bytes, None);
        assert!(
            matches!(
                probe,
                austringer_media::probe::ProbeResult::Identified {
                    format: FormatId::Blend,
                    confidence: austringer_media::probe::Confidence::Certain,
                    ..
                }
            ),
            "{name}: probe Certain"
        );
        let verdict = blend::verify(&dumper, &blend_spec.canonical_tree(), &bytes)
            .unwrap_or_else(|error| panic!("{name}: verify: {error:?}"));
        let verified = match verdict.inner() {
            OpOutput::Verify(austringer_media::diff::RoundTripVerdict::Pass) => true,
            OpOutput::Verify(austringer_media::diff::RoundTripVerdict::Fail(diff)) => {
                panic!("{name}: non-empty diff: {diff:?}")
            }
            _ => false,
        };
        assert!(verified, "{name}: verify verdict is Pass");
    }
}

fn generate_bytes(produced: &Produced) -> Vec<u8> {
    match produced.inner() {
        OpOutput::Generate(bytes) => bytes.clone(),
        other => panic!("expected Generate output, got {other:?}"),
    }
}

/// DEFAULT-DISABLED: the shipped registry answers blend extract with the
/// typed ToolUnavailable (blender is opt-in); probe still works pure.
#[test]
fn default_config_extract_is_typed_tool_unavailable_probe_still_works() {
    let registry = austringer_media::executor::registry();
    let error = registry
        .dispatch(
            FormatId::Blend,
            OpRequest::Extract {
                bytes: b"BLENDER_v305",
                budgets: Default::default(),
            },
        )
        .expect_err("default: blender not enabled");
    match error {
        MediaError::ToolUnavailable { tool, install_hint } => {
            assert_eq!(tool, "blender");
            assert!(!install_hint.is_empty(), "actionable hint");
        }
        other => panic!("expected ToolUnavailable, got {other:?}"),
    }
    let fixture = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join("blend.blend"),
    )
    .expect("committed blend fixture");
    let probed = registry
        .dispatch(
            FormatId::Blend,
            OpRequest::Probe {
                bytes: &fixture,
                name_hint: None,
            },
        )
        .expect("probe never needs the tool");
    assert!(matches!(probed.inner(), OpOutput::Probe(_)));
}

/// DISPATCH: generate through the registry carries the subprocess
/// producer disclosure (engine blender, command line, manifest pin).
#[test]
fn dispatch_generate_carries_blender_producer_disclosure() {
    let temp = tempfile::tempdir().expect("tempdir");
    let registry =
        austringer_media::executor::registry().with_tools(blend_exec("blendbuild", temp.path()));
    let (_, blend_spec) = corpus().remove(0);
    let produced = registry
        .dispatch(
            FormatId::Blend,
            OpRequest::Generate {
                spec: &generation(&blend_spec),
            },
        )
        .expect("dispatch generate through the mock lane");
    let producer = produced.producer();
    assert_eq!(producer.engine, "blender");
    assert_eq!(
        producer.tier,
        austringer_media::executor::AdapterTier::Subprocess
    );
    // The disclosure carries the ACTUAL command line: the mock
    // substitution argv here (honest - the real blender argv belongs to
    // the #[ignore] real-tool tier); the manifest template below pins
    // the real invocation shape.
    assert!(
        producer
            .command
            .as_deref()
            .is_some_and(|command| command.contains("--exact"))
    );
    let spec = austringer_media::tool::manifests()
        .get("blender")
        .expect("blender manifest");
    let build_args = spec
        .op_template("scene-build")
        .expect("scene-build op")
        .args;
    assert!(build_args.contains(&"--background"));
    assert!(build_args.contains(&"--factory-startup"));
    let dump_args = spec.op_template("scene-dump").expect("scene-dump op").args;
    assert!(dump_args.contains(&"--factory-startup"));
    assert!(!generate_bytes(&produced).is_empty());
}

/// NON-ZERO EXIT: a failing mock blender surfaces the typed NonZeroExit
/// with the bounded stderr head - and no partial artifact escapes.
#[test]
fn mock_failure_is_typed_nonzero_exit_no_partial_artifact() {
    let temp = tempfile::tempdir().expect("tempdir");
    let failing = ToolExecutor::new(ToolConfig {
        enabled: [("blender".to_owned(), mocktool_override())].into(),
        install_enabled: false,
        session_temp: temp.path().to_path_buf(),
        env: [(
            "AUSTRINGER_MOCKTOOL_ARGV".to_owned(),
            "exit 3 blender-boom".to_owned(),
        )]
        .into(),
    });
    let (_, blend_spec) = corpus().remove(0);
    let error = blend::generate(&failing, &generation(&blend_spec))
        .expect_err("failing tool exit is typed");
    match error {
        MediaError::NonZeroExit { code, stderr_head } => {
            assert_eq!(code, 3);
            assert!(
                stderr_head.contains("blender-boom"),
                "stderr head carries why"
            );
        }
        other => panic!("expected NonZeroExit, got {other:?}"),
    }
}

/// MOCK-DRIFT LAW: the dump-script's stdout JSON contract is pinned by a
/// versioned schema test (top-level and per-object key sets, schema id).
#[test]
fn dump_schema_contract_is_version_pinned() {
    let temp = tempfile::tempdir().expect("tempdir");
    let builder = blend_exec("blendbuild", temp.path());
    let dumper = blend_exec("blenddump", temp.path());
    let (_, blend_spec) = corpus().remove(2); // all-primitives
    let bytes =
        generate_bytes(&blend::generate(&builder, &generation(&blend_spec)).expect("generate"));
    let extracted = blend::extract(&dumper, &bytes, Default::default()).expect("extract");
    let tree = match extracted.inner() {
        OpOutput::Extract(extracted) => extracted.tree.clone(),
        other => panic!("expected Extract output, got {other:?}"),
    };
    let top: Vec<&str> = tree
        .as_object()
        .expect("tree root is an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        top,
        vec![
            "action_count",
            "collections",
            "objects",
            "scenes",
            "schema",
            "world",
        ]
    );
    assert_eq!(tree["schema"], blend::DUMP_SCHEMA);
    let first = tree["objects"][0]
        .as_object()
        .expect("object entry is an object");
    let keys: Vec<&str> = first.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        vec![
            "material_slots",
            "mesh",
            "modifiers",
            "name",
            "parent",
            "type",
            "world_transform",
        ]
    );
    let mesh = first["mesh"].as_object().expect("mesh stats");
    assert_eq!(
        mesh.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["edges", "faces", "verts"]
    );
}

/// TEXT PROJECTION: the extract projection is a bounded scene outline a
/// text-only model can read (object names present, char ceiling held).
#[test]
fn text_projection_is_a_bounded_scene_outline() {
    let temp = tempfile::tempdir().expect("tempdir");
    let builder = blend_exec("blendbuild", temp.path());
    let dumper = blend_exec("blenddump", temp.path());
    let (_, blend_spec) = corpus().remove(2); // all-primitives
    let bytes =
        generate_bytes(&blend::generate(&builder, &generation(&blend_spec)).expect("generate"));
    let extracted = blend::extract(&dumper, &bytes, Default::default()).expect("extract");
    let projection = match extracted.inner() {
        OpOutput::Extract(extracted) => extracted.text_projection.clone(),
        other => panic!("expected Extract output, got {other:?}"),
    };
    for name in ["Cube", "Sphere", "Cylinder", "Icosphere", "Cone"] {
        assert!(
            projection.contains(name),
            "outline names {name}: {projection}"
        );
    }
    assert!(projection.len() <= austringer_media::extract::TEXT_PROJECTION_MAX_CHARS);
}

/// HERMETIC HARDENING: the mock blender mode runs the emitted builder
/// script through python validation - a syntactically broken script
/// fails HERMETICALLY (typed NonZeroExit from the mock).
#[test]
fn syntactically_broken_builder_script_fails_hermetically() {
    let temp = tempfile::tempdir().expect("tempdir");
    let dir = temp.path().join("blend-build");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("build.py"), "def broken(:\n    pass\n").expect("write script");
    let error = blend_exec("blendbuild", temp.path())
        .run(&ToolCall::new("blender", "scene-build"))
        .expect_err("broken python refuses in the mock");
    match error {
        austringer_media::tool::ToolError::NonZeroExit { stderr_head, .. } => {
            assert!(
                stderr_head.to_lowercase().contains("python"),
                "stderr names the python validation: {stderr_head}"
            );
        }
        other => panic!("expected NonZeroExit, got {other:?}"),
    }
}

/// The generator side of the same guarantee: every emitted builder
/// script parses as python (validated through the mocktool validator).
#[test]
fn generated_builder_scripts_parse_as_python() {
    for (name, blend_spec) in corpus() {
        let source = blend::builder_script(&blend_spec);
        austringer_test_support::mocktool::validate_python(&source)
            .unwrap_or_else(|error| panic!("{name}: builder script is valid python: {error}"));
    }
    assert!(austringer_test_support::mocktool::validate_python("def broken(:").is_err());
}

/// HONESTY: bytes without mock census (the committed probe fixture is a
/// bare header) cannot dump hermetically - typed failure, never a fake
/// scene.
#[test]
fn blend_bytes_without_census_fail_typed_not_faked() {
    let temp = tempfile::tempdir().expect("tempdir");
    let fixture = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join("blend.blend"),
    )
    .expect("committed blend fixture");
    let error = blend::extract(
        &blend_exec("blenddump", temp.path()),
        &fixture,
        Default::default(),
    )
    .expect_err("no census in the fixture: typed refusal");
    assert!(
        matches!(error, MediaError::NonZeroExit { .. }),
        "expected NonZeroExit, got {error:?}"
    );
}

/// INVALID SPEC: format mismatch, unknown primitive, non-finite floats,
/// oversized names and object counts are the typed InvalidSpec.
#[test]
fn invalid_specs_are_typed_refusals() {
    let wrong_format = GenerationSpec {
        format: FormatId::Svg,
        params: serde_json::json!({}),
    };
    assert!(matches!(
        blend::generate(&blend_exec("blendbuild", Path::new(".")), &wrong_format),
        Err(MediaError::InvalidSpec)
    ));
    let bad_primitive = GenerationSpec {
        format: FormatId::Blend,
        params: serde_json::json!({"objects": [{"name": "X", "primitive": "torus"}]}),
    };
    let temp = tempfile::tempdir().expect("tempdir");
    assert!(matches!(
        blend::generate(&blend_exec("blendbuild", temp.path()), &bad_primitive),
        Err(MediaError::InvalidSpec)
    ));
    let nan_transform = spec(
        vec![object(
            "NaN",
            Primitive::Cube,
            transform([f64::NAN, 0.0, 0.0], [0.0; 3], [1.0; 3]),
            None,
        )],
        None,
        "Collection",
    );
    assert!(matches!(
        blend::generate(
            &blend_exec("blendbuild", temp.path()),
            &generation(&nan_transform)
        ),
        Err(MediaError::InvalidSpec)
    ));
    let too_many = spec(
        (0..=blend::MAX_OBJECTS)
            .map(|index| {
                object(
                    &format!("O{index}"),
                    Primitive::Cube,
                    Transform {
                        location: [0.0; 3],
                        rotation_euler: [0.0; 3],
                        scale: [1.0; 3],
                    },
                    None,
                )
            })
            .collect(),
        None,
        "Collection",
    );
    assert!(matches!(
        blend::generate(
            &blend_exec("blendbuild", temp.path()),
            &generation(&too_many)
        ),
        Err(MediaError::InvalidSpec)
    ));
}

/// REAL-TOOL TIER: one real blender over the primitive corpus (generate,
/// extract, and diff). Never runs in CI; run at-todo-time or on the M25
/// cadence with `BLENDER_BIN=<blender.exe>` and
/// `cargo nextest run -p austringer-media --run-ignored only --test
/// blend_adapter`.
#[test]
#[ignore = "real-tool tier: needs Blender (4.5 LTS or 5.x) via BLENDER_BIN; never CI"]
fn real_blender_smoke_round_trips_the_primitive_corpus() {
    let bin = std::env::var_os("BLENDER_BIN").unwrap_or_else(|| {
        panic!(
            "set BLENDER_BIN to a blender executable (4.5 LTS or 5.x; blender.org builds); \
             hermetic CI never spawns it"
        )
    });
    let env: BTreeMap<String, String> = ["PATH", "SYSTEMROOT", "SYSTEMDRIVE", "TEMP", "TMP"]
        .iter()
        .filter_map(|key| {
            std::env::var(key)
                .ok()
                .map(|value| ((*key).to_owned(), value))
        })
        .collect();
    for (name, blend_spec) in corpus() {
        let temp = tempfile::tempdir().expect("tempdir");
        let tools = ToolExecutor::new(ToolConfig {
            enabled: [(
                "blender".to_owned(),
                ToolOverride {
                    program: Some(bin.to_string_lossy().into_owned()),
                    args: Vec::new(),
                    timeout_ms: None,
                },
            )]
            .into(),
            install_enabled: false,
            session_temp: temp.path().to_path_buf(),
            env: env.clone(),
        });
        let bytes = generate_bytes(
            &blend::generate(&tools, &generation(&blend_spec))
                .unwrap_or_else(|error| panic!("{name}: real generate: {error:?}")),
        );
        assert!(bytes.starts_with(b"BLENDER"), "{name}: real .blend magic");
        let verdict = blend::verify(&tools, &blend_spec.canonical_tree(), &bytes)
            .unwrap_or_else(|error| panic!("{name}: real verify: {error:?}"));
        assert!(
            matches!(
                verdict.inner(),
                OpOutput::Verify(austringer_media::diff::RoundTripVerdict::Pass)
            ),
            "{name}: real blender round-trip passes the structural diff"
        );
    }
}
