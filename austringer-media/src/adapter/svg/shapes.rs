//! The eight SVG 1.1 basic shapes the generation subset supports, plus
//! the common attribute set every shape carries. JSON wire form is
//! internally tagged on `"type"` (`{"type": "rect", ...}`) with snake_case
//! keys; the writer emits the SVG kebab-case attribute names.

use serde::{Deserialize, Serialize};

use crate::error::MediaError;

/// Attributes common to every shape kind. All optional; empty strings are
/// contract violations (validated away before writing).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CommonAttrs {
    /// Element `id` (unique across the document).
    #[serde(default)]
    pub id: Option<String>,
    /// CSS `class` (space-separated).
    #[serde(default)]
    pub class: Option<String>,
    /// Fill paint (`red`, `#rgb`, `none`, ...).
    #[serde(default)]
    pub fill: Option<String>,
    /// Stroke paint.
    #[serde(default)]
    pub stroke: Option<String>,
    /// Stroke width in user units (>= 0).
    #[serde(default)]
    pub stroke_width: Option<f64>,
    /// Opacity in `[0, 1]`.
    #[serde(default)]
    pub opacity: Option<f64>,
    /// Transform list (`translate(1,2) rotate(45)`), applied verbatim.
    #[serde(default)]
    pub transform: Option<String>,
}

/// One shape in the generation subset (plan M11: rect | circle | ellipse
/// | line | polyline | polygon | path(d) | text + common attrs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Shape {
    /// `<rect x y width height rx? ry?>`.
    Rect {
        /// Left edge.
        x: f64,
        /// Top edge.
        y: f64,
        /// Width (>= 0).
        width: f64,
        /// Height (>= 0).
        height: f64,
        /// Horizontal corner radius (>= 0).
        #[serde(default)]
        rx: Option<f64>,
        /// Vertical corner radius (>= 0).
        #[serde(default)]
        ry: Option<f64>,
        /// Common attributes.
        #[serde(flatten)]
        common: CommonAttrs,
    },
    /// `<circle cx cy r>`.
    Circle {
        /// Center x.
        cx: f64,
        /// Center y.
        cy: f64,
        /// Radius (>= 0).
        r: f64,
        /// Common attributes.
        #[serde(flatten)]
        common: CommonAttrs,
    },
    /// `<ellipse cx cy rx ry>`.
    Ellipse {
        /// Center x.
        cx: f64,
        /// Center y.
        cy: f64,
        /// X radius (>= 0).
        rx: f64,
        /// Y radius (>= 0).
        ry: f64,
        /// Common attributes.
        #[serde(flatten)]
        common: CommonAttrs,
    },
    /// `<line x1 y1 x2 y2>`.
    Line {
        /// Start x.
        x1: f64,
        /// Start y.
        y1: f64,
        /// End x.
        x2: f64,
        /// End y.
        y2: f64,
        /// Common attributes.
        #[serde(flatten)]
        common: CommonAttrs,
    },
    /// `<polyline points>` - flat `[x, y, x, y, ...]`, even length, >= 2 points.
    Polyline {
        /// Coordinate list.
        points: Vec<f64>,
        /// Common attributes.
        #[serde(flatten)]
        common: CommonAttrs,
    },
    /// `<polygon points>` - flat coordinate list, even length, >= 3 points.
    Polygon {
        /// Coordinate list.
        points: Vec<f64>,
        /// Common attributes.
        #[serde(flatten)]
        common: CommonAttrs,
    },
    /// `<path d>` - the path data string, emitted verbatim.
    Path {
        /// Path data (`M 1 1 L 2 2 Z`); non-empty.
        d: String,
        /// Common attributes.
        #[serde(flatten)]
        common: CommonAttrs,
    },
    /// `<text x y>` with content.
    Text {
        /// Anchor x.
        x: f64,
        /// Baseline y.
        y: f64,
        /// Text content (XML-escaped on write).
        content: String,
        /// `font-size` in user units (> 0 when present).
        #[serde(default)]
        font_size: Option<f64>,
        /// `font-family`; defaults to a common stack on write.
        #[serde(default)]
        font_family: Option<String>,
        /// Common attributes.
        #[serde(flatten)]
        common: CommonAttrs,
    },
}

