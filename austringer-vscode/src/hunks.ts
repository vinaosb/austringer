/**
 * Diff-event → IDE surface mapping: each hunk header in a daemon
 * `diff` session event becomes one decoration range plus one
 * accept/reject code lens on its new-side lines, and the accept
 * action is gated on editor cleanliness so a hunk never silently
 * shadows unsaved user edits.
 */

import type { AskPayload, SessionEvent } from "./protocol.ts";

export type HunkRange = {
    startLine: number;
    endLine: number;
    header: string;
};

export type HunkPlan = {
    path: string;
    hunks: HunkRange[];
};

const HUNK_HEADER = /^@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@/;

function parseHunk(header: string): HunkRange | null {
    const match = HUNK_HEADER.exec(header);
    if (match === null) {
        return null;
    }
    const start = Number(match[1]);
    const lineCount = match[2] === undefined ? 1 : Number(match[2]);
    const end = lineCount === 0 ? start : start + lineCount - 1;
    return { startLine: start, endLine: end, header };
}

export function planDiff(event: Extract<SessionEvent, { type: "diff" }>): HunkPlan {
    const hunks: HunkRange[] = [];
    for (const raw of event.hunks) {
        const range = parseHunk(raw);
        if (range !== null) {
            hunks.push(range);
        }
    }
    return { path: event.path, hunks };
}

export type HunkGate =
    | { action: "apply" }
    | { action: "confirm"; reason: string };

export function gateHunkAccept(target: { path: string; dirty: boolean }): HunkGate {
    if (target.dirty) {
        return {
            action: "confirm",
            reason: `${target.path} has unsaved changes; accepting a hunk would shadow them`,
        };
    }
    return { action: "apply" };
}

export type HunkDecision =
    | { kind: "answer"; ask_id: number; answer: "allow_once" | "deny" }
    | { kind: "steer"; text: string };

export function hunkAction(
    pendingAsk: AskPayload | null,
    hunk: HunkRange,
    verdict: "allow_once" | "deny" = "allow_once",
): HunkDecision {
    if (pendingAsk !== null) {
        return { kind: "answer", ask_id: pendingAsk.ask_id, answer: verdict };
    }
    const verb = verdict === "allow_once" ? "accept" : "reject";
    return { kind: "steer", text: `${verb} hunk ${hunk.header}` };
}
