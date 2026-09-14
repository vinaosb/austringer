/**
 * Typed wire codecs for the harness daemon client protocol v1 — a
 * line-for-line mirror of `harness-daemon/src/protocol.rs` (serde
 * shapes, snake_case tags, bounds). Frames are validated before they
 * leave the editor and rejected before they are parsed here, exactly
 * like the daemon side: client input is untrusted in both directions.
 */

export const PROTOCOL_VERSION = 1;
export const MAX_FRAME_BYTES = 1024 * 1024;
export const MAX_TEXT_FIELD_BYTES = 64 * 1024;
export const MAX_ID_BYTES = 128;

/** Typed codec failure (mirrors the daemon's `ProtocolError`). */
export type ProtocolError =
    | { kind: "oversized" }
    | { kind: "malformed"; detail: string }
    | { kind: "out_of_bounds"; field: string };

export type Result<T> = { ok: true; value: T } | { ok: false; error: ProtocolError };

export type Answer = "allow_once" | "allow_session" | "deny";

export type SessionCommand =
    | { type: "submit"; text: string }
    | { type: "steer"; text: string }
    | { type: "cancel" };

export type ClientMsg =
    | { type: "hello"; version: number; token: string; client_id: string; session: string | null }
    | { type: "command"; command: SessionCommand }
    | { type: "permission_answer"; ask_id: number; answer: Answer }
    | { type: "permission_cancel"; ask_id: number }
    | { type: "ping" };

export type ContentBlock =
    | { type: "text"; text: string }
    | { type: "image"; mime: string; width: number; height: number; media: unknown };

export type EngineEvent =
    | { type: "user"; blocks: ContentBlock[] }
    | { type: "assistant"; blocks: ContentBlock[] }
    | { type: "tool_call"; call_id: string; tool: string; arguments: unknown }
    | { type: "tool_result"; call_id: string; blocks: ContentBlock[] }
    | { type: "system"; text: string }
    | { type: "condensation"; summary: string; through_seq: number }
    | { type: "snapshot"; label: string };

export type SessionEvent =
    | { type: "recorded"; seq: number; event: EngineEvent }
    | { type: "turn_started"; turn: number }
    | { type: "turn_finished"; turn: number; outcome: string }
    | { type: "diff"; path: string; before_hash: string; after_hash: string; hunks: string[] }
    | { type: "notification"; kind: string; message: string }
    | { type: "worker_claimed"; queue: string; worker: string }
    | { type: "worker_healed"; queue: string };

export type Frame = { id: number; event: SessionEvent };

export type AskPayload = {
    ask_id: number;
    tool: string;
    target: string | null;
    reason: string;
    deadline_ms: number;
};

/** Externally tagged on the wire: `{"allowed":{"by":..}}` or `"timeout"`. */
export type Resolution =
    | { allowed: { by: string } }
    | { denied: { by: string } }
    | { cancelled: { by: string } }
    | { timeout: true };

export type ErrorCode =
    | "protocol_version"
    | "auth"
    | "oversized_frame"
    | "malformed_frame"
    | "unknown_session"
    | "too_many_clients"
    | "idle_timeout"
    | "internal";

export type ServerMsg =
    | { type: "hello_ack"; version: number; last_event_id: number }
    | { type: "event"; frame: Frame }
    | { type: "permission_ask"; ask: AskPayload }
    | { type: "permission_resolved"; ask_id: number; resolution: Resolution }
    | { type: "state_resync_required"; last_event_id: number }
    | { type: "snapshot"; frames: Frame[] }
    | { type: "error"; code: ErrorCode; message: string }
    | { type: "pong" }
    | { type: "bye"; reason: string };

