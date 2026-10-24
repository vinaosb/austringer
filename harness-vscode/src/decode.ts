/**
 * Untrusted-input decoders for daemon server messages: primitive type
 * guards first, then structural decode of every event shape. Nothing
 * here trusts the wire — unknown tags and wrong field types decode to
 * null so callers surface typed `malformed` failures.
 *
 * allow: SIZE_OK — a mechanical extraction table mirroring the
 * daemon's protocol/events enums one arm per wire variant (no logic
 * to isolate); splitting it would break the mirror-per-source-file
 * correspondence.
 */

import type {
    AskPayload,
    ContentBlock,
    EngineEvent,
    FallbackAskPayload,
    Frame,
    ModelIdentity,
    PlanAskPayload,
    PlanOutcome,
    Resolution,
    SessionEvent,
} from "./protocol.ts";

export function asObject(value: unknown): Record<string, unknown> | null {
    return typeof value === "object" && value !== null && !Array.isArray(value)
        ? (value as Record<string, unknown>)
        : null;
}

export function asString(value: unknown): string | null {
    return typeof value === "string" ? value : null;
}

export function asNumber(value: unknown): number | null {
    return typeof value === "number" ? value : null;
}

export function asArray(value: unknown): unknown[] | null {
    return Array.isArray(value) ? value : null;
}

function asContentBlocks(value: unknown): ContentBlock[] | null {
    const arr = asArray(value);
    if (arr === null) {
        return null;
    }
    const blocks: ContentBlock[] = [];
    for (const raw of arr) {
        const obj = asObject(raw);
        const type = obj === null ? null : asString(obj["type"]);
        if (obj === null || type === "text") {
            const text = obj === null ? null : asString(obj["text"]);
            if (text === null) {
                return null;
            }
            blocks.push({ type: "text", text });
        } else if (type === "image") {
            const mime = asString(obj["mime"]);
            if (mime === null) {
                return null;
            }
            blocks.push({
                type: "image",
                mime,
                width: asNumber(obj["width"]) ?? 0,
                height: asNumber(obj["height"]) ?? 0,
                media: obj["media"],
            });
        } else {
            return null;
        }
    }
    return blocks;
}

export function decodeEngineEvent(value: unknown): EngineEvent | null {
    const obj = asObject(value);
    const type = obj === null ? null : asString(obj["type"]);
    if (obj === null || type === null) {
        return null;
    }
    switch (type) {
        case "user":
        case "assistant": {
            const blocks = asContentBlocks(obj["blocks"]);
            return blocks === null ? null : { type, blocks };
        }
        case "tool_call": {
            const callId = asString(obj["call_id"]);
            const tool = asString(obj["tool"]);
            if (callId === null || tool === null || !("arguments" in obj)) {
                return null;
            }
            return { type, call_id: callId, tool, arguments: obj["arguments"] };
        }
        case "tool_result": {
            const callId = asString(obj["call_id"]);
            const blocks = asContentBlocks(obj["blocks"]);
            return callId === null || blocks === null
                ? null
                : { type, call_id: callId, blocks };
        }
        case "system": {
            const text = asString(obj["text"]);
            return text === null ? null : { type, text };
        }
        case "condensation": {
            const summary = asString(obj["summary"]);
            const through = asNumber(obj["through_seq"]);
            return summary === null || through === null
                ? null
                : { type, summary, through_seq: through };
        }
        case "snapshot": {
            const label = asString(obj["label"]);
            return label === null ? null : { type, label };
        }
        default:
            return null;
    }
}

