//! M20 STEP adapter tests: the pure ISO-10303-21 tokenizer over the
//! hand-committed fixture (pinned tree), the hermetic mock-freecadcmd
//! round-trip (builder script -> mock .step -> topology census ->
//! structural diff), the default-disabled typed refusals (generate via
//! the subprocess lane, extract/probe stay pure), dispatch producer
//! disclosure, the versioned topology-schema pin (mock-drift law), the
//! mock's python-syntax gate, the determinism-contract epoch pin, and
//! the `#[ignore]` real-freecadcmd tier. Hermetic: no network, no real
//! FreeCAD, mocktool children only.

use std::collections::BTreeMap;
use std::path::Path;

use harness_media::adapter::step::{self, SolidShape, SolidSpec, StepSpec, Transform};
use harness_media::error::MediaError;
use harness_media::executor::{OpOutput, OpRequest, Produced};
use harness_media::format::FormatId;
use harness_media::generate::GenerationSpec;

/// T40-style re-exec entry: when HARNESS_MOCKTOOL_ARGV is set, THIS test
/// becomes a mocktool child; the executor spawns this binary as the
/// substituted freecadcmd program.
#[test]
fn mocktool_child_entry() {
    if std::env::var("HARNESS_MOCKTOOL_ARGV").is_err() {
        return; // normal suite run: nothing to do
    }
    std::process::exit(harness_test_support::mocktool::run_from_env());
}

