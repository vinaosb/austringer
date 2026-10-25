//! Brace-balanced JSON object scanning over bounded screen text (the T19
//! discipline: what we parse is already windowed). Because cargo emits
//! one JSON object per diagnostic and PTY grids may soft-wrap long lines,
//! extraction scans for BALANCED objects (string- and escape-aware)
//! instead of parsing line-by-line — a wrap-inserted newline mid-object
//! is just whitespace to the scanner.

use serde::Deserialize;

use super::error::Stage;
use super::structured::{MAX_SURFACED_ERRORS, StructuredError};

/// Upper bound on objects parsed per extraction (bounded work on
/// adversarial output).
const MAX_OBJECTS: usize = 512;

#[derive(Deserialize)]
struct CargoMessage {
    reason: String,
    #[serde(default)]
    message: Option<Diagnostic>,
}

#[derive(Deserialize)]
struct Diagnostic {
    level: String,
    message: String,
    #[serde(default)]
    spans: Vec<Span>,
}

#[derive(Deserialize)]
struct Span {
    file_name: String,
    line_start: usize,
    column_start: usize,
    #[serde(default)]
    primary: bool,
}

/// Yield every brace-balanced `{...}` substring, skipping garbage between
/// objects. Cap at [`MAX_OBJECTS`].
fn scan_objects(text: &str) -> Vec<&str> {
    let mut objects = Vec::new();
    let mut depth: usize = 0;
    let mut start: usize = 0;
    let mut in_string = false;
    let mut escaped = false;
    for (index, ch) in text.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    start = index;
                }
                depth += 1;
            }
            '}' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    objects.push(&text[start..=index]);
                    if objects.len() == MAX_OBJECTS {
                        return objects;
                    }
                }
            }
            _ => {}
        }
    }
    objects
}

/// cargo-check extraction: `reason == "compiler-message"` with
/// `level == "error"` only (warnings are counted out by construction),
/// primary span first, file:line:col + the structured message — the
/// `rendered` block is never read, so MSVC-style multi-line renderings
/// are irrelevant by construction. Unparseable objects (non-UTF-8
/// corruption, truncation) are skipped silently: the bounded fallback in
/// `with_fallback` covers the nothing-parseable case.
pub(super) fn cargo_errors(text: &str, tool: &'static str) -> Vec<StructuredError> {
    let mut errors = Vec::new();
    for object in scan_objects(text) {
        let Ok(parsed) = serde_json::from_str::<CargoMessage>(object) else {
            continue;
        };
        if parsed.reason != "compiler-message" {
            continue;
        }
        let Some(diagnostic) = parsed.message else {
            continue;
        };
        if diagnostic.level != "error" {
            continue;
        }
        let span = diagnostic
            .spans
            .iter()
            .find(|span| span.primary)
            .or_else(|| diagnostic.spans.first());
        errors.push(StructuredError {
            stage: Stage::Check,
            tool,
            path: span.map(|span| span.file_name.clone().into()),
            line: span.map(|span| span.line_start),
            column: span.map(|span| span.column_start),
            message: diagnostic.message.clone(),
        });
        if errors.len() == MAX_SURFACED_ERRORS {
            break;
        }
    }
    super::structured::with_fallback(Stage::Check, tool, errors, text)
}
