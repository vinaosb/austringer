import { test } from "node:test";
import assert from "node:assert/strict";

import {
    PROTOCOL_VERSION,
    MAX_FRAME_BYTES,
    MAX_TEXT_FIELD_BYTES,
    MAX_ID_BYTES,
    type ClientMsg,
} from "../src/protocol.ts";
import { encodeClient, decodeServerMessage } from "../src/codec.ts";

function ok<T>(value: { ok: true; value: T } | { ok: false; error: unknown }): T {
    assert.equal(value.ok, true, `expected ok, got ${JSON.stringify(value)}`);
    if (value.ok) {
        return value.value;
    }
    assert.fail("unreachable");
}

test("hello encodes to the exact daemon wire shape", () => {
    const msg: ClientMsg = {
        type: "hello",
        version: PROTOCOL_VERSION,
        token: "t".repeat(32),
        client_id: "vscode-1",
        session: null,
    };
    const encoded = ok(encodeClient(msg));
    const parsed: object = JSON.parse(encoded);
    assert.deepEqual(parsed, {
        type: "hello",
        version: 2,
        token: "t".repeat(32),
        client_id: "vscode-1",
        session: null,
    });
});

test("submit command encodes with nested snake_case tag", () => {
    const encoded = ok(
        encodeClient({ type: "command", command: { type: "submit", text: "fix this" } }),
    );
    assert.deepEqual(JSON.parse(encoded), {
        type: "command",
        command: { type: "submit", text: "fix this" },
    });
});

test("permission answer encodes with snake_case answer", () => {
    const encoded = ok(
        encodeClient({ type: "permission_answer", ask_id: 7, answer: "allow_session" }),
    );
    assert.deepEqual(JSON.parse(encoded), {
        type: "permission_answer",
        ask_id: 7,
        answer: "allow_session",
    });
});

test("oversized submit text is refused before it reaches the daemon", () => {
    const result = encodeClient({
        type: "command",
        command: { type: "submit", text: "a".repeat(MAX_TEXT_FIELD_BYTES + 1) },
    });
    assert.deepEqual(result, { ok: false, error: { kind: "out_of_bounds", field: "text" } });
});

test("oversized token is refused client-side", () => {
    const result = encodeClient({
        type: "hello",
        version: 1,
        token: "x".repeat(MAX_TEXT_FIELD_BYTES + 1),
        client_id: "vscode",
        session: null,
    });
    assert.deepEqual(result, { ok: false, error: { kind: "out_of_bounds", field: "token" } });
});

test("client id over the bound or with control bytes is refused", () => {
    const long = encodeClient({
        type: "hello",
        version: 1,
        token: "t",
        client_id: "c".repeat(MAX_ID_BYTES + 1),
        session: null,
    });
    assert.deepEqual(long, { ok: false, error: { kind: "out_of_bounds", field: "client_id" } });
    const dirty = encodeClient({
        type: "hello",
        version: 1,
        token: "t",
        client_id: "bad\nid",
        session: null,
    });
    assert.deepEqual(dirty, { ok: false, error: { kind: "out_of_bounds", field: "client_id" } });
});

test("server frame over 1 MiB is rejected before parsing", () => {
    const huge = "x".repeat(MAX_FRAME_BYTES + 1);
    const result = decodeServerMessage(huge);
    assert.deepEqual(result, { ok: false, error: { kind: "oversized" } });
});

test("garbage and unknown tags decode to typed malformed errors", () => {
    const garbage = decodeServerMessage("not json at all");
    assert.equal(garbage.ok, false);
    if (!garbage.ok) {
        assert.equal(garbage.error.kind, "malformed");
    }
    const unknown = decodeServerMessage('{"type":"from_the_future"}');
    assert.equal(unknown.ok, false);
    if (!unknown.ok) {
        assert.equal(unknown.error.kind, "malformed");
    }
});

test("hello_ack frame decodes", () => {
    const msg = ok(decodeServerMessage('{"type":"hello_ack","version":1,"last_event_id":12}'));
    assert.deepEqual(msg, { type: "hello_ack", version: 1, last_event_id: 12 });
});

test("event frame carrying a turn event decodes", () => {
    const msg = ok(
        decodeServerMessage(
            '{"type":"event","frame":{"id":3,"event":{"type":"turn_started","turn":1}}}',
        ),
    );
    const frame = msg.type === "event" ? msg.frame : assert.fail("expected event");
    assert.equal(frame.id, 3);
    assert.deepEqual(frame.event, { type: "turn_started", turn: 1 });
});