fn mocktool_override() -> harness_media::tool::ToolOverride {
    harness_media::tool::ToolOverride {
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

/// The mock freecadcmd executor for one mode (`stepbuild` / `stepdump`).
fn step_exec(mode: &str, temp: &Path) -> harness_media::tool::ToolExecutor {
    harness_media::tool::ToolExecutor::new(harness_media::tool::ToolConfig {
        enabled: [("freecadcmd".to_owned(), mocktool_override())].into(),
        install_enabled: false,
        session_temp: temp.to_path_buf(),
        env: [("HARNESS_MOCKTOOL_ARGV".to_owned(), mode.to_owned())].into(),
    })
}

fn transform(location: [f64; 3], rotation_xyz: [f64; 3]) -> Transform {
    Transform {
        location,
        rotation_xyz,
    }
}

fn solid(name: &str, shape: SolidShape, transform: Transform) -> SolidSpec {
    SolidSpec {
        name: name.to_owned(),
        shape,
        transform,
    }
}

/// The plan's acceptance corpus: >=8 specs covering every primitive,
/// transforms (translation + rotations), multi-solid stacks, ordering,
/// and the determinism epoch pin.
fn corpus() -> Vec<(&'static str, StepSpec)> {
    let identity = transform([0.0; 3], [0.0; 3]);
    vec![
        (
            "single-box",
            StepSpec {
                solids: vec![solid(
                    "Block",
                    SolidShape::Box {
                        size: [2.0, 3.0, 4.0],
                    },
                    identity,
                )],
            },
        ),
        (
            "single-cylinder",
            StepSpec {
                solids: vec![solid(
                    "Pin",
                    SolidShape::Cylinder {
                        radius: 1.5,
                        height: 2.0,
                    },
                    identity,
                )],
            },
        ),
        (
            "single-sphere",
            StepSpec {
                solids: vec![solid("Ball", SolidShape::Sphere { radius: 2.0 }, identity)],
            },
        ),
        (
            "all-primitives",
            StepSpec {
                solids: vec![
                    solid(
                        "Block",
                        SolidShape::Box {
                            size: [1.0, 2.0, 3.0],
                        },
                        identity,
                    ),
                    solid(
                        "Pin",
                        SolidShape::Cylinder {
                            radius: 1.0,
                            height: 4.0,
                        },
                        identity,
                    ),
                    solid("Ball", SolidShape::Sphere { radius: 1.5 }, identity),
                ],
            },
        ),
        (
            "translated-rotated-box",
            StepSpec {
                solids: vec![solid(
                    "Wedge",
                    SolidShape::Box {
                        size: [2.0, 2.0, 2.0],
                    },
                    transform([5.0, -1.0, 0.5], [0.2, -0.35, 0.7]),
                )],
            },
        ),
        (
            "rotated-cylinder",
            StepSpec {
                solids: vec![solid(
                    "Axle",
                    SolidShape::Cylinder {
                        radius: 0.75,
                        height: 5.0,
                    },
                    transform([0.0; 3], [std::f64::consts::FRAC_PI_2, 0.0, 0.0]),
                )],
            },
        ),
        (
            "five-solid-stack",
            StepSpec {
                solids: (0..5)
                    .map(|index| {
                        solid(
                            &format!("S{index}"),
                            SolidShape::Box {
                                size: [1.0, 1.0, 0.5],
                            },
                            transform([0.0, 0.0, index as f64 * 0.5], [0.0; 3]),
                        )
                    })
                    .collect(),
            },
        ),
        (
            "reordered-twin",
            StepSpec {
                solids: vec![
                    solid("Ball", SolidShape::Sphere { radius: 1.5 }, identity),
                    solid(
                        "Pin",
                        SolidShape::Cylinder {
                            radius: 1.0,
                            height: 4.0,
                        },
                        identity,
                    ),
                    solid(
                        "Block",
                        SolidShape::Box {
                            size: [1.0, 2.0, 3.0],
                        },
                        identity,
                    ),
                ],
            },
        ),
        (
            "kitchen-sink",
            StepSpec {
                solids: vec![
                    solid(
                        "Base",
                        SolidShape::Box {
                            size: [10.0, 10.0, 0.5],
                        },
                        identity,
                    ),
                    solid(
                        "Post",
                        SolidShape::Cylinder {
                            radius: 0.5,
                            height: 6.0,
                        },
                        transform([2.0, 2.0, 0.25], [0.0; 3]),
                    ),
                    solid(
                        "Knob",
                        SolidShape::Sphere { radius: 1.25 },
                        transform([-2.5, 1.5, 1.0], [0.3, 0.3, 0.3]),
                    ),
                ],
            },
        ),
        (
            "tiny-sphere",
            StepSpec {
                solids: vec![solid(
                    "Dust",
                    SolidShape::Sphere { radius: 0.001 },
                    identity,
                )],
            },
        ),
        (
            "large-sphere",
            StepSpec {
                solids: vec![solid(
                    "Tank",
                    SolidShape::Sphere { radius: 100.0 },
                    transform([0.0; 3], [0.4, 0.0, 0.9]),
                )],
            },
        ),
    ]
}

fn generation(spec: &StepSpec) -> GenerationSpec {
    GenerationSpec {
        format: FormatId::Step,
        params: serde_json::to_value(spec).unwrap_or_default(),
    }
}

fn fixture_bytes(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join("step")
            .join(name),
    )
    .unwrap_or_else(|error| panic!("fixture {name}: {error}"))
}

fn generate_bytes(produced: &Produced) -> Vec<u8> {
    match produced.inner() {
        OpOutput::Generate(bytes) => bytes.clone(),
        other => panic!("expected Generate output, got {other:?}"),
    }
}

/// ACCEPTANCE: the hand-committed minimal .step fixture extracts to the
/// pinned tree (header AP name, entity census, PRODUCT chain skeleton,
/// units, CARTESIAN_POINT bbox; the block remark is skipped).
#[test]
fn hand_committed_fixture_extracts_pinned_tree() {
    let extracted = step::extract(&fixture_bytes("minimal.step"), Default::default())
        .unwrap_or_else(|error| panic!("extract: {error:?}"));
    assert_eq!(extracted.tree["schema"], step::EXTRACT_SCHEMA);
    assert_eq!(extracted.tree["entity_count"], 14);
    let census = &extracted.tree["entity_census"];
    assert_eq!(census["PRODUCT"], 2);
    assert_eq!(census["PRODUCT_DEFINITION"], 2);
    assert_eq!(census["PRODUCT_DEFINITION_FORMATION"], 2);
    assert_eq!(census["CARTESIAN_POINT"], 3);
    assert_eq!(census["MANIFOLD_SOLID_BREP"], 1);
    assert_eq!(census["SI_UNIT"], 2);
    assert_eq!(census["APPLICATION_CONTEXT"], 1);
    assert_eq!(census["PRODUCT_DEFINITION_CONTEXT"], 1);
    assert_eq!(
        extracted.tree["header"]["file_description"],
        "harness minimal step fixture"
    );
    assert_eq!(
        extracted.tree["header"]["file_schema"],
        "AUTOMOTIVE_DESIGN { 1 0 10303 214 3 1 4 }"
    );
    let products = extracted.tree["products"].as_array().expect("products");
    assert_eq!(products.len(), 2, "assembly skeleton carries both products");
    assert_eq!(products[0]["id"], 1);
    assert_eq!(products[0]["name"], "Widget");
    assert_eq!(products[0]["formations"], 1);
    assert_eq!(products[0]["definitions"], 1);
    assert_eq!(products[1]["id"], 2);
    assert_eq!(products[1]["name"], "Gadget");
    let units = extracted.tree["units"].as_array().expect("units");
    assert_eq!(
        units
            .iter()
            .map(|unit| unit.as_str().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["MILLI/METRE", "NANO/SECOND"]
    );
    assert_eq!(
        extracted.tree["bbox"]["min"],
        serde_json::json!([-2.5, 0.0, 0.0])
    );
    assert_eq!(
        extracted.tree["bbox"]["max"],
        serde_json::json!([10.0, 20.0, 1000.0])
    );
}

/// The bare M2 probe stub (header only) extracts honestly: zero data
/// entities, no products, no bbox - never a fabricated structure.
#[test]
fn header_only_probe_stub_extracts_an_empty_skeleton() {
    let stub = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join("step.stp"),
    )
    .expect("committed probe stub");
    let extracted = step::extract(&stub, Default::default()).expect("header-only extract");
    assert_eq!(extracted.tree["entity_count"], 0);
    assert_eq!(extracted.tree["products"].as_array().map(Vec::len), Some(0));
    assert_eq!(extracted.tree["units"].as_array().map(Vec::len), Some(0));
    assert!(extracted.tree["bbox"].is_null(), "no points, no bbox");
    assert!(
        extracted.tree["header"]["file_schema"].is_null(),
        "the stub carries no FILE_SCHEMA: null is the honest value"
    );
}

/// ACCEPTANCE (failure lane): binary garbage and malformed entities are
/// the typed ParseFailure, never a panic.
#[test]
fn malformed_input_is_typed_parse_failure() {
    let cases: Vec<Vec<u8>> = vec![
        b"\x00\x01\x02binary garbage".to_vec(),
        b"ISO-10303-21".to_vec(),
        b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('broken string'),'2;1'".to_vec(),
        b"ISO-10303-21;\n/* unterminated block remark".to_vec(),
        b"ISO-10303-21;\nDATA;\n# = PRODUCT('X','',(),());".to_vec(),
    ];
    for case in cases {
        assert!(
            matches!(
                step::extract(&case, Default::default()),
                Err(MediaError::ParseFailure)
            ),
            "expected typed ParseFailure for {:?}",
            String::from_utf8_lossy(&case)
        );
    }
}

/// ROUND-TRIP (the M20 acceptance gate): spec corpus >= 8 -> generate
/// through the mock freecadcmd -> .step magic -> probe Certain ->
/// topology re-read diff against the INDEPENDENT spec->tree projection.
#[test]
fn spec_corpus_roundtrips_through_the_mock_lane() {
    for (name, spec) in corpus() {
        let temp = tempfile::tempdir().expect("tempdir");
        let builder = step_exec("stepbuild", temp.path());
        let dumper = step_exec("stepdump", temp.path());
        let bytes = generate_bytes(
            &step::generate(&builder, &generation(&spec))
                .unwrap_or_else(|error| panic!("{name}: generate: {error:?}")),
        );
        assert!(
            bytes.starts_with(b"ISO-10303-21;"),
            "{name}: generated artifact carries the STEP magic"
        );
        let text = String::from_utf8_lossy(&bytes).into_owned();
        assert!(
            text.contains("END-ISO-10303-21;"),
            "{name}: generated artifact is terminated"
        );
        let probe = harness_media::probe::probe(&bytes, None);
        assert!(
            matches!(
                probe,
                harness_media::probe::ProbeResult::Identified {
                    format: FormatId::Step,
                    confidence: harness_media::probe::Confidence::Certain,
                    ..
                }
            ),
            "{name}: probe Certain"
        );
        let verdict = step::verify(&dumper, &spec.topology_tree(), &bytes)
            .unwrap_or_else(|error| panic!("{name}: verify: {error:?}"));
        match verdict.inner() {
            OpOutput::Verify(harness_media::diff::RoundTripVerdict::Pass) => {}
            OpOutput::Verify(harness_media::diff::RoundTripVerdict::Fail(diff)) => {
                panic!("{name}: non-empty diff: {diff:?}")
            }
            other => panic!("{name}: expected Verify output, got {other:?}"),
        }
    }
}

/// The pure tokenizer also reads mock-generated bytes: the AP214 schema
/// name and the PRODUCT chain (ordered, named) survive generation.
#[test]
fn pure_extract_reads_generated_bytes() {
    for (name, spec) in corpus() {
        let temp = tempfile::tempdir().expect("tempdir");
        let bytes = generate_bytes(
            &step::generate(&step_exec("stepbuild", temp.path()), &generation(&spec))
                .unwrap_or_else(|error| panic!("{name}: generate: {error:?}")),
        );
        let extracted = step::extract(&bytes, Default::default())
            .unwrap_or_else(|error| panic!("{name}: pure extract: {error:?}"));
        assert!(
            extracted.tree["header"]["file_schema"]
                .as_str()
                .unwrap_or_default()
                .starts_with("AUTOMOTIVE_DESIGN"),
            "{name}: AP214-flavored FILE_SCHEMA"
        );
        let products = extracted.tree["products"].as_array().expect("products");
        assert_eq!(
            products.len(),
            spec.solids.len(),
            "{name}: product per solid"
        );
        for (product, expected) in products.iter().zip(&spec.solids) {
            assert_eq!(product["name"], expected.name, "{name}: ordered names");
        }
    }
}

/// DETERMINISM CONTRACT: repeat generation is byte-identical, and the
/// FILE_NAME clock is pinned to the M1 fixed epoch (2026-01-01T00:00:00Z).
#[test]
fn generation_is_byte_deterministic_with_pinned_epoch() {
    let (_, spec) = corpus().remove(0);
    let mut first = Vec::new();
    for _ in 0..2 {
        let temp = tempfile::tempdir().expect("tempdir");
        let bytes = generate_bytes(
            &step::generate(&step_exec("stepbuild", temp.path()), &generation(&spec))
                .expect("generate"),
        );
        assert!(
            String::from_utf8_lossy(&bytes).contains("'2026-01-01T00:00:00Z'"),
            "FILE_NAME timestamp pinned to the determinism epoch"
        );
        if first.is_empty() {
            first = bytes;
        } else {
            assert_eq!(first, bytes, "byte-identical repeat generation");
        }
    }
}

/// ACCEPTANCE (default-disabled): the shipped registry answers step
/// GENERATE with the typed ToolUnavailable (freecadcmd is opt-in) while
/// extract and probe stay PURE (no tool, no staging).
#[test]
fn default_config_generate_is_typed_tool_unavailable_extract_stays_pure() {
    let registry = harness_media::executor::registry();
    let error = registry
        .dispatch(
            FormatId::Step,
            OpRequest::Generate {
                spec: &generation(&corpus().remove(0).1),
            },
        )
        .expect_err("default: freecadcmd not enabled");
    match error {
        MediaError::ToolUnavailable { tool, install_hint } => {
            assert_eq!(tool, "freecadcmd");
            assert!(!install_hint.is_empty(), "actionable hint");
        }
        other => panic!("expected ToolUnavailable, got {other:?}"),
    }
    let produced = registry
        .dispatch(
            FormatId::Step,
            OpRequest::Extract {
                bytes: &fixture_bytes("minimal.step"),
                budgets: Default::default(),
            },
        )
        .expect("extract never touches the tool lane");
    assert!(matches!(produced.inner(), OpOutput::Extract(_)));
    assert_eq!(
        produced.producer().tier,
        harness_media::executor::AdapterTier::Pure
    );
    let probed = registry
        .dispatch(
            FormatId::Step,
            OpRequest::Probe {
                bytes: &fixture_bytes("minimal.step"),
                name_hint: None,
            },
        )
        .expect("probe never touches the tool lane");
    assert!(matches!(probed.inner(), OpOutput::Probe(_)));
}

/// DISPATCH: generate through the registry carries the subprocess
/// producer disclosure (engine freecadcmd, command line, manifest pin)
/// and the manifest pins the real invocation shapes.
#[test]
fn dispatch_generate_carries_freecadcmd_producer_disclosure() {
    let temp = tempfile::tempdir().expect("tempdir");
    let registry =
        harness_media::executor::registry().with_tools(step_exec("stepbuild", temp.path()));
    let produced = registry
        .dispatch(
            FormatId::Step,
            OpRequest::Generate {
                spec: &generation(&corpus().remove(0).1),
            },
        )
        .expect("dispatch generate through the mock lane");
    let producer = produced.producer();
    assert_eq!(producer.engine, "freecadcmd");
    assert_eq!(
        producer.tier,
        harness_media::executor::AdapterTier::Subprocess
    );
    assert!(
        producer
            .command
            .as_deref()
            .is_some_and(|command| command.contains("--exact"))
    );
    let spec = harness_media::tool::manifests()
        .get("freecadcmd")
        .expect("freecadcmd manifest");
    let build = spec.op_template("step-build").expect("step-build op").args;
    assert_eq!(build, &["{script}", "{output}"]);
    let dump = spec
        .op_template("topology-dump")
        .expect("topology-dump op")
        .args;
    assert_eq!(dump, &["{script}", "{input}"]);
    assert!(!generate_bytes(&produced).is_empty());
}

/// NON-ZERO EXIT: a failing mock freecadcmd surfaces the typed
/// NonZeroExit with the bounded stderr head - no partial artifact.
#[test]
fn mock_failure_is_typed_nonzero_exit_no_partial_artifact() {
    let temp = tempfile::tempdir().expect("tempdir");
    let failing = harness_media::tool::ToolExecutor::new(harness_media::tool::ToolConfig {
        enabled: [("freecadcmd".to_owned(), mocktool_override())].into(),
        install_enabled: false,
        session_temp: temp.path().to_path_buf(),
        env: [(
            "HARNESS_MOCKTOOL_ARGV".to_owned(),
            "exit 4 freecad-boom".to_owned(),
        )]
        .into(),
    });
    let error = step::generate(&failing, &generation(&corpus().remove(0).1))
        .expect_err("failing tool exit is typed");
    match error {
        MediaError::NonZeroExit { code, stderr_head } => {
            assert_eq!(code, 4);
            assert!(
                stderr_head.contains("freecad-boom"),
                "stderr head carries why"
            );
        }
        other => panic!("expected NonZeroExit, got {other:?}"),
    }
}

/// MOCK-DRIFT LAW: the topology census stdout JSON contract is pinned by
/// a versioned schema test (top-level key set, per-solid key set, schema
/// id).
#[test]
fn topology_schema_contract_is_version_pinned() {
    let temp = tempfile::tempdir().expect("tempdir");
    let (_, spec) = corpus().remove(3); // all-primitives
    let bytes = generate_bytes(
        &step::generate(&step_exec("stepbuild", temp.path()), &generation(&spec))
            .expect("generate"),
    );
    let extracted_census = step::read_topology_census(&step_exec("stepdump", temp.path()), &bytes)
        .expect("census re-read");
    let top: Vec<&str> = extracted_census
        .as_object()
        .expect("census root is an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(top, vec!["schema", "solid_count", "solids"]);
    assert_eq!(extracted_census["schema"], step::DUMP_SCHEMA);
    let first = extracted_census["solids"][0]
        .as_object()
        .expect("solid entry is an object");
    let keys: Vec<&str> = first.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        vec![
            "bbox_max", "bbox_min", "edges", "faces", "vertices", "volume"
        ]
    );
}

/// TEXT PROJECTION: the pure extract projection is a bounded structure
/// summary a text-only model can read.
#[test]
fn text_projection_is_a_bounded_structure_summary() {
    let extracted =
        step::extract(&fixture_bytes("minimal.step"), Default::default()).expect("extract");
    for name in ["Widget", "Gadget", "AUTOMOTIVE_DESIGN", "MILLI/METRE"] {
        assert!(
            extracted.text_projection.contains(name),
            "summary names {name}: {}",
            extracted.text_projection
        );
    }
    assert!(extracted.text_projection.len() <= harness_media::extract::TEXT_PROJECTION_MAX_CHARS);
}

/// HERMETIC HARDENING: the mock freecadcmd mode runs the emitted builder
/// script through python validation - a syntactically broken script
/// fails HERMETICALLY (typed NonZeroExit from the mock).
#[test]
fn syntactically_broken_builder_script_fails_hermetically() {
    let temp = tempfile::tempdir().expect("tempdir");
    let dir = temp.path().join("step-build");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("build.py"), "def broken(:\n    pass\n").expect("write script");
    let error = step_exec("stepbuild", temp.path())
        .run(&harness_media::tool::ToolCall::new(
            "freecadcmd",
            "step-build",
        ))
        .expect_err("broken python refuses in the mock");
    match error {
        harness_media::tool::ToolError::NonZeroExit { stderr_head, .. } => {
            assert!(
                stderr_head.to_lowercase().contains("python"),
                "stderr names the python validation: {stderr_head}"
            );
        }
        other => panic!("expected NonZeroExit, got {other:?}"),
    }
}

