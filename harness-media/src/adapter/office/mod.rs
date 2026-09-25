//! # Office read adapters, pure tier (plan M18) - OOXML extract
//!
//! docx/xlsx/pptx are ZIP packages of XML parts (ECMA-376 5th ed.,
//! Dec 2021; the ISO/IEC 29500 line lags): read here through the zip
//! crate's pure-Rust deflate backend plus quick-xml over each part -
//! no C builds, no evaluation, no macro execution. Writing is M19
//! (xlsx/docx pure writers + pptx via the pandoc subprocess); every
//! generate/verify refusal names that lane.
//!
//! ## The recorded XLSX fast-path pick (calamine vs zip+quick-xml)
//!
//! calamine 0.36.1 (pure-Rust xlsx/xlsb/ods reader, very active) was
//! the plan's candidate for the xlsx extract fast path. This adapter
//! picks zip+quick-xml for all three formats, recorded here: the M18
//! tree needs the raw part layer calamine does not expose (the
//! defined-styles census from styles.xml, vbaProject.bin macro
//! disclosure, the external-rels listing), cell reading here is
//! bounded (head-cell sample under caller budgets; calamine
//! materializes whole sheet ranges against that contract), and one
//! shared stack keeps budget enforcement and ParseFailure semantics
//! uniform across the family. Revisit only if a whole-sheet census
//! requirement ever appears.
//!
//! ## Honesty rows (surfaced verbatim by the M27 matrix)
//!
//! - Licensing (digest 3.1 OOXML row): ECMA-376 5th ed. under the MS
//!   Open Specification Promise - irrevocable covenant, current +
//!   future versions, no royalty or field-of-use limit; YELLOW treated
//!   as GREEN. Extraction targets TRANSITIONAL documents (what Word,
//!   LibreOffice and Google emit); MS extension namespaces (w14:, w15:,
//!   mc:AlternateContent) live in OSP-listed [MS-*] open specs and are
//!   skipped, never interpreted.
//! - Formulas are listed AS TEXT with an `=` prefix and NEVER
//!   evaluated; stored `<v>` values are data, never invented (a
//!   formula without a cached `<v>` reads as its text alone - the
//!   digest fidelity note).
//! - vbaProject.bin and external relationships are LISTED (macro
//!   presence disclosed: `macros present: N, not executed`), never
//!   executed, never extracted.
//! - Extract trees carry a defined-styles census (styles.xml names) so
//!   an M19 writer with broken style definitions FAILS the round-trip
//!   diff - applied style names alone would pass.
//! - Slide-master/layout inheritance is NOT interpreted: the pptx tree
//!   is a per-slide shape census plus text runs (where naive parsers
//!   fail - this one does not pretend to resolve inheritance).
//! - Legacy OLE2 binaries (`D0 CF 11 E0`) are RED (digest 3.1,
//!   LibreOffice-read-only): the M2 probe identifies them typed and
//!   this lane never sees them.
//!
//! ## Deviation note (ParseFailure detail)
//!
//! The plan's QA line asks for "ParseFailure with part name"; the
//! shared [`crate::error::MediaError::ParseFailure`] is a unit variant
//! (the M11 svg lane shipped the same shape for "line/col"), so the
//! failing part name does not ride the error value. The refusal is
//! typed either way; the failing part is pinned by the per-part tests.

use harness_core::event::ProjectionKind;

use crate::error::MediaError;
use crate::format::FormatId;
use crate::projection::{
    ProjectionContext, ProjectionProvider, ProjectionRegistry, ProjectionWiringError,
};

pub mod docx;
mod parts;
pub mod pptx;
pub mod xlsx;
mod xlsx_sheet;

/// Producer-disclosure engine name for every office op (probe stays
/// under the shared sniffing engine).
pub const ENGINE: &str = "harness-media::office";

/// The office per-family text-projection bound: 1200 CHARS (the M1
/// ceiling is 2000; the office default is deliberately lower).
pub const TEXT_PROJECTION_MAX_CHARS: usize = 1200;

const _: () = assert!(TEXT_PROJECTION_MAX_CHARS <= crate::extract::TEXT_PROJECTION_MAX_CHARS);

/// The office deterministic projector (M10 seam): a thin wrap over the
/// per-format extractor's bounded text projection - same bytes, same
/// projection, never a fabricated transcript.
pub struct OfficeProjector {
    format: FormatId,
}

impl OfficeProjector {
    /// The projector for one office format.
    #[must_use]
    pub fn new(format: FormatId) -> Self {
        Self { format }
    }
}

impl ProjectionProvider for OfficeProjector {
    fn kind(&self) -> ProjectionKind {
        ProjectionKind::TextProjection
    }

    fn project(&self, bytes: &[u8], context: &ProjectionContext<'_>) -> Result<String, MediaError> {
        match self.format {
            FormatId::Docx => docx::extract(bytes, context.budgets),
            FormatId::Xlsx => xlsx::extract(bytes, context.budgets),
            FormatId::Pptx => pptx::extract(bytes, context.budgets),
            _ => Err(MediaError::NoProjector),
        }
        .map(|extracted| extracted.text_projection)
    }
}

/// Registers the office projectors (one per format).
///
/// # Errors
/// [`ProjectionWiringError::DuplicateProjector`] on contention (one
/// format, one projector).
pub fn register_projections(
    registry: &mut ProjectionRegistry,
) -> Result<(), ProjectionWiringError> {
    for format in [FormatId::Docx, FormatId::Xlsx, FormatId::Pptx] {
        registry.register(format, std::sync::Arc::new(OfficeProjector::new(format)))?;
    }
    Ok(())
}