test("recorded engine events decode across all seven kinds", () => {
    const engineEvents = [
        '{"type":"user","blocks":[{"type":"text","text":"hi"}]}',
        '{"type":"assistant","blocks":[{"type":"text","text":"hello"}]}',
        '{"type":"tool_call","call_id":"c1","tool":"edit_lines","arguments":{"file":"a.rs"}}',
        '{"type":"tool_result","call_id":"c1","blocks":[]}',
        '{"type":"system","text":"degraded"}',
        '{"type":"condensation","summary":"s","through_seq":5}',
        '{"type":"snapshot","label":"pre-edit"}',
    ];
    let expectedSeq = 1;
    for (const engine of engineEvents) {
        const msg = ok(
            decodeServerMessage(
                `{"type":"event","frame":{"id":${expectedSeq},"event":{"type":"recorded","seq":${expectedSeq},"event":${engine}}}}`,
            ),
        );
        const frame = msg.type === "event" ? msg.frame : assert.fail("expected event");
        const recorded =
            frame.event.type === "recorded"
                ? frame.event
                : assert.fail(`expected recorded, got ${JSON.stringify(frame.event)}`);
        assert.equal(recorded.seq, expectedSeq);
        expectedSeq += 1;
    }
});

test("diff event decodes with path and rendered hunks", () => {
    const msg = ok(
        decodeServerMessage(
            '{"type":"event","frame":{"id":9,"event":{"type":"diff","path":"src/lib.rs","before_hash":"aa","after_hash":"bb","hunks":["@@ -1,3 +1,4 @@"]}}}',
        ),
    );
    const frame = msg.type === "event" ? msg.frame : assert.fail("expected event");
    assert.deepEqual(frame.event, {
        type: "diff",
        path: "src/lib.rs",
        before_hash: "aa",
        after_hash: "bb",
        hunks: ["@@ -1,3 +1,4 @@"],
    });
});

test("permission ask and resolved (both payload and unit resolutions) decode", () => {
    const ask = ok(
        decodeServerMessage(
            '{"type":"permission_ask","ask":{"ask_id":7,"tool":"edit_lines","target":"src/lib.rs","reason":"semantic hunk","deadline_ms":120000}}',
        ),
    );
    assert.deepEqual(ask, {
        type: "permission_ask",
        ask: {
            ask_id: 7,
            tool: "edit_lines",
            target: "src/lib.rs",
            reason: "semantic hunk",
            deadline_ms: 120000,
        },
    });
    const allowed = ok(
        decodeServerMessage(
            '{"type":"permission_resolved","ask_id":7,"resolution":{"allowed":{"by":"vscode-1"}}}',
        ),
    );
    assert.deepEqual(
        allowed.type === "permission_resolved" ? allowed.resolution : null,
        { allowed: { by: "vscode-1" } },
    );
    const timedOut = ok(
        decodeServerMessage('{"type":"permission_resolved","ask_id":8,"resolution":"timeout"}'),
    );
    assert.deepEqual(
        timedOut.type === "permission_resolved" ? timedOut.resolution : null,
        { timeout: true },
    );
});

test("resync, snapshot, error, pong, and bye decode", () => {
    const resync = ok(
        decodeServerMessage('{"type":"state_resync_required","last_event_id":40}'),
    );
    assert.deepEqual(resync, { type: "state_resync_required", last_event_id: 40 });
    const snap = ok(
        decodeServerMessage(
            '{"type":"snapshot","frames":[{"id":1,"event":{"type":"notification","kind":"info","message":"m"}}]}',
        ),
    );
    const frames = snap.type === "snapshot" ? snap.frames : assert.fail("expected snapshot");
    assert.equal(frames.length, 1);
    const err = ok(decodeServerMessage('{"type":"error","code":"auth","message":"no"}'));
    assert.deepEqual(err, { type: "error", code: "auth", message: "no" });
    const pong = ok(decodeServerMessage('{"type":"pong"}'));
    assert.deepEqual(pong, { type: "pong" });
    const bye = ok(decodeServerMessage('{"type":"bye","reason":"idle read timeout"}'));
    assert.deepEqual(bye, { type: "bye", reason: "idle read timeout" });
});

test("malformed nested event inside a frame is rejected, not silently passed", () => {
    const result = decodeServerMessage(
        '{"type":"event","frame":{"id":1,"event":{"type":"mystery_event"}}}',
    );
    assert.equal(result.ok, false);
    if (!result.ok) {
        assert.equal(result.error.kind, "malformed");
    }
});
