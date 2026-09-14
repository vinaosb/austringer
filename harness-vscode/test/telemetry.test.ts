import { test } from "node:test";
import assert from "node:assert/strict";

import { buildTelemetryContext, type EditorSnapshot } from "../src/telemetry.ts";

const snapshot: EditorSnapshot = {
    activeFile: "src/lib.ts",
    language: "typescript",
    line: 42,
    column: 7,
    selectedText: "parseOmnibar(input)",
    diagnostics: [
        { severity: "error", line: 42, message: "Argument of type 'string' is not assignable" },
    ],
};

test("telemetry disabled (the default) sends nothing", () => {
    assert.equal(buildTelemetryContext(false, snapshot), null);
});

test("enabled telemetry emits file, position, selection, and diagnostics", () => {
    const context = buildTelemetryContext(true, snapshot);
    assert.ok(context !== null);
    assert.match(context, /src\/lib\.ts/);
    assert.match(context, /42:7/);
    assert.match(context, /parseOmnibar\(input\)/);
    assert.match(context, /error.*42:.*not assignable/);
});

test("'fix this' with telemetry carries the file so the agent never asks which file", () => {
    const context = buildTelemetryContext(true, snapshot);
    const submitted = `fix this\n\n${context ?? ""}`;
    assert.match(submitted, /fix this/);
    assert.match(submitted, /src\/lib\.ts/);
    assert.match(submitted, /42:7/);
});

test("enabled but no editor focus yields null, not an empty block", () => {
    const bare: EditorSnapshot = {
        activeFile: null,
        language: null,
        line: null,
        column: null,
        selectedText: null,
        diagnostics: [],
    };
    assert.equal(buildTelemetryContext(true, bare), null);
});

test("selection is bounded: an enormous selection is truncated, not shipped whole", () => {
    const huge: EditorSnapshot = {
        ...snapshot,
        selectedText: "x".repeat(20_000),
    };
    const context = buildTelemetryContext(true, huge);
    assert.ok(context !== null);
    assert.ok(context.length < 2_000);
});
