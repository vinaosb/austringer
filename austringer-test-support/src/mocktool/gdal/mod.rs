//! The mocktool gdal modes (plan M21 hermetic lane): stand-ins for
//! `gdal_translate` (attach georeferencing / convert to COG) and
//! `gdalinfo -json`. GDAL is not installed on the capture machine
//! (capture.md pending row), so like the M16 blend lane these mocks
//! are PARAMETERIZED structural implementations, not canned goldens:
//! `gdaltranslate` reads the staged baseline raster plus the spec
//! contract and writes a real little-endian TIFF carrying the GeoKey
//! directory, pixel-scale and tiepoint tags; `gdalinfo` parses that
//! structure back and prints the gdalinfo-shaped JSON census. Three
//! independent derivations triangulate the round-trip: this
//! hand-rolled writer/reader, the adapter's tiff-crate extractor, and
//! the adapter's independent spec->tree projection.
//!
//! Mock stand-ins for GDAL internals (documented honesty):
//! - CRS model type: GDAL consults PROJ's database; the mock ships the
//!   documented heuristic (EPSG 4000..=4999 geographic, else
//!   projected) instead of a fake database.
//! - COG profile: GDAL discloses LAYOUT=COG through its structural
//!   metadata ghost area; the mock writes and reads the same-intent
//!   ghost area after the TIFF header.
//! - Overviews: GDAL halves the raster until it fits one 512 block,
//!   smallest overview leading the IFD chain (the COG layout); the
//!   mock applies that documented rule.
//!
//! Contract (cwd = the executor's session temp): `gdaltranslate`
//! reads `gdal-gen/spec.json` + `gdal-gen/in.tif` (uncompressed 1-band
//! u8) and writes `gdal-gen/out.tif`; `gdalinfo` reads
//! `gdal-info/in.tif` and prints JSON. Anything else is a non-zero
//! exit with a stderr reason (the adapter surfaces the typed
//! NonZeroExit).

mod read;
mod write;

use serde_json::{Value, json};

use read::parse_tiff;

const SPEC_FILE: &str = "gdal-gen/spec.json";
const INPUT_FILE: &str = "gdal-gen/in.tif";
const OUTPUT_FILE: &str = "gdal-gen/out.tif";
const INFO_FILE: &str = "gdal-info/in.tif";

/// `gdaltranslate`: spec contract + baseline raster -> georeferenced
/// (optionally COG) GeoTIFF.
pub fn translate_mode() -> i32 {
    match translate() {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("mocktool: gdal translate failed: {error}");
            1
        }
    }
}

fn translate() -> Result<(), String> {
    let spec: Value = serde_json::from_str(
        &std::fs::read_to_string(SPEC_FILE).map_err(|e| format!("read {SPEC_FILE}: {e}"))?,
    )
    .map_err(|e| format!("spec is not json: {e}"))?;
    let width = u32_from(&spec["width"])?;
    let height = u32_from(&spec["height"])?;
    let epsg = u32_from(&spec["epsg"])?;
    let cog = spec["cog"].as_bool().unwrap_or(false);
    let pixels = read_baseline(width, height)?;
    let scale = [
        f64_from(&spec["geotransform"]["pixel_size"][0])?,
        f64_from(&spec["geotransform"]["pixel_size"][1])?,
    ];
    let origin = [
        f64_from(&spec["geotransform"]["origin"][0])?,
        f64_from(&spec["geotransform"]["origin"][1])?,
    ];
    let mut images = Vec::new();
    if cog {
        let mut levels = Vec::new();
        let (mut w, mut h) = (width, height);
        while w.max(h) > write::TILE {
            w = (w / 2).max(1);
            h = (h / 2).max(1);
            levels.push(downsample(w, h, &pixels, width));
        }
        images.extend(levels.into_iter().rev());
    }
    images.push((width, height, pixels));
    let bytes = write::write_tiff(&images, (origin, scale, epsg), cog);
    std::fs::write(OUTPUT_FILE, bytes).map_err(|e| format!("write {OUTPUT_FILE}: {e}"))
}

/// `gdalinfo`: staged GeoTIFF -> gdalinfo-shaped `-json` census.
pub fn info_mode() -> i32 {
    match info() {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("mocktool: gdalinfo failed: {error}");
            1
        }
    }
}

fn info() -> Result<(), String> {
    let bytes = std::fs::read(INFO_FILE).map_err(|e| format!("read {INFO_FILE}: {e}"))?;
    let parsed = parse_tiff(&bytes)?;
    let main = parsed.main_ifd().ok_or("no IFD in the staged raster")?;
    let mut census = json!({
        "size": [main.width, main.height],
        "bands": [{"band": 1, "type": "Byte"}],
    });
    if let Some(geo) = &main.geo {
        census["geoTransform"] = json!([
            geo.origin[0],
            geo.scale[0],
            0.0,
            geo.origin[1],
            0.0,
            -geo.scale[1]
        ]);
    }
    if parsed.cog_layout {
        census["metadata"] = json!({"IMAGE_STRUCTURE": {"LAYOUT": "COG"}});
    }
    println!("{census}");
    Ok(())
}

fn u32_from(value: &Value) -> Result<u32, String> {
    value
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v > 0)
        .ok_or_else(|| format!("bad integer {value}"))
}

fn f64_from(value: &Value) -> Result<f64, String> {
    value
        .as_f64()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("bad float {value}"))
}

/// The staged baseline the adapter wrote (tiff crate, uncompressed
/// strips): decode pixels through the same mini parser gdalinfo uses.
fn read_baseline(width: u32, height: u32) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(INPUT_FILE).map_err(|e| format!("read {INPUT_FILE}: {e}"))?;
    let parsed = parse_tiff(&bytes)?;
    let main = parsed.main_ifd().ok_or("baseline has no IFD")?;
    if main.width != width || main.height != height {
        return Err(format!(
            "spec dims {width}x{height} disagree with the staged raster {}x{}",
            main.width, main.height
        ));
    }
    main.pixel_bytes(&bytes)
}

fn downsample(width: u32, height: u32, pixels: &[u8], full_width: u32) -> (u32, u32, Vec<u8>) {
    let mut out = Vec::with_capacity((width * height) as usize);
    for row in 0..height {
        for column in 0..width {
            let source = 2 * row as usize * full_width as usize + 2 * column as usize;
            out.push(pixels[source]);
        }
    }
    (width, height, out)
}
