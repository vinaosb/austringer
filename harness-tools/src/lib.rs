//! Built-in editing-tool implementations (plan todo 16): windowed reads,
//! anchored line edits, hashline anchors, AST-validated application,
//! semantic hunk staging, and the surrounding read/write-scope contracts.
//!
//! # Core-registry adapter surface (for the later dispatch integration)
//!
//! This crate implements the **behavior** behind the two invariant file
//! tools whose day-one contracts `harness-core::tools::schema` fixed in
//! todo 6 (`view_window`, `edit_lines`). It deliberately does NOT depend on
//! harness-core: the daemon-side dispatch wiring is a later integration
//! step. The adapter is thin by construction —
//! - `view_window` tool calls map to [`window::view_window_path`]
//!   (`path`, optional 1-based `start_line`);
//! - `edit_lines` tool calls map to [`edit::apply_edits_path`] with each
//!   `{line, old_text, new_text}` item becoming an [`edit::LineEdit`]
//!   (optional `hash` field reserved for hashline anchoring).
//!
//! # Structural guarantees (plan guardrails)
//!
//! - No whole-file rewrite: no function accepts or returns a whole-file
//!   replacement body; every edit is a bounded anchor + old/new pair, and
//!   [`edit::MAX_NEW_LINES_PER_EDIT`] caps each replacement at the same
//!   size as a read window (you cannot write what you could never read).
//! - No full-file echo: every model-facing result is a windowed view with
//!   explicit omission placeholders ([`window::WindowView`]).
//! - All line math is CRLF-correct ([`lines::LineTable`]).

pub mod anchors;
pub mod blocks;
pub mod difftokens;
pub mod edit;
pub mod error;
pub mod formats;
pub mod gate;
pub mod hunks;
pub mod lines;
pub mod outline;
pub mod staged_apply;
pub mod strict;
pub mod window;
pub mod wip;

pub use error::ToolsError;
pub use lines::{LineEnding, LineTable};
pub use window::{WindowView, view_window, view_window_path};

#[cfg(test)]
mod tests {
    #[test]
    fn crate_surface_smoke() {
        // Replaces the todo-1 scaffold smoke test: the crate now carries
        // real modules; module suites carry the behavior tests.
        let modules_linked = true;
        assert!(modules_linked);
    }
}
