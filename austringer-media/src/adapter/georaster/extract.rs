//! The georaster extract lane (plan M21): pure tiff-crate IFD walk +
//! GeoKey directory interpretation (OGC 19-075). No tool, no decode of
//! full rasters (only the bounded head sample), no reprojection -
//! bounds are reported in the file's CRS with the EPSG disclosed.

use std::io::Cursor;
use std::time::Instant;

use serde_json::{Value, json};
use tiff::decoder::Decoder;
use tiff::tags::Tag;

use crate::error::MediaError;
use crate::extract::{ExtractBudgets, Extracted, TEXT_PROJECTION_MAX_CHARS};

// TIFF/GeoTIFF tags this walk speaks (TIFF 6.0 + OGC 19-075).
const TAG_TILE_WIDTH: u16 = 322;
const TAG_PIXEL_SCALE: u16 = 33550;
const TAG_TIEPOINT: u16 = 33922;
const TAG_GEOKEYS: u16 = 34735;
const TAG_GEO_ASCII: u16 = 34737;
// GeoKey ids.
const KEY_MODEL_TYPE: u16 = 1024;
const KEY_GEOGRAPHIC_TYPE: u16 = 2048;
const KEY_CITATION: u16 = 2049;
const KEY_PROJECTED_CS: u16 = 3072;
/// OGC 19-075 sentinel for "undefined/user-defined" key values.
const KEY_UNDEFINED: u16 = 32767;

/// IFD-chain cap: pathological overview ladders fail typed, never hang.
const MAX_IFDS: usize = 64;

/// Extract the GeoTIFF structural census.
///
/// # Errors
/// [`MediaError::ParseFailure`] when the bytes are not a decodable
/// classic tiff; [`MediaError::ExtractTooLarge`] when the caller
/// budgets or the IFD cap trip. A raster whose pixels cannot be
/// sampled (other dtype, unsupported compression) still extracts its
/// structure with the head sample honestly absent.
pub fn extract(bytes: &[u8], budgets: ExtractBudgets) -> Result<Extracted, MediaError> {
    let started = Instant::now();
    if bytes.len() as u64 > budgets.max_total_bytes {
        return Err(MediaError::ExtractTooLarge);
    }
    let mut decoder = Decoder::new(Cursor::new(bytes)).map_err(|_| MediaError::ParseFailure)?;
    let mut dimensions: Vec<(u32, u32)> = Vec::new();
    let mut main_tiled = false;
    let mut main_dtype = "other";
    let mut main_bands = 0_u64;
    let mut geo_tags: Option<GeoTags> = None;
    let mut head = Value::Null;
    loop {
        let (width, height) = decoder.dimensions().map_err(|_| MediaError::ParseFailure)?;
        let is_main = dimensions.iter().all(|&(w, h)| w * h <= width * height);
        dimensions.push((width, height));
        if is_main {
            main_tiled = decoder.get_tag_u32(Tag::Unknown(TAG_TILE_WIDTH)).is_ok();
            geo_tags = GeoTags::read(&mut decoder);
            (main_dtype, main_bands) = dtype_bands(&mut decoder);
            head = head_sample(&mut decoder, width);
        }
        if !decoder.more_images() {
            break;
        }
        if dimensions.len() >= MAX_IFDS {
            return Err(MediaError::ExtractTooLarge);
        }
        decoder.next_image().map_err(|_| MediaError::ParseFailure)?;
        if started.elapsed() > budgets.deadline {
            return Err(MediaError::ExtractTooLarge);
        }
    }
    let main = main_index(&dimensions);
    let (width, height) = dimensions[main];
    let mut tree = json!({
        "bands": main_bands,
        "dtype": main_dtype,
        "head_sample": head,
        "height": height,
        "layout": layout(&dimensions, main_tiled),
        "width": width,
    });
    match geo_tags.and_then(|tags| tags.interpret(width, height)) {
        Some(geo) => {
            tree["geo"] = geo;
        }
        None => {
            tree["geo"] = Value::Null;
            tree["note"] =
                json!("no GeoKey directory: plain TIFF, not georeferenced (structure only)");
        }
    }
    let extracted = Extracted {
        text_projection: projection(&tree),
        thumbnails: Vec::new(),
        tree,
    };
    if extracted.text_projection.len() as u64 > budgets.max_total_bytes {
        return Err(MediaError::ExtractTooLarge);
    }
    Ok(extracted)
}

