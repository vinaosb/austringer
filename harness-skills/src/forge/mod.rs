//! Skill Forge (plan todo 31): the self-authoring half of the skills
//! system. The T30 loader consumes what this module produces; the forge
//! never bypasses its gates.
//!
//! # Trust architecture (binding clauses)
//!
//! Every path from "observed signal" to "routable SKILL.md" passes the
//! same three gates, structurally, with no bypass:
//!
//! 1. **Draft tier** — every proposal ([`crate::forge::distill`]) lands in
//!    a draft namespace on disk that the T30 loader never scans. Drafts
//!    are inert by construction until a human
//!    [`ApprovalDecision`](crate::forge::gates::ApprovalDecision).
//! 2. **Eval-before-activate** — approval is necessary but not
//!    sufficient: activation also requires the replay harness
//!    ([`crate::forge::eval`]) to pass the draft against a scripted
//!    replay of the originating session type. Failure blocks activation
//!    with a typed report.
//! 3. **Delta-only updates** — existing skills change only through
//!    [`SkillDelta`](crate::forge::delta::SkillDelta) section diffs with
//!    version checks (ACE §7 "context collapse" mitigation); there is no
//!    code path that writes a whole new body over an existing skill.
//!
//! # Hermes five-mechanism loop (research §27)
//!
//! Self-evolution is composed of five explicit stages, never one magic
//! step — see [`crate::forge::hermes`]. ALL of them produce *drafts*
//! gated by write approval; none activates anything by itself.
//!
//! # Untrusted-input posture
//!
//! Session trajectories, chat feedback, and marketplace skill bodies are
//! untrusted input to the forge: proposals derived from them are scanned
//! by the [`crate::forge::quarantine`] injection scanner, and
//! third-party skills pass content-hash scan → quarantine → ledger
//! before any activation (ClawHavoc lesson).

pub mod analytics;
pub mod delta;
pub mod distill;
pub mod document;
pub mod episodes;
pub mod eval;
pub mod gates;
pub mod hermes;
pub mod journal;
pub mod knowledge;
pub mod lifecycle;
pub mod prime;
pub mod quality;
pub mod quarantine;
pub mod sources;

/// Everything that can go wrong in the forge. Typed, never a panic,
/// never a stringly-typed retry.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ForgeError {
    /// A journal file exists but is not valid JSON / expected shape.
    #[error("journal file corrupt: {detail}")]
    JournalCorrupt {
        /// What was wrong with the file.
        detail: String,
    },
    /// A staged draft file exists but does not deserialize.
    #[error("draft file corrupt: {detail}")]
    DraftCorrupt {
        /// What was wrong with the file.
        detail: String,
    },
    /// No staged draft carries this id.
    #[error("draft {id:?} not found")]
    DraftNotFound {
        /// The requested draft id.
        id: String,
    },
    /// The draft carries scanner findings and can never be activated.
    #[error("draft {id:?} is inert: {findings} scanner finding(s) — flagged drafts never activate")]
    DraftInert {
        /// The draft id.
        id: String,
        /// Number of findings.
        findings: usize,
    },
    /// A live skill exists; only delta updates may touch it.
    #[error(
        "skill {skill:?} already exists: updates require a delta + revision, never a silent overwrite"
    )]
    OverwriteRequiresDelta {
        /// The skill name.
        skill: String,
    },
    /// The draft was computed against a different live version.
    #[error("stale draft for {skill:?}: based on {expected:?} but live is {actual:?}")]
    StaleDraft {
        /// The skill name.
        skill: String,
        /// Version the draft was based on.
        expected: String,
        /// Version actually live.
        actual: String,
    },
    /// The distill model seam failed.
    #[error("distill model failed: {detail}")]
    DistillFailed {
        /// Underlying failure.
        detail: String,
    },
    /// Quarantine was asked to release unknown content.
    #[error("quarantine has no entry {hash:?}")]
    QuarantineUnknown {
        /// The unknown content hash.
        hash: String,
    },
    /// Quarantine refused release: intake found scanner findings.
    #[error("quarantine holds {hash:?}: {findings} scanner finding(s)")]
    QuarantineHeld {
        /// The held content hash.
        hash: String,
        /// Number of findings.
        findings: usize,
    },
    /// A refinement was attempted without evidence.
    #[error("refinement evidence required")]
    EvidenceRequired,
    /// No refinement carries this version.
    #[error("refinement {version} not found")]
    RefinementNotFound {
        /// The requested overlay version.
        version: u32,
    },
    /// No knowledge item carries this id.
    #[error("knowledge item {id:?} not found")]
    KnowledgeNotFound {
        /// The requested item id.
        id: String,
    },
    /// Promotion is one-directional; the item is already at the target
    /// scope.
    #[error("promotion allowed only Organization -> Enterprise; item {id:?} is already {scope}")]
    PromotionPath {
        /// The item id.
        id: String,
        /// The item's current scope.
        scope: String,
    },
    /// A source fetch seam failed.
    #[error("source fetch failed: {detail}")]
    SourceFetchFailed {
        /// Underlying failure.
        detail: String,
    },
    /// A filesystem operation failed.
    #[error("io: {detail}")]
    Io {
        /// Underlying failure description.
        detail: String,
    },
    /// A delta did not apply to the current sections.
    #[error(transparent)]
    Delta(#[from] crate::forge::delta::DeltaError),
}

impl ForgeError {
    /// Wraps any std::io error with context.
    pub fn io(context: &str, err: &std::io::Error) -> Self {
        Self::Io {
            detail: format!("{context}: {err}"),
        }
    }
}

/// Injected clock seam (lifecycle, journal capture, quarantine holds).
/// Production uses [`SystemClock`]; tests inject their own deterministic
/// clock to age skills without sleeping.
pub trait ForgeClock {
    /// Seconds since the Unix epoch.
    fn now_unix(&self) -> u64;
}

/// Wall-clock implementation of [`ForgeClock`].
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl ForgeClock for SystemClock {
    fn now_unix(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
    }
}