export function decodeSessionEvent(value: unknown): SessionEvent | null {
    const obj = asObject(value);
    const type = obj === null ? null : asString(obj["type"]);
    if (obj === null || type === null) {
        return null;
    }
    switch (type) {
        case "recorded": {
            const seq = asNumber(obj["seq"]);
            const event = decodeEngineEvent(obj["event"]);
            return seq === null || event === null ? null : { type, seq, event };
        }
        case "turn_started": {
            const turn = asNumber(obj["turn"]);
            return turn === null ? null : { type, turn };
        }
        case "turn_finished": {
            const turn = asNumber(obj["turn"]);
            const outcome = asString(obj["outcome"]);
            return turn === null || outcome === null ? null : { type, turn, outcome };
        }
        case "diff": {
            const path = asString(obj["path"]);
            const before = asString(obj["before_hash"]);
            const after = asString(obj["after_hash"]);
            const hunks = asArray(obj["hunks"]);
            if (path === null || before === null || after === null || hunks === null) {
                return null;
            }
            const strings: string[] = [];
            for (const hunk of hunks) {
                const text = asString(hunk);
                if (text === null) {
                    return null;
                }
                strings.push(text);
            }
            return { type, path, before_hash: before, after_hash: after, hunks: strings };
        }
        case "notification": {
            const kind = asString(obj["kind"]);
            const message = asString(obj["message"]);
            return kind === null || message === null ? null : { type, kind, message };
        }
        case "worker_claimed": {
            const queue = asString(obj["queue"]);
            const worker = asString(obj["worker"]);
            return queue === null || worker === null ? null : { type, queue, worker };
        }
        case "worker_healed": {
            const queue = asString(obj["queue"]);
            return queue === null ? null : { type, queue };
        }
        case "child_started": {
            const child = asString(obj["child"]);
            const task = asString(obj["task"]);
            const role = asString(obj["role"]);
            const model = asString(obj["model"]);
            return child === null || task === null || role === null || model === null
                ? null
                : { type, child, task, role, model };
        }
        case "child_progress": {
            const child = asString(obj["child"]);
            const phase = asString(obj["phase"]);
            const detail = asString(obj["detail"]);
            return child === null || phase === null || detail === null
                ? null
                : { type, child, phase, detail };
        }
        case "child_finished": {
            const child = asString(obj["child"]);
            const task = asString(obj["task"]);
            const state = asString(obj["state"]);
            const detail = asString(obj["detail"]);
            return child === null || task === null || state === null || detail === null
                ? null
                : { type, child, task, state, detail };
        }
        case "cost_warning":
        case "cost_stop": {
            const consumed = asNumber(obj["consumed_micros"]);
            const cap = asNumber(obj["cap_micros"]);
            return consumed === null || cap === null
                ? null
                : { type, consumed_micros: consumed, cap_micros: cap };
        }
        case "team_budget": {
            const spawned = asNumber(obj["spawned"]);
            const tokensSpent = asNumber(obj["tokens_spent"]);
            const fanOutCap = asNumber(obj["fan_out_cap"]);
            const tokenBudget = asNumber(obj["token_budget"]);
            return spawned === null || tokensSpent === null || fanOutCap === null || tokenBudget === null
                ? null
                : { type, spawned, tokens_spent: tokensSpent, fan_out_cap: fanOutCap, token_budget: tokenBudget };
        }
        case "team_budget_exhausted": {
            const spent = asNumber(obj["spent"]);
            const budget = asNumber(obj["budget"]);
            return spent === null || budget === null ? null : { type, spent, budget };
        }
        case "plan_mode_entered":
        case "plan_mode_exited":
        case "plan_approved":
        case "plan_rejected": {
            const plan = asNumber(obj["plan"]);
            return plan === null ? null : { type, plan };
        }
        case "plan_blocked": {
            const plan = asNumber(obj["plan"]);
            const tool = asString(obj["tool"]);
            const targetRaw = obj["target"];
            const target = targetRaw === undefined || targetRaw === null ? null : asString(targetRaw);
            if (plan === null || tool === null || target === null && targetRaw !== null) {
                return null;
            }
            return { type, plan, tool, target };
        }
        case "plan_updated": {
            const plan = asNumber(obj["plan"]);
            const revision = asNumber(obj["revision"]);
            return plan === null || revision === null ? null : { type, plan, revision };
        }
        default:
            return null;
    }
}

