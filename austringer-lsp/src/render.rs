//! Compact tool-result rendering with a hard character budget.
//!
//! Token-ness is approximated by characters: the plan budget is "< 25
//! tokens per location list"; at roughly 4 characters per token for
//! path/number text (conservative vs. common BPE tokenizers),
//! [`LOCATION_BUDGET_CHARS`] = 100 chars ≈ 25 tokens. The budget is a
//! ceiling on the whole rendered list, not per location.

use std::path::Path;

use crate::provider::Location;

/// Rendered-location budget: 100 chars ≈ 25 tokens (see module docs).
pub const LOCATION_BUDGET_CHARS: usize = 100;

/// Hard bound on how many typed locations a navigation answer carries.
pub const MAX_LOCATIONS: usize = 20;

/// Renders locations as compact `path:line:col` entries (1-based), paths
/// relative to `root` when they live under it, joined by `", "`. If the
/// list does not fit, it is truncated within budget with a `+N` marker.
#[must_use]
pub fn render_locations(root: &Path, locations: &[Location]) -> String {
    let budget = LOCATION_BUDGET_CHARS;
    // Reserve room for a "+N" suffix (up to 5 chars covers 4 digits).
    let reserve = 6;
    let mut out = String::new();
    let mut shown = 0usize;

    for location in locations.iter().take(MAX_LOCATIONS) {
        let mut piece = format!(
            "{}:{}:{}",
            relative_display(root, &location.file),
            location.line,
            location.column
        );
        truncate_chars(&mut piece, budget);

        let separator = if out.is_empty() { 0 } else { 2 };
        let needed = out.chars().count() + separator + piece.chars().count();
        if !out.is_empty() && needed + reserve > budget {
            break;
        }
        if !out.is_empty() {
            out.push_str(", ");
        }
        out.push_str(&piece);
        shown += 1;
    }

    let total = locations.len().min(MAX_LOCATIONS);
    if shown < total {
        let suffix = format!("+{}", total - shown);
        out = truncate_chars_owned(out, budget.saturating_sub(suffix.chars().count()));
        out.push_str(&suffix);
    }
    out
}

/// Path as displayed: relative to `root` (forward slashes) when inside it,
/// otherwise the full forward-slashed path.
fn relative_display(root: &Path, file: &Path) -> String {
    let file_fwd = file.to_string_lossy().replace('\\', "/");
    let root_fwd = root.to_string_lossy().replace('\\', "/");
    let root_trimmed = root_fwd.trim_end_matches('/');
    if !root_trimmed.is_empty()
        && let Some(stripped) = file_fwd.strip_prefix(&format!("{root_trimmed}/"))
    {
        return stripped.to_owned();
    }
    file_fwd
}

/// Truncates `s` in place to at most `budget` chars, appending `…` when
/// truncation happened.
fn truncate_chars(s: &mut String, budget: usize) {
    if s.chars().count() > budget {
        *s = truncate_chars_owned(std::mem::take(s), budget.saturating_sub(1));
        s.push('…');
    }
}

/// `to_owned` variant returning a prefix of at most `budget` chars.
fn truncate_chars_owned(s: String, budget: usize) -> String {
    s.chars().take(budget).collect()
}
