import { test } from "node:test";
import assert from "node:assert/strict";

import { planDiff, gateHunkAccept, hunkAction, type HunkPlan } from "../src/hunks.ts";

const diffEvent = {
    type: "diff" as const,
    path: "src/lib.ts",
    before_hash: "aa",
    after_hash: "bb",
    hunks: ["@@ -10,7 +10,9 @@ fn main() {", "@@ -40,3 +42,4 @@"],
};

test("diff event plans one lens/decoration range per hunk on its new-side lines", () => {
    const plan = planDiff(diffEvent);
    assert.deepEqual<HunkPlan>(plan, {
        path: "src/lib.ts",
        hunks: [
            { startLine: 10, endLine: 18, header: "@@ -10,7 +10,9 @@ fn main() {" },
            { startLine: 42, endLine: 45, header: "@@ -40,3 +42,4 @@" },
        ],
    });
});

test("unparseable hunk headers are skipped, not guessed at", () => {
    const plan = planDiff({
        ...diffEvent,
        hunks: ["not a header", "@@ -1,2 +3,4 @@"],
    });
    assert.deepEqual(plan.hunks.map((hunk) => hunk.startLine), [3]);
});

test("accept on a clean file proceeds", () => {
    const gate = gateHunkAccept({ path: "src/lib.ts", dirty: false });
    assert.deepEqual(gate, { action: "apply" });
});

test("accept on a dirty file demands explicit confirmation, never a silent apply", () => {
    const gate = gateHunkAccept({ path: "src/lib.ts", dirty: true });
    assert.deepEqual(gate, {
        action: "confirm",
        reason: "src/lib.ts has unsaved changes; accepting a hunk would shadow them",
    });
});

test("hunk accept answers the pending ask when one targets the file", () => {
    const action = hunkAction(
        { ask_id: 7, tool: "edit_lines", target: "src/lib.ts", reason: "apply", deadline_ms: 1000 },
        { startLine: 10, endLine: 18, header: "@@ -10,7 +10,9 @@" },
    );
    assert.deepEqual(action, { kind: "answer", ask_id: 7, answer: "allow_once" });
});

test("hunk accept with no pending ask falls back to a steer instruction", () => {
    const action = hunkAction(null, { startLine: 10, endLine: 18, header: "@@ -10,7 +10,9 @@" });
    assert.deepEqual(action, {
        kind: "steer",
        text: "accept hunk @@ -10,7 +10,9 @@",
    });
});

test("hunk reject denies the pending ask or steers a rejection", () => {
    const answer = hunkAction(
        { ask_id: 8, tool: "edit_lines", target: "src/lib.ts", reason: "apply", deadline_ms: 1000 },
        { startLine: 1, endLine: 2, header: "@@ -1 +1 @@" },
        "deny",
    );
    assert.deepEqual(answer, { kind: "answer", ask_id: 8, answer: "deny" });
    const steer = hunkAction(null, { startLine: 1, endLine: 2, header: "@@ -1 +1 @@" }, "deny");
    assert.deepEqual(steer, { kind: "steer", text: "reject hunk @@ -1 +1 @@" });
});
