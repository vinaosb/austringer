//! The svg generation-spec schema: the typed subset a model may drive
//! (plan M11: viewBox, title/desc, shapes + common attrs). Parsed strictly
//! from [`GenerationSpec::params`] - a spec that does not satisfy the
//! contract is the typed `InvalidSpec`, never a guessed default.

use serde::{Deserialize, Serialize};

use crate::adapter::svg::shapes::Shape;
use crate::error::MediaError;
use crate::format::FormatId;
use crate::generate::GenerationSpec;

/// Upper bound on shapes one document may carry (bounded writer output).
pub const MAX_SHAPES: usize = 10_000;

/// The svg generation spec: root geometry, accessibility elements, and
/// the ordered shape list (document order is spec order).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SvgSpec {
    /// Root `viewBox` as `[min_x, min_y, width, height]`.
    #[serde(default, rename = "viewBox")]
    pub view_box: Option<[f64; 4]>,
    /// Root `width` in user units.
    #[serde(default)]
    pub width: Option<f64>,
    /// Root `height` in user units.
    #[serde(default)]
    pub height: Option<f64>,
    /// `<title>` content (accessibility; also shown by hover tooltips).
    #[serde(default)]
    pub title: Option<String>,
    /// `<desc>` content (accessibility description).
    #[serde(default)]
    pub desc: Option<String>,
    /// Shapes in document order.
    #[serde(default)]
    pub shapes: Vec<Shape>,
}

impl SvgSpec {
    /// Parse and validate a generation spec addressed to svg. Specs for
    /// any other format, or params violating the contract below, are the
    /// typed [`MediaError::InvalidSpec`].
    ///
    /// # Errors
    /// [`MediaError::InvalidSpec`] on format mismatch, malformed params,
    /// or a semantic violation (negative geometry, non-finite numbers,
    /// odd/short point lists, empty `d`, out-of-range opacity, duplicate
    /// ids, more than [`MAX_SHAPES`] shapes).
    pub fn from_generation(spec: &GenerationSpec) -> Result<Self, MediaError> {
        if spec.format != FormatId::Svg {
            return Err(MediaError::InvalidSpec);
        }
        let parsed: Self =
            serde_json::from_value(spec.params.clone()).map_err(|_| MediaError::InvalidSpec)?;
        parsed.validate()?;
        Ok(parsed)
    }

    /// The INDEPENDENT spec->tree projection (M5 provenance rule): the
    /// canonical tree this spec's document must extract to, computed
    /// WITHOUT running the extractor or the writer.
    ///
    /// # Errors
    /// Never in practice (the tree is plain data); the impossible serde
    /// failure surfaces as a null tree rather than a panic.
    pub fn canonical_tree(&self) -> serde_json::Value {
        let tree = super::tree::spec_tree(self);
        match serde_json::to_value(tree) {
            Ok(value) => value,
            Err(_) => serde_json::Value::Null,
        }
    }

    fn validate(&self) -> Result<(), MediaError> {
        let invalid = || MediaError::InvalidSpec;
        if self.shapes.len() > MAX_SHAPES {
            return Err(invalid());
        }
        if let Some(view_box) = self.view_box
            && view_box.iter().any(|number| !number.is_finite())
        {
            return Err(invalid());
        }
        for number in [self.width, self.height] {
            if number.is_some_and(|value| !value.is_finite() || value < 0.0) {
                return Err(invalid());
            }
        }
        let mut ids = std::collections::BTreeSet::new();
        for shape in &self.shapes {
            shape.validate()?;
            if let Some(id) = shape.common().id.as_deref()
                && (id.is_empty() || !ids.insert(id.to_owned()))
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}
