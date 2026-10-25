//! The post-edit AST syntax gate (plan todo 16, clause 2): after an edit
//! batch (or staged application) the new content is parsed via
//! `austringer-ast`. An edit that introduces NEW error/missing nodes is
//! rejected with the diagnostic; already-broken files stay editable as
//! long as the edit adds no new errors; files without a grammar (unknown
//! extension or un-compiled feature) skip the gate — the austringer-ast
//! degrade contract.

use crate::error::ToolsError;
use austringer_ast::{Analysis, Analyzer, AstError, LanguageId};
use std::path::Path;

/// Cap on diagnostics attached to a [`ToolsError::SyntaxGate`].
pub const MAX_GATE_DIAGNOSTICS: usize = 10;

/// Which language the post-edit syntax gate parses with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxGate {
    /// Resolved language; `None` (unknown extension) skips the gate.
    pub language: Option<LanguageId>,
}

impl SyntaxGate {
    /// Resolve the gate language from a path's extension.
    #[must_use]
    pub fn for_path(path: &Path) -> Self {
        SyntaxGate {
            language: LanguageId::from_path(path),
        }
    }
}

/// How the syntax gate ruled on an application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateOutcome {
    /// Post-edit parse introduced no new error/missing nodes.
    Passed,
    /// No gate was supplied.
    Off,
    /// Language unknown or grammar not compiled in (degrade contract).
    SkippedNoGrammar,
}

fn diagnostic_count(analysis: &Analysis) -> usize {
    analysis.diagnostics.len()
}

fn parse(language: LanguageId, bytes: &[u8]) -> Result<Option<Analysis>, ToolsError> {
    match Analyzer::new().analyze_bytes(language, bytes.to_vec()) {
        Ok(analysis) => Ok(Some(analysis)),
        Err(AstError::NoGrammarCompiled { .. }) => Ok(None),
        Err(other) => Err(ToolsError::GateEngine(other)),
    }
}

/// Run the gate over a before/after content pair. Shared by
/// `edit::apply_edits` and `staged_apply::apply_staged`.
pub(crate) fn run_gate(
    gate: Option<&SyntaxGate>,
    before: &[u8],
    after: &[u8],
) -> Result<GateOutcome, ToolsError> {
    let Some(gate) = gate else {
        return Ok(GateOutcome::Off);
    };
    let Some(language) = gate.language else {
        return Ok(GateOutcome::SkippedNoGrammar);
    };
    let (Some(pre), Some(post)) = (parse(language, before)?, parse(language, after)?) else {
        return Ok(GateOutcome::SkippedNoGrammar);
    };
    let added = diagnostic_count(&post) as isize - diagnostic_count(&pre) as isize;
    if added > 0 {
        let first = post
            .diagnostics
            .first()
            .map(|d| format!("{}:{} {}", d.line, d.column, d.message))
            .unwrap_or_default();
        return Err(ToolsError::SyntaxGate {
            count: added as usize,
            first,
            diagnostics: post
                .diagnostics
                .iter()
                .take(MAX_GATE_DIAGNOSTICS)
                .cloned()
                .collect(),
        });
    }
    Ok(GateOutcome::Passed)
}
