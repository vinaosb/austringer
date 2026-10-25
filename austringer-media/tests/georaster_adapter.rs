//! M21 GeoTIFF/COG adapter tests: pure extract over the tiff crate
//! (committed fixture pins EPSG + bounds; plain tiff gets the honest no-
//! geo-keys note), the hermetic mock-GDAL lane (gdaltranslate ->
//! georeferenced/COG output -> gdalinfo assert + re-extract diff), the
//! default-disabled typed refusals, dispatch producer disclosure, the
//! spec validation refusals, and the `#[ignore]` real-GDAL tier.
//! Hermetic: no network, no real gdal, mocktool children only.

use std::collections::BTreeMap;
use std::path::Path;

use austringer_media::adapter::georaster::{self, GeoRasterSpec, GeoTransform};
use austringer_media::error::MediaError;
use austringer_media::executor::{OpOutput, OpRequest, Produced};
use austringer_media::format::FormatId;
use austringer_media::generate::GenerationSpec;

/// T40-style re-exec entry: when austringer_MOCKTOOL_ARGV is set, THIS test
/// becomes a mocktool child; the executor spawns this binary as the
/// substituted gdal program.
#[test]
fn mocktool_child_entry() {
    if std::env::var("austringer_MOCKTOOL_ARGV").is_err() {
        return; // normal suite run: nothing to do
    }
    std::process::exit(austringer_test_support::mocktool::run_from_env());
}

