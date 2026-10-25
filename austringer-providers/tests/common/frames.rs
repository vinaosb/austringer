//! Vendor-shaped SSE frame builders over the T2 SseEvent DSL.
//!
//! Chat-completions/Gemini frames carry an inert event: line that real
//! endpoints omit - the codecs dispatch on the data payload and ignore the
//! event name, which a dedicated test pins.

#![allow(dead_code)]

use austringer_test_support::sse::{SseEvent, SseScript};
use serde_json::{Value, json};
// ---------------------------------------------------------------------------
// Vendor frame builders (all Anthropic-shaped frames carry real wire names).

pub fn an_message_start(model: &str, input: u64, cache_read: u64, cache_write: u64) -> SseEvent {
    SseEvent::new(
        "message_start",
        json!({
            "type": "message_start",
            "message": {
                "model": model,
                "usage": {
                    "input_tokens": input,
                    "cache_read_input_tokens": cache_read,
                    "cache_creation_input_tokens": cache_write
                }
            }
        }),
    )
}

pub fn an_text_delta(text: &str) -> SseEvent {
    SseEvent::new(
        "content_block_delta",
        json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": text } }),
    )
}

pub fn an_tool_block_start(index: u64, call_id: &str, name: &str) -> SseEvent {
    SseEvent::new(
        "content_block_start",
        json!({
            "type": "content_block_start",
            "index": index,
            "content_block": { "type": "tool_use", "id": call_id, "name": name, "input": {} }
        }),
    )
}

pub fn an_tool_args_delta(index: u64, fragment: &str) -> SseEvent {
    SseEvent::new(
        "content_block_delta",
        json!({
            "type": "content_block_delta",
            "index": index,
            "delta": { "type": "input_json_delta", "partial_json": fragment }
        }),
    )
}

pub fn an_message_delta(output: u64, stop_reason: &str) -> SseEvent {
    SseEvent::new(
        "message_delta",
        json!({
            "type": "message_delta",
            "delta": { "stop_reason": stop_reason, "stop_sequence": Value::Null },
            "usage": { "output_tokens": output }
        }),
    )
}

pub fn an_ping() -> SseEvent {
    SseEvent::new("ping", json!({ "type": "ping" }))
}

pub fn an_error(kind: &str, message: &str) -> SseEvent {
    SseEvent::new(
        "error",
        json!({ "type": "error", "error": { "type": kind, "message": message } }),
    )
}

/// The canonical happy-path Anthropic reply: two text deltas, one fragmented
/// tool call, usage with cache detail, `end_turn`.
pub fn anthropic_happy(model: &str) -> SseScript {
    austringer_test_support::sse::SseScriptBuilder::new()
        .event(an_message_start(model, 1_234, 5_000, 1_000))
        .event(an_text_delta("Let me check "))
        .event(an_text_delta("the todos."))
        .event(an_tool_block_start(1, "toolu_01", "execute_command"))
        .event(an_tool_args_delta(1, "{\"cmd\":"))
        .event(an_tool_args_delta(1, "\"grep -r TODO src/\"}"))
        .event(an_message_delta(42, "tool_use"))
        .event(an_ping())
        .event(austringer_test_support::sse::ev_message_stop())
        .build()
}

// --- chat-completions frames (data payload shape; the inert `event:` line
// --- is tolerated by the codec by design) ---

pub fn cc_text(model: &str, text: &str) -> SseEvent {
    SseEvent::new(
        "message",
        json!({ "model": model, "choices": [{ "index": 0, "delta": { "content": text }, "finish_reason": Value::Null }] }),
    )
}

pub fn cc_tool_start(model: &str, index: u64, call_id: &str, name: &str) -> SseEvent {
    SseEvent::new(
        "message",
        json!({
            "model": model,
            "choices": [{ "index": 0, "delta": { "tool_calls": [
                { "index": index, "id": call_id, "type": "function", "function": { "name": name, "arguments": "" } }
            ]}, "finish_reason": Value::Null }]
        }),
    )
}

pub fn cc_tool_args(model: &str, index: u64, fragment: &str) -> SseEvent {
    SseEvent::new(
        "message",
        json!({
            "model": model,
            "choices": [{ "index": 0, "delta": { "tool_calls": [
                { "index": index, "function": { "arguments": fragment } }
            ]}, "finish_reason": Value::Null }]
        }),
    )
}

