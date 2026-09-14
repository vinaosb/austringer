/**
 * Untrusted-input decoders for daemon server messages: primitive type
 * guards first, then structural decode of every event shape. Nothing
 * here trusts the wire — unknown tags and wrong field types decode to
 * null so callers surface typed `malformed` failures.
 */

import type {
    AskPayload,
    ContentBlock,
    EngineEvent,
    Frame,
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
