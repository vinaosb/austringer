/**
 * Transcript projection: frames in, display entries out. Pure and
 * headless-testable; the webview panel glue lives in webview.ts. Frame
 * types with their own editor surface (diff decorations) are silent
 * here so the transcript stays a chronological activity list.
 */

import type { EngineEvent, Frame, SessionEvent } from "./protocol.ts";

export type TranscriptEntryKind =
    | "turn"
    | "message"
    | "tool"
    | "condensation"
    | "notification"
    | "team"
    | "plan";

export type TranscriptEntry = {
    seq: number;
    kind: TranscriptEntryKind;
    text: string;
};

function firstText(blocks: { type: string; text?: string }[]): string | null {
    for (const block of blocks) {
        if (block.type === "text" && typeof block.text === "string") {
            return block.text;
        }
    }
    return null;
}

function entryFromEngine(seq: number, event: EngineEvent): TranscriptEntry | null {
    switch (event.type) {
        case "user":
            return { seq, kind: "message", text: `you: ${firstText(event.blocks) ?? ""}` };
        case "assistant":
            return { seq, kind: "message", text: `assistant: ${firstText(event.blocks) ?? ""}` };
        case "tool_call":
            return { seq, kind: "tool", text: `tool call: ${event.tool} (${event.call_id})` };
        case "tool_result":
            return { seq, kind: "tool", text: `tool result (${event.call_id}): ${firstText(event.blocks) ?? ""}` };
        case "system":
            return { seq, kind: "notification", text: `system: ${event.text}` };
        case "condensation":
            return { seq, kind: "condensation", text: `~ condensed through seq ${event.through_seq}: ${event.summary}` };
        case "snapshot":
            return null;
    }
}

function entryFromSession(frameId: number, event: SessionEvent): TranscriptEntry | null {
    switch (event.type) {
        case "recorded":
            return entryFromEngine(event.seq, event.event);
        case "turn_started":
            return { seq: frameId, kind: "turn", text: `— turn ${event.turn} started —` };
        case "turn_finished":
            return { seq: frameId, kind: "turn", text: `— turn ${event.turn} finished: ${event.outcome} —` };
        case "notification":
            return { seq: frameId, kind: "notification", text: `[${event.kind}] ${event.message}` };
        case "child_started":
            return {
                seq: frameId,
                kind: "team",
                text: `agent started: ${event.child} (${event.role} on ${event.model}) — ${event.task}`,
            };
        case "child_progress":
            return { seq: frameId, kind: "team", text: `agent ${event.child}: ${event.phase} ${event.detail}` };
        case "child_finished":
            return {
                seq: frameId,
                kind: "team",
                text: `agent finished: ${event.child} — ${event.state} ${event.detail}`,
            };
        case "team_budget":
            return {
                seq: frameId,
                kind: "team",
                text: `agents budget: ${event.spawned}/${event.fan_out_cap} spawned, ${event.tokens_spent}/${event.token_budget} tokens`,
            };
        case "team_budget_exhausted":
            return { seq: frameId, kind: "team", text: `agents budget exhausted: ${event.spent}/${event.budget} tokens` };
        case "cost_warning":
            return { seq: frameId, kind: "notification", text: `cost warning: ${event.consumed_micros}/${event.cap_micros} µUSD` };
        case "cost_stop":
            return { seq: frameId, kind: "notification", text: `cost stop: ${event.consumed_micros}/${event.cap_micros} µUSD` };
        case "plan_mode_entered":
            return { seq: frameId, kind: "plan", text: `plan mode entered (plan ${event.plan})` };
        case "plan_mode_exited":
            return { seq: frameId, kind: "plan", text: `plan mode exited (plan ${event.plan})` };
        case "plan_blocked":
            return {
                seq: frameId,
                kind: "plan",
                text: `plan ${event.plan} blocked write: ${event.tool}${event.target === null ? "" : ` → ${event.target}`}`,
            };
        case "plan_updated":
            return { seq: frameId, kind: "plan", text: `plan ${event.plan} updated to revision ${event.revision}` };
        case "plan_approved":
            return { seq: frameId, kind: "plan", text: `plan ${event.plan} approved` };
        case "plan_rejected":
            return { seq: frameId, kind: "plan", text: `plan ${event.plan} rejected` };
        case "diff":
        case "worker_claimed":
        case "worker_healed":
            return null;
    }
}

export function transcriptEntryFromFrame(frame: Frame): TranscriptEntry | null {
    return entryFromSession(frame.id, frame.event);
}

function escapeHtml(text: string): string {
    return text
        .replaceAll("&", "&amp;")
        .replaceAll("<", "&lt;")
        .replaceAll(">", "&gt;")
        .replaceAll('"', "&#34;")
        .replaceAll("'", "&#39;");
}

export function renderTranscriptHtml(entries: readonly TranscriptEntry[]): string {
    const items = entries
        .map((entry) => `    <li class="entry ${entry.kind}">${escapeHtml(entry.text)}</li>`)
        .join("\n");
    return [
        "<!DOCTYPE html>",
        '<html lang="en">',
        "<head><meta charset=\"utf-8\"><title>Austringer transcript</title></head>",
        "<body>",
        '  <ul class="transcript">',
        items,
        "  </ul>",
        "</body>",
        "</html>",
    ].join("\n");
}
