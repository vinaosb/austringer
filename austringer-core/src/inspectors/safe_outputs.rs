//! gh-aw SAFE-OUTPUTS pattern (Copilot, research §20): tool results are
//! untrusted output.
//!
//! Three controls live here, applied before a result leaves the tool
//! boundary:
//!
//! 1. [`OutputBudget`] — read-only result tokens by default: every result
//!    is capped, and capping is explicit (an omission marker), never
//!    silent elision.
//! 2. [`WriteChannel`] — ONE validated write channel: a typed allowlist of
//!    tool names that may emit writes. Everything else is read-only by
//!    policy; the default allowlist is empty.
//! 3. [`ThreatScanner`] — a hook that scans result text before it enters
//!    the model context. The default [`NoThreatScan`] is a no-op;
//!    flagged blocks are quarantined (replaced by a typed notice, payload
//!    dropped).
//!
//! Wiring point: executors (todo 6's registry) route finished results
//! through [`SafeOutputs::sanitize`] and consult [`WriteChannel::validate`]
//! before performing writes — the approval chain cannot do this, it runs
//! before execution.

use crate::event::content::ContentBlock;
use crate::inspectors::glob::{Glob, InvalidGlob};

/// Default per-result output cap in characters.
pub const DEFAULT_OUTPUT_CAP_CHARS: usize = 8192;

/// Per-result output cap. Truncation always leaves an explicit marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputBudget {
    /// Maximum characters of text kept per result block.
    pub max_chars: usize,
}

impl Default for OutputBudget {
    fn default() -> Self {
        Self {
            max_chars: DEFAULT_OUTPUT_CAP_CHARS,
        }
    }
}

impl OutputBudget {
    /// Caps `text` to [`OutputBudget::max_chars`] characters plus an
    /// explicit omission marker; under-budget text passes through
    /// unchanged.
    #[must_use]
    pub fn truncate(&self, text: &str) -> String {
        let total = text.chars().count();
        if total <= self.max_chars {
            return text.to_owned();
        }
        let kept: String = text.chars().take(self.max_chars).collect();
        format!(
            "{kept}\n[... output truncated: showing {} of {total} characters ...]",
            self.max_chars
        )
    }
}

/// Why a write was refused by the one validated write channel.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("write refused: tool '{tool}' is not in the write allowlist {}", allowed.join(", "))]
pub struct WriteRefused {
    /// The tool that attempted to emit a write.
    pub tool: String,
    /// The allowlist patterns that were checked.
    pub allowed: Vec<String>,
}

/// ONE validated write channel: the typed allowlist of tools that may
/// emit writes. The default is empty — read-only by default.
#[derive(Debug, Clone, Default)]
pub struct WriteChannel {
    allowed_writers: Vec<Glob>,
}

impl WriteChannel {
    /// Compiles the allowlist; malformed patterns are rejected (fail
    /// closed — an unresolvable glob can never widen the channel).
    pub fn new(allowed: &[&str]) -> Result<Self, InvalidGlob> {
        let mut allowed_writers = Vec::with_capacity(allowed.len());
        for pattern in allowed {
            allowed_writers.push(Glob::new(pattern)?);
        }
        Ok(Self { allowed_writers })
    }

    /// Whether `tool` is on the write allowlist.
    #[must_use]
    pub fn may_emit_writes(&self, tool: &str) -> bool {
        self.allowed_writers.iter().any(|glob| glob.matches(tool))
    }

    /// Typed refusal when `tool` may not write.
    pub fn validate(&self, tool: &str) -> Result<(), WriteRefused> {
        if self.may_emit_writes(tool) {
            Ok(())
        } else {
            Err(WriteRefused {
                tool: tool.to_owned(),
                allowed: self
                    .allowed_writers
                    .iter()
                    .map(|glob| glob.pattern().to_owned())
                    .collect(),
            })
        }
    }
}

/// One threat-scan judgment over a result text block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreatVerdict {
    /// The text may enter model context.
    Clean,
    /// The text is untrusted and must be quarantined.
    Flagged {
        /// Why the text was flagged.
        reason: String,
    },
}

/// Threat-scanning hook over tool-result text before it leaves the tool
/// boundary. No live API: implementations are injected.
pub trait ThreatScanner: Send + Sync {
    /// Scans one result text block.
    fn scan(&self, text: &str) -> ThreatVerdict;
}

/// No-op default scanner.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoThreatScan;

impl ThreatScanner for NoThreatScan {
    fn scan(&self, _text: &str) -> ThreatVerdict {
        ThreatVerdict::Clean
    }
}

/// The sanitized form of one tool result.
#[derive(Debug, Clone, PartialEq)]
pub struct SanitizedOutput {
    /// The blocks that may enter model context.
    pub blocks: Vec<ContentBlock>,
    /// Reasons for every quarantined block.
    pub quarantined: Vec<String>,
    /// Whether any block was capped.
    pub truncated: bool,
}

/// The output-side guard bundle: budget + write channel + threat scanner.
pub struct SafeOutputs {
    /// Per-result output cap.
    pub budget: OutputBudget,
    /// The one validated write channel.
    pub writes: WriteChannel,
    /// Threat-scanning hook.
    pub scanner: Box<dyn ThreatScanner>,
}

impl Default for SafeOutputs {
    fn default() -> Self {
        Self {
            budget: OutputBudget::default(),
            writes: WriteChannel::default(),
            scanner: Box::new(NoThreatScan),
        }
    }
}

impl SafeOutputs {
    /// Sanitizes one finished tool result: threat-scan first (flagged
    /// blocks are replaced by a typed quarantine notice, payload dropped),
    /// then cap the survivors. Image blocks pass through (they are
    /// out-of-line content hashes).
    #[must_use]
    pub fn sanitize(&self, blocks: Vec<ContentBlock>) -> SanitizedOutput {
        let mut sanitized = Vec::with_capacity(blocks.len());
        let mut quarantined = Vec::new();
        let mut truncated = false;
        for block in blocks {
            let ContentBlock::Text { text } = block else {
                sanitized.push(block);
                continue;
            };
            match self.scanner.scan(&text) {
                ThreatVerdict::Clean => {
                    let capped = self.budget.truncate(&text);
                    truncated |= capped != text;
                    sanitized.push(ContentBlock::Text { text: capped });
                }
                ThreatVerdict::Flagged { reason } => {
                    quarantined.push(reason.clone());
                    sanitized.push(ContentBlock::Text {
                        text: format!(
                            "output quarantined by threat scanner: {reason} \
                             (untrusted content dropped)"
                        ),
                    });
                }
            }
        }
        SanitizedOutput {
            blocks: sanitized,
            quarantined,
            truncated,
        }
    }
}
