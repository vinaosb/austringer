//! Typed cassette errors. Strict replay treats any deviation — unused
//! interactions, wrong-order requests, dry cassette, corrupt file — as a hard
//! error with a message that names the cause.

use std::path::PathBuf;

/// Every way cassette recording or (strict) replay can fail.
#[derive(Debug, thiserror::Error)]
pub enum CassetteError {
    /// The cassette file exists but does not deserialize.
    #[error("corrupted cassette at {path}: {source}")]
    Corrupt {
        /// Path of the unreadable cassette.
        path: PathBuf,
        /// Underlying JSON error (includes line/column).
        source: serde_json::Error,
    },
    /// The cassette could not be serialized.
    #[error("cassette serialization failed for {path}: {source}")]
    Serialize {
        /// Path of the file being written.
        path: PathBuf,
        /// Underlying JSON error.
        source: serde_json::Error,
    },
    /// Filesystem error while reading or writing a cassette.
    #[error("cassette I/O error at {path}: {source}")]
    Io {
        /// Path of the offending file.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },
    /// The next incoming request does not match the next recorded interaction.
    /// Descriptors are boxed to keep the error type small on the happy path.
    #[error(
        "cassette '{name}' replay mismatch at interaction {index}: expected \
         model={e_model:?} prompt={e_prompt:?}, got model={g_model:?} prompt={g_prompt:?}",
        e_model = expected.model,
        e_prompt = expected.prompt,
        g_model = got.model,
        g_prompt = got.prompt
    )]
    Mismatch {
        /// Cassette name.
        name: String,
        /// Zero-based index of the expected interaction.
        index: usize,
        /// The recorded request that was expected next.
        expected: Box<super::record::RecordedRequest>,
        /// The request that actually arrived.
        got: Box<crate::provider::ProviderRequest>,
    },
    /// The cassette has no interactions left for an incoming request.
    #[error(
        "cassette '{name}' exhausted: no interaction left for model={model:?} prompt={prompt:?}"
    )]
    Exhausted {
        /// Cassette name.
        name: String,
        /// Offending request's model.
        model: String,
        /// Offending request's prompt.
        prompt: String,
    },
    /// Strict replay finished with recorded interactions never consumed.
    #[error(
        "cassette '{name}' strict replay violated: {count} unused interaction(s) remain \
         (every recorded interaction must be consumed)"
    )]
    UnusedInteractions {
        /// Cassette name.
        name: String,
        /// How many recorded interactions were never replayed.
        count: usize,
    },
}
