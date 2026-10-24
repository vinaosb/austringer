/**
 * Typed wire codecs for the harness daemon client protocol v2 — a
 * line-for-line mirror of `harness-daemon/src/protocol.rs` (serde
 * shapes, snake_case tags, bounds). Frames are validated before they
 * leave the editor and rejected before they are parsed here, exactly
 * like the daemon side: client input is untrusted in both directions.
 */

/**
 * Wire protocol version (mirrors the daemon's PROTOCOL_VERSION = 2).
 * Rides the SSE URL as `version=` — the daemon serves unknown or absent
 * versions as v1 and WITHHOLDS v2 frames (usage/fallback/plan/team),
 * so every v2 frame type below must stay decodable or the client's
 * cursor stalls and gap-reconnects.
 */
export const PROTOCOL_VERSION = 2;
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

/** Model identity on the wire (mirrors `harness_providers::identity`). */
export type ModelIdentity = {
    provider: string;
    model: string;
};

/** A model-fallback ask (protocol v2; the ask-first identity contract). */
export type FallbackAskPayload = {
    ask_id: number;
    current: ModelIdentity;
    proposed: ModelIdentity;
    reason: string;
    deadline_ms: number;
};

/** A plan-approval decision (approve/reject; edit carries a new artifact). */
export type PlanDecision = "approve" | "reject" | { edit: { artifact: string } };

/** How one plan ask resolved (externally tagged; "timeout" is a bare string). */
export type PlanOutcome =
    | { approved: { by: string } }
    | { rejected: { by: string } }
    | { edited: { by: string; artifact: string } }
    | { cancelled: { by: string } }
    | { timeout: true };

/** A plan-approval ask (protocol v2). */
export type PlanAskPayload = {
    ask_id: number;
    plan_id: number;
    revision: number;
    artifact: string;
    deadline_ms: number;
};

export type SessionCommand =
    | { type: "submit"; text: string }
    | { type: "steer"; text: string }
    | { type: "cancel" };

export type ClientMsg =
    | { type: "hello"; version: number; token: string; client_id: string; session: string | null }
    | { type: "command"; command: SessionCommand }
    | { type: "permission_answer"; ask_id: number; answer: Answer }
    | { type: "permission_cancel"; ask_id: number }
    | { type: "fallback_answer"; ask: number; chosen: ModelIdentity }
    | { type: "plan_mode"; on: boolean }
    | { type: "rewind"; steps: number }
    | { type: "plan_answer"; ask: number; decision: PlanDecision }
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
    | { type: "worker_healed"; queue: string }
    | { type: "child_started"; child: string; task: string; role: string; model: string }
    | { type: "child_progress"; child: string; phase: string; detail: string }
    | { type: "child_finished"; child: string; task: string; state: string; detail: string }
    | { type: "cost_warning"; consumed_micros: number; cap_micros: number }
    | { type: "cost_stop"; consumed_micros: number; cap_micros: number }
    | { type: "team_budget"; spawned: number; tokens_spent: number; fan_out_cap: number; token_budget: number }
    | { type: "team_budget_exhausted"; spent: number; budget: number }
    | { type: "plan_mode_entered"; plan: number }
    | { type: "plan_mode_exited"; plan: number }
    | { type: "plan_blocked"; plan: number; tool: string; target: string | null }
    | { type: "plan_updated"; plan: number; revision: number }
    | { type: "plan_approved"; plan: number }
    | { type: "plan_rejected"; plan: number };

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
    | { type: "fallback_ask"; ask: FallbackAskPayload }
    | { type: "fallback_resolved"; ask_id: number; chosen: ModelIdentity }
    | { type: "plan_ask"; ask: PlanAskPayload }
    | { type: "plan_resolved"; ask_id: number; plan_id: number; outcome: PlanOutcome }
    | { type: "state_resync_required"; last_event_id: number }
    | { type: "snapshot"; frames: Frame[] }
    | { type: "error"; code: ErrorCode; message: string }
    | { type: "pong" }
    | { type: "bye"; reason: string };

