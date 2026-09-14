import { test } from "node:test";
import assert from "node:assert/strict";

import { parseOmnibar } from "../src/omnibar.ts";

test("plain text parses to a submit intent", () => {
    assert.deepEqual(parseOmnibar("fix this"), { kind: "submit", text: "fix this" });
});

test("whitespace-only input is a typed empty intent", () => {
    assert.deepEqual(parseOmnibar("   "), { kind: "empty" });
});

test("/steer with text parses to a steer intent", () => {
    assert.deepEqual(parseOmnibar("/steer keep it typed"), {
        kind: "steer",
        text: "keep it typed",
    });
});

test("/steer without text is a typed usage error", () => {
    const parsed = parseOmnibar("/steer");
    assert.equal(parsed.kind, "unknown");
    if (parsed.kind === "unknown") {
        assert.match(parsed.usage, /\/steer <text>/);
    }
});

test("/cancel parses to a cancel intent and ignores stray text", () => {
    assert.deepEqual(parseOmnibar("/cancel"), { kind: "cancel" });
    assert.deepEqual(parseOmnibar("/cancel now"), { kind: "cancel" });
});

test("unknown slash command is a typed error listing the grammar", () => {
    const parsed = parseOmnibar("/model sonnet");
    assert.equal(parsed.kind, "unknown");
    if (parsed.kind === "unknown") {
        assert.match(parsed.usage, /\/steer/);
        assert.match(parsed.usage, /\/cancel/);
        assert.doesNotMatch(parsed.usage, /\/model/);
    }
});

test("intent maps onto the exact daemon client message shape", () => {
    const steer = parseOmnibar("/steer hurry up");
    assert.deepEqual(steer, { kind: "steer", text: "hurry up" });
    const cancel = parseOmnibar("/cancel");
    assert.deepEqual(cancel, { kind: "cancel" });
});