fn main_index(dimensions: &[(u32, u32)]) -> usize {
    dimensions
        .iter()
        .enumerate()
        .max_by_key(|(index, pair)| (pair.0 * pair.1, *index))
        .map_or(0, |(index, _)| index)
}

fn layout(dimensions: &[(u32, u32)], tiled: bool) -> Value {
    let first = dimensions.first().map_or(&(0, 0), |pair| pair);
    let last = dimensions.last().map_or(&(0, 0), |pair| pair);
    let order = if dimensions.len() <= 1 || first == last {
        "single"
    } else if first.0 * first.1 < last.0 * last.1 {
        "overviews-leading"
    } else {
        "main-first"
    };
    json!({
        "ifd_order": order,
        "overview_count": dimensions.len() - 1,
        "tiled": tiled,
    })
}

fn dtype_bands(decoder: &mut Decoder<Cursor<&[u8]>>) -> (&'static str, u64) {
    match decoder.colortype() {
        Ok(tiff::ColorType::Gray(bits)) => (dtype_name(u32::from(bits)), 1),
        Ok(tiff::ColorType::GrayA(_)) => ("gray_a", 2),
        Ok(tiff::ColorType::RGB(_)) => ("rgb8", 3),
        Ok(tiff::ColorType::RGBA(_)) => ("rgba8", 4),
        Ok(tiff::ColorType::Palette(_)) => ("palette", 1),
        _ => ("other", 0),
    }
}

fn dtype_name(bits: u32) -> &'static str {
    match bits {
        8 => "u8",
        16 => "u16",
        32 => "u32",
        64 => "u64",
        _ => "other",
    }
}

/// The bounded head sample: the first <=4 rows x <=4 columns of the
/// main IFD, u8 rasters only - anything else reports honestly absent.
fn head_sample(decoder: &mut Decoder<Cursor<&[u8]>>, width: u32) -> Value {
    if !matches!(decoder.colortype(), Ok(tiff::ColorType::Gray(8))) {
        return Value::Null;
    }
    match decoder.read_image() {
        Ok(tiff::decoder::DecodingResult::U8(pixels)) => {
            let stride = width as usize;
            let columns = stride.min(4);
            (0..4)
                .take_while(|row| row * stride < pixels.len())
                .map(|row| json!(pixels[row * stride..row * stride + columns]))
                .collect::<Vec<_>>()
                .into()
        }
        _ => Value::Null,
    }
}

/// The GeoKey-adjacent tags read off the main IFD.
struct GeoTags {
    scale: Option<Vec<f64>>,
    tiepoint: Option<Vec<f64>>,
    keys: Option<Vec<u16>>,
    ascii: Option<String>,
}

impl GeoTags {
    fn read(decoder: &mut Decoder<Cursor<&[u8]>>) -> Option<Self> {
        let scale = decoder.get_tag_f64_vec(Tag::Unknown(TAG_PIXEL_SCALE)).ok();
        let tiepoint = decoder.get_tag_f64_vec(Tag::Unknown(TAG_TIEPOINT)).ok();
        let keys = decoder.get_tag_u16_vec(Tag::Unknown(TAG_GEOKEYS)).ok();
        let ascii = decoder
            .get_tag_ascii_string(Tag::Unknown(TAG_GEO_ASCII))
            .ok();
        (scale.is_some() || tiepoint.is_some() || keys.is_some()).then_some(Self {
            scale,
            tiepoint,
            keys,
            ascii,
        })
    }

