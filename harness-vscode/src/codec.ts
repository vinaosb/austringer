/**
 * Wire codecs: decode whole server messages (bound-checked before
 * parsing) and validate + encode outbound client messages. Mirrors
 * the daemon's own gates so bad input never leaves the editor and bad
 * output never surprises the daemon.
 */

import type { ClientMsg, ErrorCode, ProtocolError, Result, ServerMsg } from "./protocol.ts";
import { PROTOCOL_VERSION, MAX_FRAME_BYTES, MAX_ID_BYTES, MAX_TEXT_FIELD_BYTES } from "./protocol.ts";
import { asNumber, asObject, asArray, asString, decodeAsk, decodeFrame, decodeResolution } from "./decode.ts";

const malformed = (detail: string): { ok: false; error: ProtocolError } => ({
    ok: false,
    error: { kind: "malformed", detail },
});

const ERROR_CODES: readonly ErrorCode[] = [
    "protocol_version",
    "auth",
    "oversized_frame",
    "malformed_frame",
    "unknown_session",
    "too_many_clients",
    "idle_timeout",
    "internal",
];

/** Decode one server message; oversized input is rejected before parsing. */
export function decodeServerMessage(raw: string): Result<ServerMsg> {
    if (raw.length > MAX_FRAME_BYTES) {
        return { ok: false, error: { kind: "oversized" } };
    }
    let parsed: unknown;
    try {
        parsed = JSON.parse(raw);
    } catch (error) {
        return malformed(`json: ${String(error)}`);
    }
    const obj = asObject(parsed);
    const type = obj === null ? null : asString(obj["type"]);
    if (obj === null || type === null) {
        return malformed("missing type tag");
    }
    switch (type) {
        case "hello_ack": {
            const version = asNumber(obj["version"]);
            const lastId = asNumber(obj["last_event_id"]);
            return version === null || lastId === null
                ? malformed("hello_ack fields")
                : { ok: true, value: { type, version, last_event_id: lastId } };
        }
        case "event": {
            const frame = decodeFrame(obj["frame"]);
            return frame === null ? malformed("event frame") : { ok: true, value: { type, frame } };
        }
        case "permission_ask": {
            const ask = decodeAsk(obj["ask"]);
            return ask === null ? malformed("ask payload") : { ok: true, value: { type, ask } };
        }
        case "permission_resolved": {
            const askId = asNumber(obj["ask_id"]);
            const resolution = decodeResolution(obj["resolution"]);
            return askId === null || resolution === null
                ? malformed("permission_resolved fields")
                : { ok: true, value: { type, ask_id: askId, resolution } };
        }
        case "state_resync_required": {
            const lastId = asNumber(obj["last_event_id"]);
            return lastId === null
                ? malformed("resync fields")
                : { ok: true, value: { type, last_event_id: lastId } };
        }
        case "snapshot": {
            const framesRaw = asArray(obj["frames"]);
            if (framesRaw === null) {
                return malformed("snapshot frames");
            }
            const frames = [];
            for (const rawFrame of framesRaw) {
                const frame = decodeFrame(rawFrame);
                if (frame === null) {
                    return malformed("snapshot frame");
                }
                frames.push(frame);
            }
            return { ok: true, value: { type, frames } };
        }
        case "error": {
            const code = asString(obj["code"]);
            const message = asString(obj["message"]);
            if (code === null || message === null || !ERROR_CODES.includes(code as ErrorCode)) {
                return malformed("error fields");
            }
            return { ok: true, value: { type, code: code as ErrorCode, message } };
        }
        case "pong":
            return { ok: true, value: { type } };
        case "bye": {
            const reason = asString(obj["reason"]);
            return reason === null
                ? malformed("bye reason")
                : { ok: true, value: { type, reason } };
        }
        default:
            return malformed(`unknown message type ${type}`);
    }
}

function boundedId(value: string, field: string): ProtocolError | null {
    if (value.length === 0 || value.length > MAX_ID_BYTES) {
        return { kind: "out_of_bounds", field };
    }
    for (const char of value) {
        const code = char.charCodeAt(0);
        const printable = (code >= 0x21 && code <= 0x7e) || code === 0x20;
        if (!printable) {
            return { kind: "out_of_bounds", field };
        }
    }
    return null;
}

/** Validate one outbound client message (mirrors the daemon's gate). */
export function validateClient(msg: ClientMsg): ProtocolError | null {
    switch (msg.type) {
        case "hello": {
            const id = boundedId(msg.client_id, "client_id");
            if (id !== null) {
                return id;
            }
            if (msg.token.length > MAX_TEXT_FIELD_BYTES) {
                return { kind: "out_of_bounds", field: "token" };
            }
            if (msg.session !== null) {
                const session = boundedId(msg.session, "session");
                if (session !== null) {
                    return session;
                }
            }
            return null;
        }
        case "command": {
            if (msg.command.type === "submit" || msg.command.type === "steer") {
                return msg.command.text.length > MAX_TEXT_FIELD_BYTES
                    ? { kind: "out_of_bounds", field: "text" }
                    : null;
            }
            return null;
        }
        case "permission_answer":
        case "permission_cancel":
        case "ping":
            return null;
    }
}

/** Encode one client message after bound validation. */
export function encodeClient(msg: ClientMsg): Result<string> {
    const error = validateClient(msg);
    if (error !== null) {
        return { ok: false, error };
    }
    return { ok: true, value: JSON.stringify(msg) };
}

export { PROTOCOL_VERSION };
