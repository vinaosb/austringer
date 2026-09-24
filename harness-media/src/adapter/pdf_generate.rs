//! pdf generate: the krilla simple-text-layout writer (plan M13).
//!
//! Spec contract (typed, validated before any bytes are produced):
//! `pages` is an array of `{blocks: [{text, size, font?}]}` where
//! `font` optionally names a bundled subset (`"latin"` default,
//! `"cjk"`); `page_size` optionally overrides the A4 default with
//! `{width, height}` in points; `title` optionally sets Info metadata.
//!
//! Blocks are stacked top-down with a fixed leading (SIMPLE TEXT
//! LAYOUT: the producer engine id says so, with no justification,
//! hyphenation, or pagination claims). Every used font is subsetted
//! and EMBEDDED by krilla with a ToUnicode CMap, the
//! anti-unembedded-font law. Document dates are pinned to the M1
//! determinism epoch.

use krilla::geom::Point;
use krilla::metadata::DateTime;
use krilla::metadata::Metadata;
use krilla::page::PageSettings;
use krilla::text::Font;
use krilla::text::TextDirection;
use krilla::Document;
use serde::Deserialize;

use crate::error::MediaError;
use crate::format::FormatId;
use crate::generate::GenerationSpec;

/// Bundled OFL Noto subsets (fixtures/fonts/README.md records
/// provenance): the writer must ship real glyph coverage for its
/// non-Latin1 round-trip law - an unembedded or coverage-less font is
/// the #1 agent-PDF defect, so the adapter owns its fonts.
const FONT_LATIN_CYRILLIC: &[u8] = include_bytes!("../../fixtures/fonts/latin-cyrillic.ttf");
const FONT_CJK: &[u8] = include_bytes!("../../fixtures/fonts/cjk.otf");

/// Default page size: A4 portrait, points.
const DEFAULT_PAGE: (f32, f32) = (595.0, 842.0);
/// Page margins and line leading, points.
const MARGIN: f32 = 56.0;
const LEADING: f32 = 1.6;

/// Spec caps: pathological specs fail typed InvalidSpec, never the
/// writer's internal limits.
const MAX_PAGES: usize = 2000;
const MAX_BLOCKS_PER_PAGE: usize = 1000;
const MAX_BLOCK_CHARS: usize = 10_000;
const MIN_FONT_SIZE: f32 = 4.0;
const MAX_FONT_SIZE: f32 = 288.0;
const MIN_DIM: f32 = 72.0;
const MAX_DIM: f32 = 14_400.0;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BlockSpec {
    pub(crate) text: String,
    size: f32,
    font: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PageSpec {
    pub(crate) blocks: Vec<BlockSpec>,
}

#[derive(Debug, Clone, Deserialize)]
struct PageSizeSpec {
    width: f32,
    height: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PdfSpec {
    pub(crate) pages: Vec<PageSpec>,
    page_size: Option<PageSizeSpec>,
    title: Option<String>,
}

fn invalid() -> MediaError {
    MediaError::InvalidSpec
}

fn is_finite_in_range(value: f32, low: f32, high: f32) -> bool {
    value.is_finite() && value >= low && value <= high
}

/// Parses + validates the spec (shared by the writer and the independent
/// spec->tree projection, so one contract governs both).
pub(crate) fn parse_spec(spec: &GenerationSpec) -> Result<PdfSpec, MediaError> {
    if spec.format != FormatId::Pdf {
        return Err(invalid());
    }
    let parsed: PdfSpec = serde_json::from_value(spec.params.clone()).map_err(|_| invalid())?;
    if parsed.pages.is_empty() || parsed.pages.len() > MAX_PAGES {
        return Err(invalid());
    }
    for page in &parsed.pages {
        if page.blocks.is_empty() || page.blocks.len() > MAX_BLOCKS_PER_PAGE {
            return Err(invalid());
        }
        for block in &page.blocks {
            if block.text.chars().count() > MAX_BLOCK_CHARS
                || !is_finite_in_range(block.size, MIN_FONT_SIZE, MAX_FONT_SIZE)
                || block.font.as_deref().is_some_and(|font| !matches!(font, "latin" | "cjk"))
            {
                return Err(invalid());
            }
        }
    }
    if let Some(size) = &parsed.page_size
        && (!is_finite_in_range(size.width, MIN_DIM, MAX_DIM)
            || !is_finite_in_range(size.height, MIN_DIM, MAX_DIM))
    {
        return Err(invalid());
    }
    Ok(parsed)
}

fn bundled_font(name: &str) -> Option<Font> {
    let bytes = match name {
        "cjk" => FONT_CJK,
        _ => FONT_LATIN_CYRILLIC,
    };
    Font::new(bytes.to_vec().into(), 0)
}

/// The pdf generate handler: validated spec -> deterministic PDF bytes
/// (krilla simple text layout, embedded subsetted fonts with ToUnicode
/// maps, dates pinned to the M1 epoch).
pub fn generate(spec: &GenerationSpec) -> Result<Vec<u8>, MediaError> {
    let parsed = parse_spec(spec)?;
    let (width, height) = parsed
        .page_size
        .as_ref()
        .map_or(DEFAULT_PAGE, |size| (size.width, size.height));

    let mut document = Document::new();
    let mut metadata = Metadata::new().producer("harness-media pdf adapter".into());
    if let Some(title) = &parsed.title {
        metadata = metadata.title(title.clone());
    }
    // M1 determinism contract: the document clock is pinned to the fixed
    // epoch (2026-01-01T00:00:00Z) unless a future spec field overrides.
    metadata = metadata.creation_date(epoch_datetime());
    document.set_metadata(metadata);

    let mut y = height - MARGIN;
    for page_spec in &parsed.pages {
        let settings = PageSettings::from_wh(width, height).ok_or(invalid())?;
        let mut krilla_page = document.start_page_with(settings);
        let mut surface = krilla_page.surface();
        for block in &page_spec.blocks {
            let font_name = block.font.as_deref().unwrap_or("latin");
            let font = bundled_font(font_name).ok_or(invalid())?;
            y -= block.size * LEADING;
            surface.draw_text(
                Point::from_xy(MARGIN, y),
                font,
                block.size,
                &block.text,
                false,
                TextDirection::Auto,
            );
        }
        surface.finish();
        krilla_page.finish();
        y = height - MARGIN;
    }
    document.finish().map_err(|_| invalid())
}

/// The fixed epoch (2026-01-01T00:00:00Z, [`DETERMINISM_EPOCH_UNIX`]) as
/// a krilla metadata date: UTC midnight, all offsets zero.
fn epoch_datetime() -> DateTime {
    DateTime::new(2026).month(1).day(1)
}