/// The generator side of the same guarantee: every emitted builder
/// script parses as python.
#[test]
fn generated_builder_scripts_parse_as_python() {
    for (name, spec) in corpus() {
        let source = step::builder_script(&spec);
        harness_test_support::mocktool::validate_python(&source)
            .unwrap_or_else(|error| panic!("{name}: builder script is valid python: {error}"));
    }
    assert!(harness_test_support::mocktool::validate_python("def broken(:").is_err());
}

/// HONESTY: bytes without the mock census (the hand-committed fixture)
/// cannot topology-dump hermetically - typed failure, never a fake
/// census.
#[test]
fn step_bytes_without_mock_census_fail_typed_not_faked() {
    let temp = tempfile::tempdir().expect("tempdir");
    let error = step::read_topology_census(
        &step_exec("stepdump", temp.path()),
        &fixture_bytes("minimal.step"),
    )
    .expect_err("no mock census in the fixture: typed refusal");
    assert!(
        matches!(error, MediaError::NonZeroExit { .. }),
        "expected NonZeroExit, got {error:?}"
    );
}

/// INVALID SPEC: format mismatch, unknown shape, non-finite floats,
/// non-positive dimensions, oversized/duplicate/control-char names, and
/// oversize solid counts are the typed InvalidSpec.
#[test]
fn invalid_specs_are_typed_refusals() {
    let temp = tempfile::tempdir().expect("tempdir");
    let builder = step_exec("stepbuild", temp.path());
    let wrong_format = GenerationSpec {
        format: FormatId::Svg,
        params: serde_json::json!({}),
    };
    assert!(matches!(
        step::generate(&builder, &wrong_format),
        Err(MediaError::InvalidSpec)
    ));
    let bad_shape = GenerationSpec {
        format: FormatId::Step,
        params: serde_json::json!({"solids": [{"name": "X", "shape": {"torus": {"radius": 1.0}}}]}),
    };
    assert!(matches!(
        step::generate(&builder, &bad_shape),
        Err(MediaError::InvalidSpec)
    ));
    let nan_size = StepSpec {
        solids: vec![solid(
            "NaN",
            SolidShape::Box {
                size: [f64::NAN, 1.0, 1.0],
            },
            transform([0.0; 3], [0.0; 3]),
        )],
    };
    assert!(matches!(
        step::generate(&builder, &generation(&nan_size)),
        Err(MediaError::InvalidSpec)
    ));
    let zero_radius = StepSpec {
        solids: vec![solid(
            "Zero",
            SolidShape::Sphere { radius: 0.0 },
            transform([0.0; 3], [0.0; 3]),
        )],
    };
    assert!(matches!(
        step::generate(&builder, &generation(&zero_radius)),
        Err(MediaError::InvalidSpec)
    ));
    let duplicate_names = StepSpec {
        solids: vec![
            solid(
                "Twin",
                SolidShape::Sphere { radius: 1.0 },
                transform([0.0; 3], [0.0; 3]),
            ),
            solid(
                "Twin",
                SolidShape::Sphere { radius: 2.0 },
                transform([0.0; 3], [0.0; 3]),
            ),
        ],
    };
    assert!(matches!(
        step::generate(&builder, &generation(&duplicate_names)),
        Err(MediaError::InvalidSpec)
    ));
    let control_char_name = StepSpec {
        solids: vec![solid(
            "Bad\x01Name",
            SolidShape::Sphere { radius: 1.0 },
            transform([0.0; 3], [0.0; 3]),
        )],
    };
    assert!(matches!(
        step::generate(&builder, &generation(&control_char_name)),
        Err(MediaError::InvalidSpec)
    ));
    let too_many = StepSpec {
        solids: (0..=step::MAX_SOLIDS)
            .map(|index| {
                solid(
                    &format!("S{index}"),
                    SolidShape::Sphere { radius: 1.0 },
                    transform([0.0; 3], [0.0; 3]),
                )
            })
            .collect(),
    };
    assert!(matches!(
        step::generate(&builder, &generation(&too_many)),
        Err(MediaError::InvalidSpec)
    ));
}

