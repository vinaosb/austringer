//! The named, ordered eviction ladder and the tool-result pruning
//! budgets (plan todo 12).
//!
//! Vocabulary (Gemini CLI): masking → distillation → compression →
//! truncation — four named stages in one canonical order, each carrying
//! an explicit trigger percent and reclaim budget in config. The engine
//! walks the ladder in order and stops the moment the budget target is
//! met. The Grok tool-result pruning ladder rides stages 3–4: compression
//! is the soft-trim (head/tail excerpts), truncation the hard-clear
//! (placeholder + retrieval pointer).

/// The ordered ladder stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LadderStage {
    /// Clear stale tool outputs to typed placeholders (keep return
    /// codes). Cheapest, lossy on bulk only.
    Masking,
    /// Condense middle-zone conversation via the cheap-model seam into
    /// `<active_plan>`-style typed blocks.
    Distillation,
    /// Soft-trim large tool results to head/tail excerpts.
    Compression,
    /// Hard-clear remaining tool results to placeholder + retrieval
    /// pointer. Last resort, unlimited budget.
    Truncation,
}

impl LadderStage {
    /// The canonical stage name (config keys, decision logs).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Masking => "masking",
            Self::Distillation => "distillation",
            Self::Compression => "compression",
            Self::Truncation => "truncation",
        }
    }
}

/// The ladder in canonical walk order.
pub const LADDER_ORDER: [LadderStage; 4] = [
    LadderStage::Masking,
    LadderStage::Distillation,
    LadderStage::Compression,
    LadderStage::Truncation,
];

/// One stage's explicit policy: when it may fire and how much it may
/// reclaim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageConfig {
    /// Eligibility: usage at or above this percent of the window.
    pub trigger_percent: u64,
    /// Maximum tokens this stage may reclaim in one walk.
    pub budget_tokens: u64,
}

/// One applied ladder stage's decision record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageDecision {
    /// The stage that applied.
    pub stage: LadderStage,
    /// Items the stage touched.
    pub items: usize,
    /// Tokens reclaimed by the stage.
    pub reclaimed_tokens: u64,
}

/// The full ladder policy (one entry per stage, in canonical order).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LadderConfig {
    /// `(stage, config)` pairs in walk order.
    pub stages: Vec<(LadderStage, StageConfig)>,
}

impl Default for LadderConfig {
    fn default() -> Self {
        Self {
            stages: LADDER_ORDER
                .map(|stage| {
                    let config = match stage {
                        LadderStage::Masking => StageConfig {
                            trigger_percent: 75,
                            budget_tokens: 1_500,
                        },
                        LadderStage::Distillation => StageConfig {
                            trigger_percent: 75,
                            budget_tokens: 3_000,
                        },
                        LadderStage::Compression => StageConfig {
                            trigger_percent: 75,
                            budget_tokens: 4_000,
                        },
                        LadderStage::Truncation => StageConfig {
                            trigger_percent: 75,
                            budget_tokens: u64::MAX,
                        },
                    };
                    (stage, config)
                })
                .to_vec(),
        }
    }
}

impl LadderConfig {
    /// The policy of `stage`.
    #[must_use]
    pub fn stage(&self, stage: LadderStage) -> Option<&StageConfig> {
        self.stages
            .iter()
            .find(|(candidate, _)| *candidate == stage)
            .map(|(_, config)| config)
    }

    /// Mutable policy of `stage` (config tuning).
    pub fn stage_mut(&mut self, stage: LadderStage) -> Option<&mut StageConfig> {
        self.stages
            .iter_mut()
            .find(|(candidate, _)| *candidate == stage)
            .map(|(_, config)| config)
    }
}

/// Tool-result pruning budgets (the Grok `[compaction.pruning]` shape):
/// how much of a large result the soft-trim stage keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PruningConfig {
    /// Head bytes an excerpt keeps.
    pub soft_head_bytes: usize,
    /// Tail bytes an excerpt keeps.
    pub soft_tail_bytes: usize,
}

impl Default for PruningConfig {
    fn default() -> Self {
        Self {
            soft_head_bytes: 128,
            soft_tail_bytes: 128,
        }
    }
}

/// Soft-trim `text` to a head/tail excerpt with an explicit omission
/// marker. Bodies too small to pay for the marker return verbatim.
/// Byte slicing is char-boundary safe (multibyte payloads never split).
#[must_use]
pub fn soft_trim(text: &str, head_bytes: usize, tail_bytes: usize) -> String {
    let keep = head_bytes + tail_bytes;
    if text.len() <= keep + 64 {
        return text.to_owned();
    }
    let head = floor_boundary(text, head_bytes);
    let tail_start = ceil_boundary(text, text.len().saturating_sub(tail_bytes));
    let omitted = tail_start - head;
    format!(
        "{}[trimmed {omitted} bytes]{}",
        &text[..head],
        &text[tail_start..]
    )
}

fn floor_boundary(text: &str, mut index: usize) -> usize {
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn ceil_boundary(text: &str, mut index: usize) -> usize {
    while index < text.len() && !text.is_char_boundary(index) {
        index += 1;
    }
    index
}
