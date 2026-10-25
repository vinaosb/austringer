//! Executor type vocabulary (plan M6): the three honest tiers, the
//! producer disclosure every artifact carries, the request/output
//! envelopes dispatch speaks, and the disclosure-enforced `Produced`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::descriptor::Operation;
use crate::diff::RoundTripVerdict;
use crate::extract::{ExtractBudgets, Extracted};
use crate::generate::GenerationSpec;
use crate::probe::ProbeResult;

/// The execution tier an adapter runs in. The tier decides the sandbox
/// posture, the disclosure, and the permission class of an operation
/// (M31 mirrors it for the engine tools).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterTier {
    /// In-process pure Rust (the feature-gated codec families inside
    /// austringer-media). Default trust, default binary-size budget.
    Pure,
    /// Sandboxed external tool via the M15 executor: opt-in, disabled by
    /// default, SHA-pinned, redacted env, kill-tree timeout.
    Subprocess,
    /// Interface-2 WASM media adapter (declared at M6 - spec + manifest
    /// validation only; host-fn implementations land post-v0.2).
    Wasm,
}

/// What produced an artifact. EVERY extract/generate/verify result
/// carries one (standing constraint 4: honesty and producer
/// disclosure); M9's `MediaResult` events and M31's tool results
/// serialize it next to the artifact reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProducerInfo {
    /// The tier that executed.
    pub tier: AdapterTier,
    /// The named engine (`austringer-media::probe`, `pandoc`, ...).
    pub engine: String,
    /// The engine's version (crate version for pure adapters, pinned
    /// tool version for the subprocess tier).
    pub version: String,
    /// The executed command line, for tiers that spawn one.
    pub command: Option<String>,
}

/// A successful dispatch result: the artifact plus its disclosure.
///
/// A produced-without-producer value is UNREPRESENTABLE outside this
/// module: both fields are private, there is no `Default`, and the only
/// constructor is [`Produced::new`], which takes the disclosure as a
/// required argument. Compile-level proof (fails with E0451, private
/// field - run via `cargo test -p austringer-media --doc`):
///
/// ```compile_fail
/// use austringer_media::executor::{AdapterTier, OpOutput, Produced, ProducerInfo};
///
/// let producer = ProducerInfo {
///     tier: AdapterTier::Pure,
///     engine: "e".into(),
///     version: "0".into(),
///     command: None,
/// };
/// // error[E0451]: field `inner` of struct `Produced` is private
/// let _ = Produced {
///     inner: OpOutput::Probe(austringer_media::probe::probe(b"", None)),
///     producer,
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Produced {
    inner: OpOutput,
    producer: ProducerInfo,
}

impl Produced {
    /// The only constructor: an artifact and its disclosure, together or
    /// not at all.
    #[must_use]
    pub fn new(inner: OpOutput, producer: ProducerInfo) -> Self {
        Self { inner, producer }
    }

    /// The artifact.
    #[must_use]
    pub fn inner(&self) -> &OpOutput {
        &self.inner
    }

    /// The disclosure for [`Produced::inner`].
    #[must_use]
    pub fn producer(&self) -> &ProducerInfo {
        &self.producer
    }
}

/// The output of one dispatched operation (the artifact `Produced`
/// wraps).
#[derive(Debug, Clone, PartialEq)]
pub enum OpOutput {
    /// Typed identification (M2 sniffing; unknown is a typed variant).
    Probe(ProbeResult),
    /// Canonical tree + bounded text projection + thumbnails.
    Extract(Extracted),
    /// Generated document bytes (content-addressed by the caller on
    /// storage; bytes never ride events or frames).
    Generate(Vec<u8>),
    /// Round-trip verdict (M5 diff engine).
    Verify(RoundTripVerdict),
}

/// One dispatched operation's typed input.
#[derive(Debug, Clone, Copy)]
pub enum OpRequest<'a> {
    /// Identify bytes (with the name tie-break hint).
    Probe {
        /// Bytes to sniff.
        bytes: &'a [u8],
        /// File name or bare extension, Likely-confidence tie-breaks only.
        name_hint: Option<&'a str>,
    },
    /// Structurally extract, under caller-provided safety budgets.
    Extract {
        /// Bytes to extract.
        bytes: &'a [u8],
        /// Wall-clock deadline + total-size cap (typed ExtractTooLarge on
        /// breach - never hangs, never allocates unbounded).
        budgets: ExtractBudgets,
    },
    /// Generate from a spec (DETERMINISM CONTRACT applies).
    Generate {
        /// The format + params spec.
        spec: &'a GenerationSpec,
    },
    /// Verify produced bytes against an INDEPENDENT expected tree.
    Verify {
        /// Expected tree from a separate spec-to-tree projection (M5
        /// provenance rule), never from the adapter's own extractor.
        expected: &'a Value,
        /// The produced bytes under test.
        bytes: &'a [u8],
    },
}

impl OpRequest<'_> {
    /// Which operation this request dispatches.
    #[must_use]
    pub fn operation(&self) -> Operation {
        match self {
            Self::Probe { .. } => Operation::Probe,
            Self::Extract { .. } => Operation::Extract,
            Self::Generate { .. } => Operation::Generate,
            Self::Verify { .. } => Operation::Verify,
        }
    }
}
