//! Prime-agent pattern (research §27): continual-harness CRUD
//! (/refine-style evidence-backed, versioned overlays) over an
//! IMMUTABLE base prompt. [`BasePrompt`] exposes no mutation —
//! improvements are additive, revertable overlays; the base can never
//! be rewritten in place.

use crate::forge::ForgeError;

/// An immutable base prompt. Constructed once; `as_str` is the only
/// accessor — there is deliberately no setter or `&mut` surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasePrompt(String);

impl BasePrompt {
    /// Freezes `text` as the base prompt.
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// The frozen text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One evidence-backed refinement overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefinementOverlay {
    /// Monotonic overlay version (1, 2, …).
    pub version: u32,
    /// The evidence justifying the refinement (required, non-empty).
    pub evidence: String,
    /// What the refinement changes.
    pub note: String,
}

/// The continual austringer: immutable base + ordered overlays with CRUD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinualHarness {
    base: BasePrompt,
    overlays: Vec<RefinementOverlay>,
}

impl ContinualHarness {
    /// A austringer over an immutable base.
    pub fn new(base: BasePrompt) -> Self {
        Self {
            base,
            overlays: Vec::new(),
        }
    }

    /// The frozen base prompt.
    pub fn base(&self) -> &BasePrompt {
        &self.base
    }

    /// All overlays, oldest first.
    pub fn overlays(&self) -> &[RefinementOverlay] {
        &self.overlays
    }

    /// Appends a versioned refinement. Evidence is mandatory: a
    /// refinement without evidence is refused, not defaulted.
    pub fn refine(&mut self, evidence: &str, note: &str) -> Result<u32, ForgeError> {
        if evidence.trim().is_empty() {
            return Err(ForgeError::EvidenceRequired);
        }
        let version = self.overlays.last().map_or(1, |o| o.version + 1);
        self.overlays.push(RefinementOverlay {
            version,
            evidence: evidence.to_string(),
            note: note.to_string(),
        });
        Ok(version)
    }

    /// Removes one overlay by version (refinements are revertable;
    /// the base never is).
    pub fn revert(&mut self, version: u32) -> Result<(), ForgeError> {
        let before = self.overlays.len();
        self.overlays.retain(|o| o.version != version);
        if self.overlays.len() == before {
            return Err(ForgeError::RefinementNotFound { version });
        }
        Ok(())
    }

    /// Renders base + overlays (the base text is never reflowed).
    pub fn render(&self) -> String {
        let mut out = self.base.as_str().to_string();
        for overlay in &self.overlays {
            out.push_str(&format!(
                "\n[refine v{}] {} (evidence: {})",
                overlay.version, overlay.note, overlay.evidence
            ));
        }
        out
    }
}