/// Default font stack for generated `<text>` (digest row: font
/// availability is the #1 render divergence - never claim exotic fonts).
pub const DEFAULT_FONT_FAMILY: &str = "sans-serif";

impl Shape {
    /// The SVG element name this shape writes (and the census key it
    /// projects to).
    #[must_use]
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Rect { .. } => "rect",
            Self::Circle { .. } => "circle",
            Self::Ellipse { .. } => "ellipse",
            Self::Line { .. } => "line",
            Self::Polyline { .. } => "polyline",
            Self::Polygon { .. } => "polygon",
            Self::Path { .. } => "path",
            Self::Text { .. } => "text",
        }
    }

    /// The common attribute set of this shape.
    #[must_use]
    pub fn common(&self) -> &CommonAttrs {
        match self {
            Self::Rect { common, .. }
            | Self::Circle { common, .. }
            | Self::Ellipse { common, .. }
            | Self::Line { common, .. }
            | Self::Polyline { common, .. }
            | Self::Polygon { common, .. }
            | Self::Path { common, .. }
            | Self::Text { common, .. } => common,
        }
    }

    /// Semantic validation: finite numbers, non-negative sizes, even and
    /// long-enough point lists, non-empty `d`, opacity/stroke-width
    /// ranges, non-empty optional strings.
    ///
    /// # Errors
    /// [`MediaError::InvalidSpec`] on any violation.
    pub fn validate(&self) -> Result<(), MediaError> {
        let invalid = || MediaError::InvalidSpec;
        let all_finite = |numbers: &[f64]| numbers.iter().all(|number| number.is_finite());
        let non_negative = |number: &f64| number.is_finite() && *number >= 0.0;
        let (numbers, nonneg): (Vec<f64>, Vec<f64>) = match self {
            Self::Rect {
                x,
                y,
                width,
                height,
                rx,
                ry,
                ..
            } => (
                [*x, *y].into(),
                [*width, *height, rx.unwrap_or(0.0), ry.unwrap_or(0.0)].into(),
            ),
            Self::Circle { cx, cy, r, .. } => ([*cx, *cy].into(), [*r].into()),
            Self::Ellipse { cx, cy, rx, ry, .. } => ([*cx, *cy].into(), [*rx, *ry].into()),
            Self::Line { x1, y1, x2, y2, .. } => ([*x1, *y1, *x2, *y2].into(), vec![]),
            Self::Polyline { points, .. } => {
                if points.len() % 2 != 0 || points.len() < 4 || !all_finite(points) {
                    return Err(invalid());
                }
                return self.validate_common();
            }
            Self::Polygon { points, .. } => {
                if points.len() % 2 != 0 || points.len() < 6 || !all_finite(points) {
                    return Err(invalid());
                }
                return self.validate_common();
            }
            Self::Path { d, .. } => {
                if d.is_empty() {
                    return Err(invalid());
                }
                return self.validate_common();
            }
            Self::Text {
                x, y, font_size, ..
            } => ([*x, *y].into(), [font_size.unwrap_or(1.0)].into()),
        };
        if !all_finite(&numbers) || nonneg.iter().any(|number| !non_negative(number)) {
            return Err(invalid());
        }
        self.validate_common()
    }

    fn validate_common(&self) -> Result<(), MediaError> {
        let invalid = || MediaError::InvalidSpec;
        let common = self.common();
        if common
            .stroke_width
            .is_some_and(|value| !value.is_finite() || value < 0.0)
        {
            return Err(invalid());
        }
        if common
            .opacity
            .is_some_and(|value| !(0.0..=1.0).contains(&value))
        {
            return Err(invalid());
        }
        for optional in [
            &common.id,
            &common.class,
            &common.fill,
            &common.stroke,
            &common.transform,
        ] {
            if optional.as_is_some_and_empty() {
                return Err(invalid());
            }
        }
        if let Self::Text { font_family, .. } = self
            && font_family.as_is_some_and_empty()
        {
            return Err(invalid());
        }
        Ok(())
    }
}

/// Non-empty check helper: `Some("")` violates the contract.
trait OptionExt {
    fn as_is_some_and_empty(&self) -> bool;
}

impl OptionExt for Option<String> {
    fn as_is_some_and_empty(&self) -> bool {
        self.as_ref().is_some_and(String::is_empty)
    }
}
