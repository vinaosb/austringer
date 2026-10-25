//! The georaster generation-spec schema (plan M21): the typed subset a
//! model may drive - a 1-band uint8 raster, an affine geotransform
//! (upper-left origin + positive pixel size, north-up), an EPSG-coded
//! CRS, and the COG flag. Parsed strictly from
//! [`GenerationSpec::params`](crate::generate::GenerationSpec);
//! violations are the typed `InvalidSpec`, never a guessed default.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::MediaError;
use crate::format::FormatId;
use crate::generate::GenerationSpec;

/// Per-axis dimension bound (bounded writer output and bounded tool
/// staging).
pub const MAX_DIM: u32 = 4096;
/// Total pixel bound (16 MiB of u8 - the staging ceiling).
pub const MAX_PIXELS: u64 = 16 * 1024 * 1024;
/// GeoKey storage is 16-bit: 0 is invalid and 32767 is the
/// user-defined sentinel (OGC 19-075 key tables).
const EPSG_USER_DEFINED: u32 = 32767;
/// The overview block size the COG layout halves down to (GDAL's
/// documented rule the mock lane and the canonical projection share).
const COG_BLOCK: u32 = 512;

/// Affine geotransform: upper-left origin (map x/y of the corner of
/// pixel (0,0), pixel-is-area) plus positive per-pixel size.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoTransform {
    /// Upper-left corner ground coordinates.
    pub origin: [f64; 2],
    /// Pixel size (x per column, y per row; both positive, north-up).
    pub pixel_size: [f64; 2],
}

/// The georaster generation spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeoRasterSpec {
    /// Raster width in pixels.
    pub width: u32,
    /// Raster height in pixels.
    pub height: u32,
    /// Row-major 1-band uint8 pixels (`width * height` exactly).
    pub pixels: Vec<u8>,
    /// The affine geotransform.
    pub geotransform: GeoTransform,
    /// EPSG code of the file's CRS (never reprojected).
    pub epsg: u32,
    /// Write the COG layout (tiled + overviews) instead of a plain
    /// stripped GeoTIFF.
    pub cog: bool,
}

impl GeoRasterSpec {
    /// Parse and validate a generation spec addressed to geotiff.
    ///
    /// # Errors
    /// [`MediaError::InvalidSpec`] on format mismatch, malformed
    /// params, non-finite numbers, non-positive pixel size, pixel
    /// count disagreement, dimension/total caps, or an EPSG outside
    /// the keyable GeoKey range.
    pub fn from_generation(spec: &GenerationSpec) -> Result<Self, MediaError> {
        if spec.format != FormatId::Geotiff {
            return Err(MediaError::InvalidSpec);
        }
        let parsed: Self =
            serde_json::from_value(spec.params.clone()).map_err(|_| MediaError::InvalidSpec)?;
        parsed.validate()?;
        Ok(parsed)
    }

    fn validate(&self) -> Result<(), MediaError> {
        let invalid = || MediaError::InvalidSpec;
        if self.width == 0
            || self.height == 0
            || self.width > MAX_DIM
            || self.height > MAX_DIM
            || u64::from(self.width) * u64::from(self.height) > MAX_PIXELS
            || self.pixels.len() != (self.width as usize) * (self.height as usize)
            || self.epsg == 0
            || self.epsg >= EPSG_USER_DEFINED
        {
            return Err(invalid());
        }
        let [x, y] = self.geotransform.pixel_size;
        if !self
            .geotransform
            .origin
            .iter()
            .all(|value| value.is_finite())
            || !x.is_finite()
            || !y.is_finite()
            || x <= 0.0
            || y <= 0.0
        {
            return Err(invalid());
        }
        Ok(())
    }

    /// The INDEPENDENT spec->tree projection (M5 provenance rule): the
    /// canonical extract tree this raster must produce, computed
    /// WITHOUT running the extractor, the gdal tool, or the mock.
    #[must_use]
    pub fn canonical_tree(&self) -> Value {
        let [origin_x, origin_y] = self.geotransform.origin;
        let [pixel_x, pixel_y] = self.geotransform.pixel_size;
        let tree = json!({
            "bands": 1,
            "dtype": "u8",
            "geo": {
                "bounds": {
                    "maxx": origin_x + f64::from(self.width) * pixel_x,
                    "maxy": origin_y,
                    "minx": origin_x,
                    "miny": origin_y - f64::from(self.height) * pixel_y,
                },
                "citation": null,
                "crs_source": "geokey",
                "epsg": self.epsg,
                "model_type": if (4000..=4999).contains(&self.epsg) { "geographic" } else { "projected" },
                "pixel_size": [pixel_x, pixel_y],
                "tie_point": [0.0, 0.0, 0.0, origin_x, origin_y, 0.0],
            },
            "head_sample": self.sample_rows(),
            "height": self.height,
            "layout": self.layout(),
            "width": self.width,
        });
        tree
    }

    fn layout(&self) -> Value {
        let mut overviews = 0;
        let mut bound = self.width.max(self.height);
        while bound > COG_BLOCK {
            overviews += 1;
            bound = (bound / 2).max(1);
        }
        json!({
            "ifd_order": if overviews > 0 { "overviews-leading" } else { "single" },
            "overview_count": overviews,
            "tiled": self.cog,
        })
    }

    fn sample_rows(&self) -> Value {
        let columns = self.width.min(4) as usize;
        (0..self.height.min(4))
            .map(|row| {
                let start = row as usize * self.width as usize;
                json!(self.pixels[start..start + columns])
            })
            .collect::<Vec<_>>()
            .into()
    }
}