fn mocktool_override() -> austringer_media::tool::ToolOverride {
    austringer_media::tool::ToolOverride {
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

/// The mock gdal executor for one mode (`gdaltranslate` / `gdalinfo`).
fn gdal_exec(mode: &str, temp: &Path) -> austringer_media::tool::ToolExecutor {
    austringer_media::tool::ToolExecutor::new(austringer_media::tool::ToolConfig {
        enabled: [("gdal".to_owned(), mocktool_override())].into(),
        install_enabled: false,
        session_temp: temp.to_path_buf(),
        env: [("austringer_MOCKTOOL_ARGV".to_owned(), mode.to_owned())].into(),
    })
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join(name),
    )
    .unwrap_or_else(|error| panic!("committed fixture {name}: {error}"))
}

fn spec_4x4() -> GeoRasterSpec {
    GeoRasterSpec {
        width: 4,
        height: 4,
        pixels: (0u16..16).map(|value| value as u8).collect(),
        geotransform: GeoTransform {
            origin: [10.0, 50.0],
            pixel_size: [0.5, 0.25],
        },
        epsg: 4326,
        cog: false,
    }
}

fn spec_cog_1024(epsg: u32) -> GeoRasterSpec {
    GeoRasterSpec {
        width: 1024,
        height: 1024,
        pixels: vec![7; 1024 * 1024],
        geotransform: GeoTransform {
            origin: [600_000.0, 6_000_000.0],
            pixel_size: [30.0, 30.0],
        },
        epsg,
        cog: true,
    }
}

fn generation(spec: &GeoRasterSpec) -> GenerationSpec {
    GenerationSpec {
        format: FormatId::Geotiff,
        params: serde_json::to_value(spec).unwrap_or_default(),
    }
}

fn generate_bytes(produced: &Produced) -> Vec<u8> {
    match produced.inner() {
        OpOutput::Generate(bytes) => bytes.clone(),
        other => panic!("expected Generate output, got {other:?}"),
    }
}

/// ACCEPTANCE (plan M21): the committed tiny GeoTIFF fixture extracts
/// its EPSG + bounds pinned (4x4, EPSG:4326, tiepoint 10/50, pixel
/// 0.5/0.25 -> bounds [10, 49, 12, 50] in the FILE's CRS).
#[test]
fn fixture_extracts_epsg_and_bounds_pinned() {
    let extracted = georaster::extract(&fixture("geotiff-geo.tif"), Default::default())
        .unwrap_or_else(|error| panic!("extract: {error:?}"));
    let tree = &extracted.tree;
    assert_eq!(tree["width"], 4);
    assert_eq!(tree["height"], 4);
    assert_eq!(tree["bands"], 1);
    assert_eq!(tree["dtype"], "u8");
    assert_eq!(tree["geo"]["epsg"], 4326);
    assert_eq!(tree["geo"]["model_type"], "geographic");
    assert_eq!(tree["geo"]["crs_source"], "geokey");
    assert_eq!(tree["geo"]["bounds"]["minx"], 10.0);
    assert_eq!(tree["geo"]["bounds"]["miny"], 49.0);
    assert_eq!(tree["geo"]["bounds"]["maxx"], 12.0);
    assert_eq!(tree["geo"]["bounds"]["maxy"], 50.0);
    assert_eq!(tree["layout"]["tiled"], false);
    assert_eq!(tree["layout"]["overview_count"], 0);
    assert_eq!(
        tree["head_sample"][0],
        serde_json::json!([0, 1, 2, 3]),
        "bounded head sample carries the raster's first pixels"
    );
    assert!(
        extracted.text_projection.contains("EPSG:4326"),
        "projection names the CRS: {}",
        extracted.text_projection
    );
    assert!(
        extracted.text_projection.len() <= austringer_media::extract::TEXT_PROJECTION_MAX_CHARS
    );
}

/// ACCEPTANCE (plan M21): a plain TIFF without geo keys is NOT
/// mislabeled GeoTIFF - the honest typed note names what is missing.
#[test]
fn plain_tiff_reports_the_no_geo_keys_note() {
    let plain = plain_tiff_bytes(
        4,
        4,
        &[
            10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120, 130, 140, 150, 160,
        ],
    );
    let extracted = georaster::extract(&plain, Default::default())
        .unwrap_or_else(|error| panic!("plain tiff extracts structurally: {error:?}"));
    assert!(extracted.tree["geo"].is_null(), "no geo block");
    let note = extracted.tree["note"].as_str().unwrap_or_default();
    assert!(
        note.to_lowercase().contains("no geokey directory"),
        "note names the missing georeferencing: {note}"
    );
    assert_eq!(extracted.tree["width"], 4);
    assert_eq!(extracted.tree["dtype"], "u8");
}

/// A plain in-test tiff via the same codec the adapter writes as its
/// gdal_translate baseline (gray u8, uncompressed).
fn plain_tiff_bytes(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
    let mut buffer = std::io::Cursor::new(Vec::new());
    let mut encoder = tiff::encoder::TiffEncoder::new(&mut buffer)
        .unwrap_or_else(|error| panic!("encoder: {error:?}"));
    encoder
        .write_image::<tiff::encoder::colortype::Gray8>(width, height, pixels)
        .unwrap_or_else(|error| panic!("write: {error:?}"));
    buffer.into_inner()
}

/// ROUND-TRIP (the M21 acceptance gate): spec corpus -> mock
/// gdal_translate -> GeoTIFF/COG bytes -> probe Certain -> verify
/// (mock gdalinfo assert + re-extract structural diff vs the
/// INDEPENDENT spec->tree projection) -> Pass.
#[test]
fn spec_corpus_roundtrips_through_the_mock_lane() {
    let corpus = vec![
        ("plain-4x4-wgs84", spec_4x4()),
        ("cog-1024-utm33", spec_cog_1024(32633)),
        ("cog-1024-webmercator", spec_cog_1024(3857)),
        (
            "plain-3x5-negative-origin",
            GeoRasterSpec {
                width: 3,
                height: 5,
                pixels: vec![200; 15],
                geotransform: GeoTransform {
                    origin: [-17.25, -3.5],
                    pixel_size: [0.75, 0.5],
                },
                epsg: 4326,
                cog: false,
            },
        ),
        (
            "small-cog-no-overviews",
            GeoRasterSpec {
                width: 8,
                height: 8,
                pixels: vec![9; 64],
                geotransform: GeoTransform {
                    origin: [0.0, 1.0],
                    pixel_size: [1.0, 1.0],
                },
                epsg: 4326,
                cog: true,
            },
        ),
    ];
    for (name, spec) in corpus {
        let temp = tempfile::tempdir().expect("tempdir");
        let translator = gdal_exec("gdaltranslate", temp.path());
        let informer = gdal_exec("gdalinfo", temp.path());
        let produced = georaster::generate(&translator, &generation(&spec))
            .unwrap_or_else(|error| panic!("{name}: generate: {error:?}"));
        let bytes = generate_bytes(&produced);
        let probe = austringer_media::probe::probe(&bytes, None);
        assert!(
            matches!(
                probe,
                austringer_media::probe::ProbeResult::Identified {
                    format: FormatId::Geotiff,
                    confidence: austringer_media::probe::Confidence::Certain,
                    ..
                }
            ),
            "{name}: probe Certain"
        );
        let verdict = georaster::verify(&informer, &spec.canonical_tree(), &bytes)
            .unwrap_or_else(|error| panic!("{name}: verify: {error:?}"));
        match verdict.inner() {
            OpOutput::Verify(austringer_media::diff::RoundTripVerdict::Pass) => {}
            OpOutput::Verify(austringer_media::diff::RoundTripVerdict::Fail(diff)) => {
                panic!("{name}: non-empty diff: {diff:?}")
            }
            other => panic!("{name}: expected Verify output, got {other:?}"),
        }
        // The COG signature is honestly reported by the pure extractor:
        // tiled + overviews-leading when overviews exist; a small COG
        // (no overviews needed at its size) reports the honest layout.
        let extracted = georaster::extract(&bytes, Default::default())
            .unwrap_or_else(|error| panic!("{name}: re-extract: {error:?}"));
        if spec.cog {
            assert_eq!(extracted.tree["layout"]["tiled"], true, "{name}: tiled");
            if spec.width > 512 {
                assert_eq!(
                    extracted.tree["layout"]["ifd_order"], "overviews-leading",
                    "{name}"
                );
                assert!(
                    extracted.tree["layout"]["overview_count"]
                        .as_u64()
                        .unwrap_or(0)
                        >= 1,
                    "{name}: overview built"
                );
            }
        }
    }
}

/// DEFAULT-DISABLED (plan M21): generation without an opted-in gdal is
/// the typed ToolUnavailable with an actionable install hint; probe and
/// extract stay PURE (no tool needed).
#[test]
fn default_config_generate_refuses_extract_stays_pure() {
    let registry = austringer_media::executor::registry();
    let error = registry
        .dispatch(
            FormatId::Geotiff,
            OpRequest::Generate {
                spec: &generation(&spec_4x4()),
            },
        )
        .expect_err("default: gdal not enabled");
    match error {
        MediaError::ToolUnavailable { tool, install_hint } => {
            assert_eq!(tool, "gdal");
            assert!(!install_hint.is_empty(), "actionable hint");
        }
        other => panic!("expected ToolUnavailable, got {other:?}"),
    }
    let fixture = fixture("geotiff-geo.tif");
    let probed = registry
        .dispatch(
            FormatId::Geotiff,
            OpRequest::Probe {
                bytes: &fixture,
                name_hint: None,
            },
        )
        .expect("probe never needs the tool");
    assert!(matches!(probed.inner(), OpOutput::Probe(_)));
    let extracted = registry
        .dispatch(
            FormatId::Geotiff,
            OpRequest::Extract {
                bytes: &fixture,
                budgets: Default::default(),
            },
        )
        .expect("extract is pure (tiff crate)");
    assert!(matches!(extracted.inner(), OpOutput::Extract(_)));
}

/// DISPATCH: generate through the registry carries the subprocess
/// producer disclosure (engine gdal, tier Subprocess, command line),
/// and the manifest declares the fixed gdalinfo op shape.
#[test]
fn dispatch_generate_carries_gdal_producer_disclosure() {
    let temp = tempfile::tempdir().expect("tempdir");
    let registry =
        austringer_media::executor::registry().with_tools(gdal_exec("gdaltranslate", temp.path()));
    let produced = registry
        .dispatch(
            FormatId::Geotiff,
            OpRequest::Generate {
                spec: &generation(&spec_4x4()),
            },
        )
        .expect("dispatch generate through the mock lane");
    let producer = produced.producer();
    assert_eq!(producer.engine, "gdal");
    assert_eq!(
        producer.tier,
        austringer_media::executor::AdapterTier::Subprocess
    );
    assert!(producer.command.is_some(), "command line disclosed");
    let spec = austringer_media::tool::manifests()
        .get("gdal")
        .expect("gdal manifest");
    let info = spec.op_template("info").expect("info op");
    assert_eq!(info.program, Some("gdalinfo"));
    assert!(info.args.contains(&"-json"));
    assert!(!generate_bytes(&produced).is_empty());
}

/// INVALID SPEC: format mismatch, pixel-length mismatch, non-finite
/// geotransform, zero pixel size, out-of-range EPSG, oversized dims -
/// all the typed InvalidSpec, before any staging or spawn.
#[test]
fn invalid_specs_are_typed_refusals() {
    let tools = gdal_exec("gdaltranslate", Path::new("."));
    let wrong_format = GenerationSpec {
        format: FormatId::Svg,
        params: serde_json::json!({}),
    };
    assert!(matches!(
        georaster::generate(&tools, &wrong_format),
        Err(MediaError::InvalidSpec)
    ));
    let sixteen = serde_json::Value::Array(vec![serde_json::Value::from(0); 16]);
    let cases = vec![
        serde_json::json!({"width": 4, "height": 4, "pixels": [0, 1, 2], "geotransform": {"origin": [10.0, 50.0], "pixel_size": [0.5, 0.25]}, "epsg": 4326}),
        serde_json::json!({"width": 4, "height": 4, "pixels": sixteen, "geotransform": {"origin": [10.0, serde_json::json!(f64::NAN)], "pixel_size": [0.5, 0.25]}, "epsg": 4326}),
        serde_json::json!({"width": 4, "height": 4, "pixels": sixteen, "geotransform": {"origin": [10.0, 50.0], "pixel_size": [0.0, 0.25]}, "epsg": 4326}),
        serde_json::json!({"width": 4, "height": 4, "pixels": sixteen, "geotransform": {"origin": [10.0, 50.0], "pixel_size": [0.5, 0.25]}, "epsg": 0}),
        serde_json::json!({"width": 4, "height": 4, "pixels": sixteen, "geotransform": {"origin": [10.0, 50.0], "pixel_size": [0.5, 0.25]}, "epsg": 32767}),
        serde_json::json!({"width": 5000, "height": 4, "pixels": serde_json::Value::Array(vec![serde_json::Value::from(0); 20000]), "geotransform": {"origin": [10.0, 50.0], "pixel_size": [0.5, 0.25]}, "epsg": 4326}),
    ];
    for params in cases {
        let spec = GenerationSpec {
            format: FormatId::Geotiff,
            params,
        };
        assert!(
            matches!(
                georaster::generate(&tools, &spec),
                Err(MediaError::InvalidSpec)
            ),
            "typed InvalidSpec for {spec:?}"
        );
    }
}

/// NON-ZERO EXIT: a failing mock gdal surfaces the typed NonZeroExit
/// with the bounded stderr head - and no partial artifact escapes.
#[test]
fn mock_failure_is_typed_nonzero_exit() {
    let temp = tempfile::tempdir().expect("tempdir");
    let failing = austringer_media::tool::ToolExecutor::new(austringer_media::tool::ToolConfig {
        enabled: [("gdal".to_owned(), mocktool_override())].into(),
        install_enabled: false,
        session_temp: temp.path().to_path_buf(),
        env: [(
            "austringer_MOCKTOOL_ARGV".to_owned(),
            "exit 3 gdal-boom".to_owned(),
        )]
        .into(),
    });
    let error = georaster::generate(&failing, &generation(&spec_4x4()))
        .expect_err("failing tool exit is typed");
    match error {
        MediaError::NonZeroExit { code, stderr_head } => {
            assert_eq!(code, 3);
            assert!(stderr_head.contains("gdal-boom"), "stderr head carries why");
        }
        other => panic!("expected NonZeroExit, got {other:?}"),
    }
}

/// FAILURE LANE (plan QA): garbage bytes are the typed ParseFailure,
/// never a panic; a byte-cap trip is the typed ExtractTooLarge.
#[test]
fn garbage_bytes_and_budget_trips_are_typed() {
    assert!(matches!(
        georaster::extract(b"not a tiff at all", Default::default()),
        Err(MediaError::ParseFailure)
    ));
    let fixture = fixture("geotiff-geo.tif");
    let tight = austringer_media::extract::ExtractBudgets {
        deadline: Default::default(),
        max_total_bytes: 8,
    };
    assert!(matches!(
        georaster::extract(&fixture, tight),
        Err(MediaError::ExtractTooLarge)
    ));
}

/// REAL-TOOL TIER: one real gdal_translate + gdalinfo over a spec (the
/// plan's at-todo-time evidence run). Never runs in CI; run with
/// GDAL_TRANSLATE_BIN=<gdal_translate> GDALINFO_BIN=<gdalinfo> and
/// `cargo nextest run -p austringer-media --run-ignored only --test
/// georaster_adapter`.
#[test]
#[ignore = "real-tool tier: needs gdal_translate + gdalinfo via GDAL_TRANSLATE_BIN/GDALINFO_BIN; never CI"]
fn real_gdal_translate_and_info_round_trip() {
    let translate_bin = std::env::var_os("GDAL_TRANSLATE_BIN")
        .unwrap_or_else(|| panic!("set GDAL_TRANSLATE_BIN to a gdal_translate executable"));
    let info_bin = std::env::var_os("GDALINFO_BIN")
        .unwrap_or_else(|| panic!("set GDALINFO_BIN to a gdalinfo executable"));
    let spec = spec_4x4();
    let temp = tempfile::tempdir().expect("tempdir");
    let translator =
        austringer_media::tool::ToolExecutor::new(austringer_media::tool::ToolConfig {
            enabled: [(
                "gdal".to_owned(),
                austringer_media::tool::ToolOverride {
                    program: Some(translate_bin.to_string_lossy().into_owned()),
                    args: Vec::new(),
                    timeout_ms: None,
                },
            )]
            .into(),
            install_enabled: false,
            session_temp: temp.path().to_path_buf(),
            env: minimal_child_env(),
        });
    let informer = austringer_media::tool::ToolExecutor::new(austringer_media::tool::ToolConfig {
        enabled: [(
            "gdal".to_owned(),
            austringer_media::tool::ToolOverride {
                program: Some(info_bin.to_string_lossy().into_owned()),
                args: Vec::new(),
                timeout_ms: None,
            },
        )]
        .into(),
        install_enabled: false,
        session_temp: temp.path().to_path_buf(),
        env: minimal_child_env(),
    });
    let bytes = generate_bytes(
        &georaster::generate(&translator, &generation(&spec))
            .unwrap_or_else(|error| panic!("real translate: {error:?}")),
    );
    let verdict = georaster::verify(&informer, &spec.canonical_tree(), &bytes)
        .unwrap_or_else(|error| panic!("real verify (gdalinfo + diff): {error:?}"));
    assert!(
        matches!(
            verdict.inner(),
            OpOutput::Verify(austringer_media::diff::RoundTripVerdict::Pass)
        ),
        "real gdal round-trip passes the gdalinfo assert + structural diff"
    );
}

fn minimal_child_env() -> BTreeMap<String, String> {
    [
        "PATH",
        "SYSTEMROOT",
        "SYSTEMDRIVE",
        "TEMP",
        "TMP",
        "PROJ_LIB",
        "GDAL_DATA",
    ]
    .iter()
    .filter_map(|key| {
        std::env::var(key)
            .ok()
            .map(|value| ((*key).to_owned(), value))
    })
    .collect()
}
