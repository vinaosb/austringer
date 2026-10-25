import { test } from "node:test";
import assert from "node:assert/strict";

import { FakeDaemon } from "./fakedaemon.ts";
import { HarnessClient, type ClientEvent } from "../src/client.ts";
import { createSseParser } from "../src/sse.ts";
import { nodeHttpIo } from "../src/nodehttp.ts";

const immediateSleep = async (): Promise<void> => {
    await new Promise<void>((resolve) => {
        setImmediate(resolve);
    });
};

async function waitFor(condition: () => boolean, what: string): Promise<void> {
    const deadline = Date.now() + 5_000;
    while (!condition()) {
        if (Date.now() > deadline) {
            assert.fail(`timeout waiting for ${what}`);
        }
        await new Promise<void>((resolve) => {
            setTimeout(resolve, 10);
        });
    }
}

function makeClient(
    daemon: FakeDaemon,
    events: ClientEvent[],
    token = daemon.token,
): HarnessClient {
    return new HarnessClient({
        baseUrl: daemon.url,
        token,
        sessionId: "vscode",
        clientId: "test-client",
        http: nodeHttpIo(),
        sleep: immediateSleep,
        random: () => 0.5,
        onEvent: (event) => {
            events.push(event);
        },
    });
}

test("sse parser buffers split chunks, ignores comments, joins data lines", () => {
    const payloads: string[] = [];
    const parser = createSseParser((payload) => {
        payloads.push(payload);
    });
    parser.push('data: {"a":');
    parser.push('1}\n\n');
    parser.push(": keep-alive\n\n");
    parser.push('data: {"b"}\n\n');
    parser.push("data: x\ndata: y\n\n");
    assert.deepEqual(payloads, ['{"a":1}', '{"b"}', "x\ny"]);
});

test("attach reaches connected, delivers ordered frames, tracks cursor", async () => {
    const daemon = await FakeDaemon.start();
    const events: ClientEvent[] = [];
    const client = makeClient(daemon, events);
    client.start();
    await waitFor(() => events.some((event) => event.kind === "status" && event.status === "connected"), "connected");
    daemon.pushFrame({ id: 1, event: { type: "turn_started", turn: 1 } });
    daemon.pushFrame({
        id: 2,
        event: { type: "notification", kind: "info", message: "hi" },
    });
    await waitFor(() => events.filter((event) => event.kind === "frame").length === 2, "two frames");
    assert.equal(client.lastEventId(), 2);
    await client.stop();
    await daemon.stop();
});

test("submit posts the exact client message with the token header", async () => {
    const daemon = await FakeDaemon.start();
    const events: ClientEvent[] = [];
    const client = makeClient(daemon, events);
    client.start();
    await waitFor(() => events.some((event) => event.kind === "status" && event.status === "connected"), "connected");
    const result = await client.submit("fix this");
    assert.deepEqual(result, { ok: true });
    assert.equal(daemon.posts.length, 1);
    const posted = daemon.posts[0];
    if (posted === undefined) {
        assert.fail("no post recorded");
    }
    assert.match(posted.url, /^\/s\/vscode\/cmd\?token=.+/);
    assert.deepEqual(JSON.parse(posted.body), {
        type: "command",
        command: { type: "submit", text: "fix this" },
    });
    await client.stop();
    await daemon.stop();
});

test("oversized submit text is refused client-side, no request sent", async () => {
    const daemon = await FakeDaemon.start();
    const client = makeClient(daemon, []);
    const result = await client.submit("a".repeat(64 * 1024 + 1));
    assert.deepEqual(result, {
        ok: false,
        error: { kind: "out_of_bounds", field: "text" },
    });
    assert.equal(daemon.posts.length, 0);
    await client.stop();
    await daemon.stop();
});

