import { test } from "node:test";
import assert from "node:assert/strict";

import type { Frame } from "../src/protocol.ts";
import {
    transcriptEntryFromFrame,
    renderTranscriptHtml,
    type TranscriptEntry,
} from "../src/transcript.ts";

/** Fixture stream: a v2-shaped mix riding monotonic frame ids. */
function fixtureFrames(): Frame[] {
    return [
        {
            id: 1,
            event: { type: "turn_started", turn: 1 },
        },
        {
            id: 2,
            event: {
                type: "recorded",
                seq: 1,
                event: { type: "user", blocks: [{ type: "text", text: "fix the parser" }] },
            },
        },
        {
            id: 3,
            event: {
                type: "recorded",
                seq: 2,
                event: { type: "tool_call", call_id: "c1", tool: "edit_lines", arguments: { path: "src/lib.rs" } },
            },
        },
        {
            id: 4,
            event: {
                type: "recorded",
                seq: 3,
                event: { type: "tool_result", call_id: "c1", blocks: [{ type: "text", text: "applied" }] },
            },
        },
        {
            id: 5,
            event: {
                type: "recorded",
                seq: 4,
                event: { type: "condensation", summary: "early history folded", through_seq: 3 },
            },
        },
        {
            id: 6,
            event: { type: "notification", kind: "info", message: "daemon note" },
        },
        {
            id: 7,
            event: { type: "child_started", child: "l1-child-explore", task: "explore", role: "explorer", model: "gpt-6-luna" },
        },
        {
            id: 8,
            event: { type: "child_progress", child: "l1-child-explore", phase: "file_touched", detail: "src/lib.rs" },
        },
        {
            id: 9,
            event: { type: "child_finished", child: "l1-child-explore", task: "explore", state: "success", detail: "3 findings" },
        },
        {
            id: 10,
            event: { type: "turn_finished", turn: 1, outcome: "complete" },
        },
    ];
}

test("transcript renders tool, condensation, notification, and team entries from a fixture stream", () => {
    const entries = fixtureFrames()
        .map((frame) => transcriptEntryFromFrame(frame))
        .filter((entry): entry is TranscriptEntry => entry !== null);
    const html = renderTranscriptHtml(entries);

    const toolCall = entries.find((entry) => entry.kind === "tool" && entry.text.includes("edit_lines"));
    assert.ok(toolCall !== undefined, "tool_call frame yields an entry");
    assert.equal(toolCall.kind, "tool");
    assert.ok(toolCall.text.includes("edit_lines"), "tool entry names the tool");

    const condensation = entries.find((entry) => entry.kind === "condensation");
    assert.ok(condensation !== undefined, "condensation frame yields an entry");
    assert.ok(condensation.text.includes("early history folded"));

    const team = entries.filter((entry) => entry.kind === "team");
    assert.equal(team.length, 3, "child started/progress/finished all surface as team entries");
    assert.ok(team[0]!.text.includes("l1-child-explore"));

    const notification = entries.find((entry) => entry.kind === "notification");
    assert.ok(notification !== undefined);
    assert.ok(notification.text.includes("daemon note"));

    assert.ok(html.includes('class="entry tool"'));
    assert.ok(html.includes('class="entry condensation"'));
    assert.ok(html.includes('class="entry team"'));
    assert.ok(html.includes("<ul") && html.includes("</ul>"), "plain HTML list, no framework");
});

test("every fixture frame either renders an entry or is explicitly silent — none crash", () => {
    for (const frame of fixtureFrames()) {
        const entry = transcriptEntryFromFrame(frame);
        assert.ok(entry === null || (entry.kind.length > 0 && entry.text.length > 0));
    }
});

test("diff frames stay silent in the transcript (they render as editor decorations)", () => {
    const entry = transcriptEntryFromFrame({
        id: 11,
        event: { type: "diff", path: "src/lib.rs", before_hash: "aa", after_hash: "bb", hunks: ["@@ -1 +1 @@"] },
    });
    assert.equal(entry, null);
});

test("html rendering escapes entry text — a hostile frame cannot inject markup", () => {
    const html = renderTranscriptHtml([
        { seq: 1, kind: "notification", text: "<script>alert(1)</script>" },
    ]);
    assert.ok(!html.includes("<script>"), "raw script tag must not survive");
    assert.ok(html.includes("&lt;script&gt;"));
});

test("rendering an empty transcript produces a valid empty list, not an error", () => {
    const html = renderTranscriptHtml([]);
    assert.ok(html.includes("<ul"));
    assert.ok(!html.includes("<li"));
});