export function decodeFrame(value: unknown): Frame | null {
    const obj = asObject(value);
    if (obj === null) {
        return null;
    }
    const id = asNumber(obj["id"]);
    const event = decodeSessionEvent(obj["event"]);
    return id === null || event === null ? null : { id, event };
}

export function decodeResolution(value: unknown): Resolution | null {
    if (value === "timeout") {
        return { timeout: true };
    }
    const obj = asObject(value);
    if (obj === null) {
        return null;
    }
    for (const key of ["allowed", "denied", "cancelled"] as const) {
        const payload = asObject(obj[key]);
        const by = payload === null ? null : asString(payload["by"]);
        if (payload !== null && by !== null) {
            return { [key]: { by } } as Resolution;
        }
    }
    return null;
}

export function decodeAsk(value: unknown): AskPayload | null {
    const obj = asObject(value);
    if (obj === null) {
        return null;
    }
    const askId = asNumber(obj["ask_id"]);
    const tool = asString(obj["tool"]);
    const reason = asString(obj["reason"]);
    const deadline = asNumber(obj["deadline_ms"]);
    const targetRaw = obj["target"];
    const target = targetRaw === undefined || targetRaw === null ? null : asString(targetRaw);
    if (askId === null || tool === null || reason === null || deadline === null || target === null && targetRaw !== null) {
        return null;
    }
    return { ask_id: askId, tool, target, reason, deadline_ms: deadline };
}

export function decodeModelIdentity(value: unknown): ModelIdentity | null {
    const obj = asObject(value);
    if (obj === null) {
        return null;
    }
    const provider = asString(obj["provider"]);
    const model = asString(obj["model"]);
    return provider === null || model === null ? null : { provider, model };
}

export function decodeFallbackAsk(value: unknown): FallbackAskPayload | null {
    const obj = asObject(value);
    if (obj === null) {
        return null;
    }
    const askId = asNumber(obj["ask_id"]);
    const current = decodeModelIdentity(obj["current"]);
    const proposed = decodeModelIdentity(obj["proposed"]);
    const reason = asString(obj["reason"]);
    const deadline = asNumber(obj["deadline_ms"]);
    return askId === null || current === null || proposed === null || reason === null || deadline === null
        ? null
        : { ask_id: askId, current, proposed, reason, deadline_ms: deadline };
}

export function decodePlanOutcome(value: unknown): PlanOutcome | null {
    if (value === "timeout") {
        return { timeout: true };
    }
    const obj = asObject(value);
    if (obj === null) {
        return null;
    }
    const approved = asObject(obj["approved"]);
    const approvedBy = approved === null ? null : asString(approved["by"]);
    if (approvedBy !== null) {
        return { approved: { by: approvedBy } };
    }
    const rejected = asObject(obj["rejected"]);
    const rejectedBy = rejected === null ? null : asString(rejected["by"]);
    if (rejectedBy !== null) {
        return { rejected: { by: rejectedBy } };
    }
    const edited = asObject(obj["edited"]);
    const editedBy = edited === null ? null : asString(edited["by"]);
    const artifact = edited === null ? null : asString(edited["artifact"]);
    if (editedBy !== null && artifact !== null) {
        return { edited: { by: editedBy, artifact } };
    }
    const cancelled = asObject(obj["cancelled"]);
    const cancelledBy = cancelled === null ? null : asString(cancelled["by"]);
    return cancelledBy === null ? null : { cancelled: { by: cancelledBy } };
}

export function decodePlanAsk(value: unknown): PlanAskPayload | null {
    const obj = asObject(value);
    if (obj === null) {
        return null;
    }
    const askId = asNumber(obj["ask_id"]);
    const planId = asNumber(obj["plan_id"]);
    const revision = asNumber(obj["revision"]);
    const artifact = asString(obj["artifact"]);
    const deadline = asNumber(obj["deadline_ms"]);
    return askId === null || planId === null || revision === null || artifact === null || deadline === null
        ? null
        : { ask_id: askId, plan_id: planId, revision, artifact, deadline_ms: deadline };
}