test("wrong token is a typed auth failure with no retry loop", async () => {
    const daemon = await FakeDaemon.start();
    const events: ClientEvent[] = [];
    const client = makeClient(daemon, events, "wrong-token");
    client.start();
    await waitFor(() => events.some((event) => event.kind === "status" && event.status === "disconnected"), "disconnected");
    const statuses = events
        .filter((event) => event.kind === "status")
        .map((event) => (event.kind === "status" ? event.status : ""));
    assert.deepEqual(statuses, ["connecting", "disconnected"]);
    assert.equal(daemon.sseOpens, 0);
    const submit = await client.submit("hello");
    assert.deepEqual(submit, { ok: false, error: { kind: "auth" } });
    await client.stop();
    await daemon.stop();
});


test("full session: attach, frames, steer, permission answer, turn completes", async () => {
    const daemon = await FakeDaemon.start();
    const events: ClientEvent[] = [];
    const client = makeClient(daemon, events);
    client.start();
    await waitFor(() => events.some((event) => event.kind === "status" && event.status === "connected"), "connected");
    daemon.pushFrame({ id: 1, event: { type: "turn_started", turn: 1 } });
    daemon.pushFrame({
        id: 2,
        event: { type: "recorded", seq: 1, event: { type: "user", blocks: [{ type: "text", text: "fix this" }] } },
    });
    await waitFor(() => client.lastEventId() === 2, "user turn recorded");
    daemon.push({
        type: "permission_ask",
        ask: { ask_id: 1, tool: "edit_lines", target: "src/lib.ts", reason: "apply hunk", deadline_ms: 60000 },
    });
    await waitFor(() => events.some((event) => event.kind === "ask"), "ask");
    const answer = await client.answerAsk(1, "allow_once");
    assert.deepEqual(answer, { ok: true });
    assert.equal(daemon.posts.length, 1);
    assert.deepEqual(JSON.parse(daemon.posts[0]!.body), {
        type: "permission_answer",
        ask_id: 1,
        answer: "allow_once",
    });
    const steered = await client.steer("keep it typed");
    assert.deepEqual(steered, { ok: true });
    daemon.pushFrame({ id: 3, event: { type: "turn_finished", turn: 1, outcome: "complete" } });
    await waitFor(
        () => events.some((event) => event.kind === "frame" && event.frame.event.type === "turn_finished"),
        "turn finished",
    );
    assert.equal(client.lastEventId(), 3);
    await client.stop();
    await daemon.stop();
});

test("permission ask and resolution surface as client events", async () => {
    const daemon = await FakeDaemon.start();
    const events: ClientEvent[] = [];
    const client = makeClient(daemon, events);
    client.start();
    await waitFor(() => daemon.sseOpens === 1, "attach");
    daemon.push({
        type: "permission_ask",
        ask: {
            ask_id: 1,
            tool: "edit_lines",
            target: "src/lib.rs",
            reason: "semantic hunk",
            deadline_ms: 120000,
        },
    });
    await waitFor(
        () => events.some((event) => event.kind === "ask"),
        "ask event",
    );
    daemon.push({
        type: "permission_resolved",
        ask_id: 1,
        resolution: { allowed: { by: "web-1" } },
    });
    await waitFor(
        () => events.some((event) => event.kind === "resolved"),
        "resolved event",
    );
    await client.stop();
    await daemon.stop();
});

test("black-holed command post fails typed within the post timeout", async () => {
    const daemon = await FakeDaemon.start();
    const client = new HarnessClient({
        baseUrl: daemon.url,
        token: daemon.token,
        sessionId: "vscode",
        clientId: "test-client",
        http: nodeHttpIo(200),
        sleep: immediateSleep,
        random: () => 0.5,
        onEvent: () => {},
    });
    client.start();
    await waitFor(() => daemon.sseOpens === 1, "attach");
    daemon.hangPosts();
    const outcome = await Promise.race([
        client.submit("hello"),
        new Promise<{ ok: false; error: { kind: string } }>((resolve) => {
            setTimeout(() => {
                resolve({ ok: false, error: { kind: "test-timeout" } });
            }, 5000);
        }),
    ]);
    assert.equal(outcome.ok, false);
    if (!outcome.ok) {
        assert.equal(outcome.error.kind, "unreachable");
    }
    await client.stop();
    await daemon.stop();
});
