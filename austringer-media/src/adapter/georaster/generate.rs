//! The georaster generate lane (plan M21): the typed spec's pixel
//! baseline (plain uncompressed u8 tiff via the tiff crate) plus a
//! staged spec contract feed `gdal_translate`, which attaches the
//! georeferencing (`-a_srs EPSG:...`, `-a_ullr ...`) and produces the
//! COG layout when the spec flags it. The argv is built per call from
//! the validated spec (the M17 pattern - EPSG/coords cannot be a
//! static manifest template); the mock lane reads the staged spec
//! contract instead of the substituted argv. A non-zero exit or
//! timeout yields NO partial artifact - the output is read only after
//! a zero-exit run.

use std::io::Cursor;

use serde_json::json;

use super::TOOL;
use super::spec::GeoRasterSpec;
use crate::error::MediaError;
use crate::executor::{OpOutput, Produced};
use crate::generate::GenerationSpec;
use crate::tool::{ToolCall, ToolExecutor};
use tiff::encoder::colortype::Gray8;

const INPUT_FILE: &str = "gdal-gen/in.tif";
const SPEC_FILE: &str = "gdal-gen/spec.json";
const OUTPUT_FILE: &str = "gdal-gen/out.tif";

/// Generate GeoTIFF/COG bytes from a validated spec through
/// gdal_translate.
///
/// # Errors
/// [`MediaError::InvalidSpec`] when the spec violates the georaster
/// contract; the routed [`ToolError`] failures otherwise. A failing or
/// absent tool never yields bytes.
pub fn generate(tools: &ToolExecutor, spec: &GenerationSpec) -> Result<Produced, MediaError> {
    let raster = GeoRasterSpec::from_generation(spec)?;
    super::unavailable(tools)?;
    let root = &tools.config().session_temp;
    let input = root.join(INPUT_FILE);
    let spec_path = root.join(SPEC_FILE);
    let output = root.join(OUTPUT_FILE);
    let directory = spec_path.parent().unwrap_or(root);
    std::fs::create_dir_all(directory).map_err(spawn_failure)?;
    let mut baseline = Cursor::new(Vec::new());
    let mut encoder =
        tiff::encoder::TiffEncoder::new(&mut baseline).map_err(|_| MediaError::ParseFailure)?;
    encoder
        .write_image::<Gray8>(raster.width, raster.height, &raster.pixels)
        .map_err(|_| MediaError::ParseFailure)?;
    std::fs::write(&input, baseline.into_inner()).map_err(spawn_failure)?;
    std::fs::write(&spec_path, stage_contract(&raster)).map_err(spawn_failure)?;
    let call = ToolCall::new(TOOL, "translate").with_argv(translate_argv(&raster, &input, &output));
    let run = tools.run(&call).map_err(MediaError::from)?;
    let bytes = std::fs::read(&output).map_err(spawn_failure)?;
    Ok(Produced::new(OpOutput::Generate(bytes), run.producer()))
}

/// The staged spec contract: what the real tool learns from the argv,
/// the mock tool reads from disk (georeferencing + layout, never the
/// pixels - those ride the baseline raster).
fn stage_contract(raster: &GeoRasterSpec) -> String {
    let [origin_x, origin_y] = raster.geotransform.origin;
    let [pixel_x, pixel_y] = raster.geotransform.pixel_size;
    json!({
        "cog": raster.cog,
        "epsg": raster.epsg,
        "geotransform": {
            "origin": [origin_x, origin_y],
            "pixel_size": [pixel_x, pixel_y],
        },
        "height": raster.height,
        "width": raster.width,
    })
    .to_string()
}

/// The real gdal_translate invocation (disclosed in the producer
/// command line; exercised by the `#[ignore]` real-tool tier):
/// attach the CRS and the upper-left/lower-right ground extent, pin
/// the 1-band u8 output, and convert to COG when the spec flags it.
fn translate_argv(
    raster: &GeoRasterSpec,
    input: &std::path::Path,
    output: &std::path::Path,
) -> Vec<String> {
    let [origin_x, origin_y] = raster.geotransform.origin;
    let [pixel_x, pixel_y] = raster.geotransform.pixel_size;
    let max_x = origin_x + f64::from(raster.width) * pixel_x;
    let min_y = origin_y - f64::from(raster.height) * pixel_y;
    let mut argv: Vec<String> = [
        "-q",
        "-a_srs",
        &format!("EPSG:{}", raster.epsg),
        "-a_ullr",
        &format!("{origin_x}"),
        &format!("{origin_y}"),
        &format!("{max_x}"),
        &format!("{min_y}"),
        "-ot",
        "Byte",
        "-b",
        "1",
    ]
    .iter()
    .map(|word| (*word).to_owned())
    .collect();
    if raster.cog {
        argv.push("-of".to_owned());
        argv.push("COG".to_owned());
    }
    argv.push(input.display().to_string());
    argv.push(output.display().to_string());
    argv
}

fn spawn_failure(detail: std::io::Error) -> MediaError {
    MediaError::ToolUnavailable {
        tool: TOOL.to_owned(),
        install_hint: format!("session staging failed: {detail}"),
    }
}
