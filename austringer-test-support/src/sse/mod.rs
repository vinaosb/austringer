//! SSE support: event type + `ev_*` constructors, script builder, byte stream
//! with drop/close/delay injection, and a partial-frame-aware parser.

mod event;
mod parser;
mod script;
mod stream;

pub use event::{
    SseEvent, ev_done, ev_error, ev_message_start, ev_message_stop, ev_text_delta,
    ev_tool_call_delta, ev_usage,
};
pub use parser::SseParser;
pub use script::{SseScript, SseScriptBuilder, SseStep};
pub use stream::SseByteStream;
