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

test("/steer without text is the table usage error (todo 23)", () => {
    const parsed = parseOmnibar("/steer");
    assert.equal(parsed.kind, "malformed");
    if (parsed.kind === "malformed") {
        assert.equal(parsed.usage, "/steer <text>");
    }
});

test("/cancel parses to a cancel intent and ignores stray text", () => {
    assert.deepEqual(parseOmnibar("/cancel"), { kind: "cancel" });
    assert.deepEqual(parseOmnibar("/cancel now"), { kind: "cancel" });
});

test("unknown slash command is the shared help string (todo 23)", () => {
    const parsed = parseOmnibar("/model sonnet");
    assert.equal(parsed.kind, "unknown");
    if (parsed.kind === "unknown") {
        assert.equal(parsed.help, "unknown command /model — /help lists commands");
    }
});

test("intent maps onto the exact daemon client message shape", () => {
    const steer = parseOmnibar("/steer hurry up");
    assert.deepEqual(steer, { kind: "steer", text: "hurry up" });
    const cancel = parseOmnibar("/cancel");
    assert.deepEqual(cancel, { kind: "cancel" });
});

test("todo 23: every core command parses to a typed intent", () => {
    assert.deepEqual(parseOmnibar("/help"), { kind: "help" });
    assert.deepEqual(parseOmnibar("/vote-cancel"), { kind: "vote-cancel", askId: null });
    assert.deepEqual(parseOmnibar("/vote-cancel 7"), { kind: "vote-cancel", askId: 7 });
    assert.deepEqual(parseOmnibar("/mode plan"), { kind: "mode", on: true });
    assert.deepEqual(parseOmnibar("/mode off"), { kind: "mode", on: false });
    assert.deepEqual(parseOmnibar("/rewind"), { kind: "rewind", steps: 1 });
    assert.deepEqual(parseOmnibar("/rewind 3"), { kind: "rewind", steps: 3 });
    assert.deepEqual(parseOmnibar("/skill audit now"), { kind: "skill", text: "/skill audit now" });
});

test("todo 23: malformed commands surface the table usage; unknown ones the shared help string", () => {
    const bareSteer = parseOmnibar("/steer");
    assert.equal(bareSteer.kind, "malformed");
    if (bareSteer.kind === "malformed") {
        assert.equal(bareSteer.usage, "/steer <text>");
    }
    const bareMode = parseOmnibar("/mode");
    assert.equal(bareMode.kind, "malformed");
    if (bareMode.kind === "malformed") {
        assert.equal(bareMode.usage, "/mode plan|off");
    }
    const unknown = parseOmnibar("/model sonnet");
    assert.equal(unknown.kind, "unknown");
    if (unknown.kind === "unknown") {
        assert.equal(unknown.help, "unknown command /model — /help lists commands");
    }
});

test("todo 23: cli-local words are recognized but marked cli-only", () => {
    assert.deepEqual(parseOmnibar("/exit"), { kind: "cli-only", word: "exit" });
    assert.deepEqual(parseOmnibar("/image pic.png"), { kind: "cli-only", word: "image" });
});