    /// Interpret into the canonical geo block; `None` when no geo
    /// material exists at all (the plain-tiff note lane).
    fn interpret(mut self, width: u32, height: u32) -> Option<Value> {
        let keys = self.keys.take()?;
        let entries = keys.len().checked_sub(4)? / 4;
        let key = |id: u16| -> Option<u16> {
            (0..entries).find_map(|index| {
                let base = 4 + index * 4;
                (keys[base] == id && keys[base + 1] == 0 && keys[base + 2] == 1)
                    .then_some(keys[base + 3])
            })
        };
        let model = key(KEY_MODEL_TYPE).map_or("unknown", |value| match value {
            1 => "projected",
            2 => "geographic",
            3 => "geocentric",
            _ => "unknown",
        });
        let (epsg, crs_source) = match key(KEY_PROJECTED_CS).filter(|value| *value != KEY_UNDEFINED)
        {
            Some(code) => (Some(u32::from(code)), "geokey"),
            None => match key(KEY_GEOGRAPHIC_TYPE).filter(|value| *value != KEY_UNDEFINED) {
                Some(code) => (Some(u32::from(code)), "geokey"),
                None => (
                    None,
                    if key(KEY_CITATION).is_some() {
                        "citation"
                    } else {
                        "unknown"
                    },
                ),
            },
        };
        let citation = key(KEY_CITATION).and_then(|count| {
            let ascii = self.ascii.take()?;
            ascii
                .get(0..usize::from(count).min(ascii.len()))
                .map(|slice| slice.trim_end_matches('|').to_owned())
        });
        let (pixel_size, bounds) = match (self.scale.as_deref(), self.tiepoint.as_deref()) {
            (Some(scale), Some(tie)) if scale.len() >= 2 && tie.len() >= 5 => {
                let (psx, psy) = (scale[0], scale[1]);
                let (ox, oy) = (tie[3], tie[4]);
                let xs = [ox, ox + psx * f64::from(width)];
                let ys = [oy, oy - psy * f64::from(height)];
                (
                    json!([psx, psy]),
                    json!({
                        "maxx": xs[0].max(xs[1]),
                        "maxy": ys[0].max(ys[1]),
                        "minx": xs[0].min(xs[1]),
                        "miny": ys[0].min(ys[1]),
                    }),
                )
            }
            _ => (Value::Null, Value::Null),
        };
        Some(json!({
            "bounds": bounds,
            "citation": citation,
            "crs_source": crs_source,
            "epsg": epsg,
            "model_type": model,
            "pixel_size": pixel_size,
            "tie_point": self.tiepoint.map_or(Value::Null, |values| json!(values)),
        }))
    }
}

fn projection(tree: &Value) -> String {
    let mut text = if tree["geo"].is_null() {
        format!(
            "tiff: {}x{} {}-band {}, no GeoKey directory (plain TIFF: not georeferenced)",
            tree["width"], tree["height"], tree["bands"], tree["dtype"]
        )
    } else {
        format!(
            "geotiff: {}x{} {}-band {}, EPSG:{} ({}), bounds [{}, {}, {}, {}] in the file's CRS (EPSG:{}; not reprojected), {}, {} overview(s)",
            tree["width"],
            tree["height"],
            tree["bands"],
            tree["dtype"],
            tree["geo"]["epsg"],
            tree["geo"]["model_type"],
            tree["geo"]["bounds"]["minx"],
            tree["geo"]["bounds"]["miny"],
            tree["geo"]["bounds"]["maxx"],
            tree["geo"]["bounds"]["maxy"],
            tree["geo"]["epsg"],
            if tree["layout"]["tiled"].as_bool().unwrap_or(false) {
                "tiled"
            } else {
                "stripped"
            },
            tree["layout"]["overview_count"],
        )
    };
    text.truncate(TEXT_PROJECTION_MAX_CHARS);
    text
}