/// BUDGETS: an input beyond the caller's size cap trips the typed
/// ExtractTooLarge - never an unbounded parse.
#[test]
fn oversize_input_trips_typed_extract_too_large() {
    let mut bytes = b"ISO-10303-21;\nDATA;\n".to_vec();
    for _ in 0..4096 {
        bytes.extend_from_slice(b"#1 = PRODUCT('X','',(),());\n");
    }
    let budgets = harness_media::extract::ExtractBudgets {
        deadline: std::time::Duration::from_secs(10),
        max_total_bytes: 1024,
    };
    assert!(matches!(
        step::extract(&bytes, budgets),
        Err(MediaError::ExtractTooLarge)
    ));
}

/// REAL-TOOL TIER: one real freecadcmd over the primitive corpus
/// (generate, topology re-read, and diff). Never runs in CI; run
/// at-todo-time or on the M25 cadence with `FREECAD_BIN=<freecadcmd>`
/// and `cargo nextest run -p harness-media --run-ignored only --test
/// step_adapter`.
#[test]
#[ignore = "real-tool tier: needs FreeCAD (1.0.x) via FREECAD_BIN; never CI"]
fn real_freecadcmd_smoke_round_trips_the_primitive_corpus() {
    let bin = std::env::var_os("FREECAD_BIN").unwrap_or_else(|| {
        panic!(
            "set FREECAD_BIN to a freecadcmd executable (FreeCAD 1.0.x; freecad.org builds); \
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
    for (name, spec) in corpus() {
        let temp = tempfile::tempdir().expect("tempdir");
        let tools = harness_media::tool::ToolExecutor::new(harness_media::tool::ToolConfig {
            enabled: [(
                "freecadcmd".to_owned(),
                harness_media::tool::ToolOverride {
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
            &step::generate(&tools, &generation(&spec))
                .unwrap_or_else(|error| panic!("{name}: real generate: {error:?}")),
        );
        assert!(
            bytes.starts_with(b"ISO-10303-21;"),
            "{name}: real STEP magic"
        );
        let verdict = step::verify(&tools, &spec.topology_tree(), &bytes)
            .unwrap_or_else(|error| panic!("{name}: real verify: {error:?}"));
        assert!(
            matches!(
                verdict.inner(),
                OpOutput::Verify(harness_media::diff::RoundTripVerdict::Pass)
            ),
            "{name}: real freecadcmd round-trip passes the topology diff"
        );
    }
}
