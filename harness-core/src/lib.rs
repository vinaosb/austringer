//! Core event model and append-only session log for the harness.
//!
//! Scope (plan todo 4): a typed conversation-event taxonomy with a
//! first-class LLM-visible/internal separation, multimodal content blocks
//! whose image payloads live out-of-line under `<session>/media/`, and a
//! crash-tolerant append-only JSONL log with an explicit fsync policy and a
//! degrade-to-read-only failure contract. A session is a directory
//! containing `events.jsonl` and `media/`; the full session store is a later
//! todo.
//!
//! Serialization is stable by construction: struct fields serialize in
//! declaration order, dynamic maps serialize with sorted keys (serde_json's
//! default BTreeMap ordering — the `preserve_order` feature must never be
//! enabled anywhere in the workspace), events embed no OS paths (image
//! payloads are referenced by content hash), and no timestamp is injected
//! into the serialized form.
#![deny(missing_docs)]

pub mod event;
pub mod log;
