//! Stage (a): the AST gate. Every touched file must parse with ZERO NEW
//! error nodes (T14 `is_error`/`is_missing` diagnostics); files without a
//! compiled-in grammar fall back to a deterministic balanced-delimiter
//! heuristic (documented below, disclosed as a skip notice).

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use austringer_ast::{AnalyzeOutcome, Analyzer, Diagnostic, DiagnosticKind};

use super::error::{PreFlightError, SkipNotice, SkipReason, Stage};
use super::structured::StructuredError;

pub(super) const GATE_TOOL: &str = "ast-gate";
const FALLBACK_TOOL: &str = "delimiter-heuristic";

/// Run the gate over `touched` (shadow-relative paths). Returns the new
/// structured errors plus disclosure notices (fallback / non-text skips).
pub(super) fn run(
    shadow_root: &Path,
    target_root: &Path,
    touched: &[PathBuf],
    analyzer: &Analyzer,
) -> Result<(Vec<StructuredError>, Vec<SkipNotice>), PreFlightError> {
    let mut errors = Vec::new();
    let mut notices = Vec::new();
    for rel in touched {
        let shadow_file = shadow_root.join(rel);
        let post = read(&shadow_file).map_err(|source| PreFlightError::ReadTouched {
            path: rel.clone(),
            source,
        })?;
        let pre = read_optional(&target_root.join(rel)).map_err(|source| {
            PreFlightError::ReadTouched {
                path: rel.clone(),
                source,
            }
        })?;
        match analyzer
            .analyze_path(&long_path(&shadow_file))
            .map_err(|source| PreFlightError::Analyze {
                path: rel.clone(),
                source,
            })? {
            AnalyzeOutcome::Analyzed(post_analysis) => {
                let language = post_analysis.language;
                let pre_analysis = pre
                    .map(|bytes| analyzer.analyze_bytes(language, bytes))
                    .transpose()
                    .map_err(|source| PreFlightError::Analyze {
                        path: rel.clone(),
                        source,
                    })?;
                collect_new_diagnostics(
                    rel,
                    pre_analysis.as_ref(),
                    Some(&post_analysis),
                    &mut errors,
                );
            }
            AnalyzeOutcome::NoGrammar { .. } => {
                gate_no_grammar(rel, &pre, &post, &mut errors, &mut notices);
            }
        }
    }
    Ok((errors, notices))
}

/// Multiset difference of diagnostics: what `post` has that `pre` does
/// not, matched position-exactly on (kind, line, column, message). A
/// baseline diagnostic that merely shifted lines counts as new — the
/// conservative direction for a gate (it may over-report around big
/// rewrites; it never reports a false pass).
fn collect_new_diagnostics(
    rel: &Path,
    pre: Option<&austringer_ast::Analysis>,
    post: Option<&austringer_ast::Analysis>,
    errors: &mut Vec<StructuredError>,
) {
    let Some(post) = post else { return };
    let mut baseline: BTreeMap<(bool, usize, usize, String), usize> = BTreeMap::new();
    if let Some(pre) = pre {
        for diagnostic in &pre.diagnostics {
            *baseline.entry(diagnostic_key(diagnostic)).or_insert(0) += 1;
        }
    }
    for diagnostic in &post.diagnostics {
        let key = diagnostic_key(diagnostic);
        match baseline.get_mut(&key) {
            Some(count) if *count > 0 => *count -= 1,
            _ => errors.push(StructuredError {
                stage: Stage::AstGate,
                tool: GATE_TOOL,
                path: Some(rel.to_path_buf()),
                line: Some(diagnostic.line),
                column: Some(diagnostic.column),
                message: diagnostic.message.clone(),
            }),
        }
    }
}

fn diagnostic_key(diagnostic: &Diagnostic) -> (bool, usize, usize, String) {
    (
        diagnostic.kind == DiagnosticKind::MissingNode,
        diagnostic.line,
        diagnostic.column,
        diagnostic.message.clone(),
    )
}

/// Deterministic fallback for grammar-less files: the three bracket pairs
/// must stay balanced relative to the pre-image. Pure counting — strings
/// and comments may contain unbalanced characters, so a false FAIL is
/// possible (conservative: the gate never lies "success"); a false pass
/// on structurally broken text is out of scope for text files.
fn gate_no_grammar(
    rel: &Path,
    pre: &Option<Vec<u8>>,
    post: &[u8],
    errors: &mut Vec<StructuredError>,
    notices: &mut Vec<SkipNotice>,
) {
    notices.push(SkipNotice {
        stage: Stage::AstGate,
        tool: FALLBACK_TOOL,
        reason: SkipReason::NoGrammarFallback,
    });
    let Some(post_text) = as_text(post) else {
        notices.push(SkipNotice {
            stage: Stage::AstGate,
            tool: FALLBACK_TOOL,
            reason: SkipReason::NotText,
        });
        return;
    };
    let pre_balanced = pre.as_ref().and_then(|bytes| as_text(bytes)).map(|text| {
        [
            balance(text, '(', ')'),
            balance(text, '{', '}'),
            balance(text, '[', ']'),
        ]
    });
    let post_balance = [
        balance(post_text, '(', ')'),
        balance(post_text, '{', '}'),
        balance(post_text, '[', ']'),
    ];
    for (index, (open, close)) in [('(', ')'), ('{', '}'), ('[', ']')].iter().enumerate() {
        let newly_unbalanced =
            !post_balance[index] && pre_balanced.is_none_or(|balances| balances[index]);
        if newly_unbalanced {
            errors.push(StructuredError {
                stage: Stage::AstGate,
                tool: FALLBACK_TOOL,
                path: Some(rel.to_path_buf()),
                line: None,
                column: None,
                message: format!("unbalanced '{open}'/'{close}' delimiters (heuristic)"),
            });
        }
    }
}

fn as_text(bytes: &[u8]) -> Option<&str> {
    std::str::from_utf8(bytes).ok()
}

fn balance(text: &str, open: char, close: char) -> bool {
    text.matches(open).count() == text.matches(close).count()
}

fn read(path: &Path) -> io::Result<Vec<u8>> {
    std::fs::read(long_path(path))
}

fn read_optional(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Extended-length form for absolute paths on Windows so >260-char shadow
/// trees read correctly regardless of the machine's long-path policy.
/// Only applies to long absolute paths, and only when already valid
/// UTF-8 (verbatim `\\?\` paths do not accept forward-slash or relative
/// forms; short/unusual paths pass through untouched).
fn long_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let text = path.as_os_str().to_str();
        if let Some(text) = text
            && path.is_absolute()
            && !text.starts_with(r"\\?\")
            && text.len() >= 200
        {
            return PathBuf::from(format!(r"\\?\{text}"));
        }
    }
    #[cfg(not(windows))]
    let _ = path;
    path.to_path_buf()
}
