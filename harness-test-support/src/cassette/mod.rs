//! Cassette recorder with strict replay and redaction (plan todo 2e).
//!
//! Record fake-provider conversations, persist them as deterministic JSON, and
//! replay them strictly: order-sensitive matching, death on unused
//! interactions, and clear aborts on corruption or mismatch.

mod error;
mod record;
mod recorder;
mod redact;
mod replay;
mod store;

pub use error::CassetteError;
pub use record::{
    Cassette, RecordedFailure, RecordedInteraction, RecordedRequest, RecordedResponse,
};
pub use recorder::CassetteRecorder;
pub use redact::RedactionFilter;
pub use replay::CassetteReplayer;
