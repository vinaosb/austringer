import { test } from "node:test";
import assert from "node:assert/strict";

import { FakeDaemon } from "./fakedaemon.ts";
import { HarnessClient, type ClientEvent } from "../src/client.ts";
import { PROTOCOL_VERSION } from "../src/protocol.ts";
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

test("the connection negotiates protocol version 2 on the SSE URL", async () => {
    const daemon = await FakeDaemon.start();
    const client = makeClient(daemon, []);
    client.start();
    await waitFor(() => daemon.sseOpens === 1, "attach");
    assert.equal(PROTOCOL_VERSION, 2, "client protocol constant must be 2");
    assert.deepEqual(daemon.versions, [2], "SSE open must carry version=2");
    await client.stop();
    await daemon.stop();
});

test("v2 hello_ack (version 2) is accepted and reaches connected", async () => {
    const daemon = await FakeDaemon.start();
    const events: ClientEvent[] = [];
    const client = makeClient(daemon, events);
    client.start();
    await waitFor(
        () => events.some((event) => event.kind === "status" && event.status === "connected"),
        "connected under v2",
    );
    await client.stop();
    await daemon.stop();
});

test("team frames (child_started/progress/finished) decode and advance the cursor without a reconnect loop", async () => {
    const daemon = await FakeDaemon.start();
    const events: ClientEvent[] = [];
    const client = makeClient(daemon, events);
    client.start();
    await waitFor(() => daemon.sseOpens === 1, "attach");
    daemon.pushFrame({
        id: 1,
        event: { type: "child_started", child: "l1-child-explore", task: "explore", role: "explorer", model: "gpt-6-luna" },
    });
    daemon.pushFrame({
        id: 2,
        event: { type: "child_progress", child: "l1-child-explore", phase: "file_touched", detail: "src/lib.rs" },
    });
    daemon.pushFrame({
        id: 3,
        event: { type: "child_finished", child: "l1-child-explore", task: "explore", state: "success", detail: "done" },
    });
    daemon.pushFrame({ id: 4, event: { type: "turn_finished", turn: 1, outcome: "complete" } });
    await waitFor(() => client.lastEventId() === 4, "all four frames accepted");
    await waitFor(
        () =>
            events.filter((event) => event.kind === "frame").length === 4,
        "four frame events",
    );
    assert.equal(daemon.sseOpens, 1, "no gap-driven reconnect: team frames must decode");
    await client.stop();
    await daemon.stop();
});

test("fallback ask surfaces as a client event; answering posts fallback_answer with the chosen identity", async () => {
    const daemon = await FakeDaemon.start();
    const events: ClientEvent[] = [];
    const client = makeClient(daemon, events);
    client.start();
    await waitFor(() => daemon.sseOpens === 1, "attach");
    daemon.push({
        type: "fallback_ask",
        ask: {
            ask_id: 1,
            current: { provider: "anthropic", model: "claude-haiku-5.5" },
            proposed: { provider: "openai", model: "gpt-6.1-sol" },
            reason: "provider 429",
            deadline_ms: 120000,
        },
    });
    await waitFor(() => events.some((event) => event.kind === "fallback-ask"), "fallback ask");
    const result = await client.answerFallback(1, { provider: "openai", model: "gpt-6.1-sol" });
    assert.deepEqual(result, { ok: true });
    assert.deepEqual(JSON.parse(daemon.posts[0]!.body), {
        type: "fallback_answer",
        ask: 1,
        chosen: { provider: "openai", model: "gpt-6.1-sol" },
    });
    daemon.push({
        type: "fallback_resolved",
        ask_id: 1,
        chosen: { provider: "openai", model: "gpt-6.1-sol" },
    });
    await waitFor(
        () => events.some((event) => event.kind === "fallback-resolved"),
        "fallback resolved",
    );
    await client.stop();
    await daemon.stop();
});

test("plan ask surfaces as a client event; approving posts plan_answer approve", async () => {
    const daemon = await FakeDaemon.start();
    const events: ClientEvent[] = [];
    const client = makeClient(daemon, events);
    client.start();
    await waitFor(() => daemon.sseOpens === 1, "attach");
    daemon.push({
        type: "plan_ask",
        ask: {
            ask_id: 1,
            plan_id: 7,
            revision: 1,
            artifact: "# plan\n- step one",
            deadline_ms: 120000,
        },
    });
    await waitFor(() => events.some((event) => event.kind === "plan-ask"), "plan ask");
    const result = await client.answerPlan(1, "approve");
    assert.deepEqual(result, { ok: true });
    assert.deepEqual(JSON.parse(daemon.posts[0]!.body), {
        type: "plan_answer",
        ask: 1,
        decision: "approve",
    });
    daemon.push({
        type: "plan_resolved",
        ask_id: 1,
        plan_id: 7,
        outcome: { approved: { by: "vscode" } },
    });
    await waitFor(
        () => events.some((event) => event.kind === "plan-resolved"),
        "plan resolved",
    );
    await client.stop();
    await daemon.stop();
});

test("plan mode toggle posts plan_mode on/off", async () => {
    const daemon = await FakeDaemon.start();
    const client = makeClient(daemon, []);
    client.start();
    await waitFor(() => daemon.sseOpens === 1, "attach");
    assert.deepEqual(await client.planMode(true), { ok: true });
    assert.deepEqual(JSON.parse(daemon.posts[0]!.body), { type: "plan_mode", on: true });
    await client.stop();
    await daemon.stop();
});

test("a malformed raw line mid-stream is ignored and later frames still arrive (webview stream never breaks)", async () => {
    const daemon = await FakeDaemon.start();
    const events: ClientEvent[] = [];
    const client = makeClient(daemon, events);
    client.start();
    await waitFor(() => daemon.sseOpens === 1, "attach");
    daemon.pushRawLine("}{ not json");
    daemon.pushFrame({ id: 1, event: { type: "notification", kind: "info", message: "still alive" } });
    await waitFor(() => client.lastEventId() === 1, "frame after garbage");
    assert.ok(events.some((event) => event.kind === "malformed"));
    assert.equal(daemon.sseOpens, 1);
    await client.stop();
    await daemon.stop();
});
