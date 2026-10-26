//! Output rendering for both modes. `--output-json` (the eval adapter
//! contract) emits one JSON object per line: event frames as
//! `{"event": {"id": N, "event": …}}` (Recorded frames unwrap to the
//! core event; daemon-level events pass through), plus a final
//! `{"type": "turn_result", …}` for run/repl turns. Human mode renders
//! streaming text lines. Everything here is pure and unit-tested.
//!
//! Layout: the NDJSON contract in [`ndjson`], the human lines and ask
//! renderers in [`human`].

mod human;
mod ndjson;

pub use human::human;
pub use ndjson::{frame_envelope, ndjson_line, outcome_ok, turn_result};