pub fn cc_finish(model: &str, reason: &str) -> SseEvent {
    SseEvent::new(
        "message",
        json!({ "model": model, "choices": [{ "index": 0, "delta": {}, "finish_reason": reason }] }),
    )
}

pub fn cc_usage(model: &str, prompt: u64, completion: u64, cached: u64) -> SseEvent {
    SseEvent::new(
        "message",
        json!({
            "model": model,
            "choices": [],
            "usage": { "prompt_tokens": prompt, "completion_tokens": completion,
                       "prompt_tokens_details": { "cached_tokens": cached } }
        }),
    )
}

pub fn cc_done() -> SseEvent {
    SseEvent::new("message", Value::String("[DONE]".to_owned()))
}

/// Canonical chat-completions happy reply: text, fragmented tool call,
/// finish, trailing usage chunk, [DONE] — usage AFTER finish is the
/// deferred-finish contract this family must honor.
pub fn chat_happy(model: &str) -> SseScript {
    austringer_test_support::sse::SseScriptBuilder::new()
        .event(cc_text(model, "Checking "))
        .event(cc_text(model, "the plan."))
        .event(cc_tool_start(model, 0, "call_abc", "view_window"))
        .event(cc_tool_args(model, 0, "{\"path\":"))
        .event(cc_tool_args(model, 0, "\"src/lib.rs\"}"))
        .event(cc_finish(model, "tool_calls"))
        .event(cc_usage(model, 900, 33, 400))
        .event(cc_done())
        .build()
}

// --- Responses-API frames ---

pub fn rs_text(delta: &str) -> SseEvent {
    SseEvent::new("response.output_text.delta", json!({ "delta": delta }))
}

pub fn rs_item_added(call_id: &str, name: &str) -> SseEvent {
    SseEvent::new(
        "response.output_item.added",
        json!({ "item": { "type": "function_call", "id": call_id, "name": name, "arguments": "" } }),
    )
}

pub fn rs_args_delta(item_id: &str, fragment: &str) -> SseEvent {
    SseEvent::new(
        "response.function_call_arguments.delta",
        json!({ "item_id": item_id, "delta": fragment }),
    )
}

pub fn rs_completed(model: &str, input: u64, output: u64, cached: u64) -> SseEvent {
    SseEvent::new(
        "response.completed",
        json!({ "response": { "model": model, "usage": {
            "input_tokens": input, "output_tokens": output,
            "input_tokens_details": { "cached_tokens": cached } } } }),
    )
}

pub fn rs_failed(code: &str, message: &str) -> SseEvent {
    SseEvent::new(
        "response.failed",
        json!({ "response": { "error": { "code": code, "message": message } } }),
    )
}

pub fn rs_incomplete(reason: &str) -> SseEvent {
    SseEvent::new(
        "response.incomplete",
        json!({ "response": { "incomplete_details": { "reason": reason } } }),
    )
}

// --- Gemini frames ---

pub fn gm_text(text: &str) -> SseEvent {
    SseEvent::new(
        "message",
        json!({ "candidates": [{ "content": { "parts": [{ "text": text }] } }] }),
    )
}

pub fn gm_function_call(name: &str, args: Value) -> SseEvent {
    SseEvent::new(
        "message",
        json!({ "candidates": [{ "content": { "parts": [{ "functionCall": { "name": name, "args": args } }] } }] }),
    )
}

pub fn gm_finish(reason: &str) -> SseEvent {
    SseEvent::new(
        "message",
        json!({ "candidates": [{ "content": {}, "finishReason": reason }] }),
    )
}

pub fn gm_usage(prompt: u64, candidates: u64, cached: u64) -> SseEvent {
    SseEvent::new(
        "message",
        json!({ "usageMetadata": { "promptTokenCount": prompt, "candidatesTokenCount": candidates, "cachedContentTokenCount": cached } }),
    )
}

/// Canonical Gemini happy reply: two text parts, one function call, final
/// chunk with finishReason + usageMetadata together.
pub fn gemini_happy() -> SseScript {
    austringer_test_support::sse::SseScriptBuilder::new()
        .event(gm_text("Reading "))
        .event(gm_text("the index."))
        .event(gm_function_call("tool_search", json!({ "query": "edit" })))
        .event(SseEvent::new(
            "message",
            json!({
                "candidates": [{ "content": { "parts": [] }, "finishReason": "STOP" }],
                "usageMetadata": { "promptTokenCount": 777, "candidatesTokenCount": 55, "cachedContentTokenCount": 100 }
            }),
        ))
        .build()
}
