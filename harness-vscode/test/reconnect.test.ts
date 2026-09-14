import { test } from "node:test";
import assert from "node:assert/strict";

import { FakeDaemon } from "./fakedaemon.ts";
import { HarnessClient, type ClientEvent } from "../src/client.ts";
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

test("daemon absent retries with capped exponential backoff", async () => {
    const events: ClientEvent[] = [];
    const sleepDelays: number[] = [];
    const sleepGate: Array<() => void> = [];
    const client = new HarnessClient({
        baseUrl: "http://127.0.0.1:9",
        token: "t",
        sessionId: "vscode",
        clientId: "test-client",
        http: nodeHttpIo(),
        sleep: (ms) => {
            sleepDelays.push(ms);
            return new Promise<void>((resolve) => {
                sleepGate.push(resolve);
            });
        },
        random: () => 0.5,
        onEvent: (event) => {
            events.push(event);
        },
    });
    client.start();
    const releaseNextSleep = async (): Promise<void> => {
        while (sleepGate.length === 0) {
            await new Promise<void>((resolve) => {
                setTimeout(resolve, 5);
            });
        }
        sleepGate.shift()!();
    };
    // Deterministic single-stepping: the loop cannot retry until the test
    // releases each backoff sleep, so recorded delays are exact.
    await releaseNextSleep();
    await releaseNextSleep();
    await releaseNextSleep();
    await releaseNextSleep();
    await releaseNextSleep();
    await releaseNextSleep();
    await waitFor(
        () => events.filter((event) => event.kind === "status" && event.status === "connecting").length === 7,
        "seventh attempt",
    );
    assert.deepEqual(sleepDelays, [1000, 2000, 4000, 8000, 15000, 15000, 15000]);
    const stopDone = client.stop();
    await new Promise<void>((resolve) => {
        let done = false;
        const pump = (): void => {
            if (done) {
                return;
            }
            if (sleepGate.length > 0) {
                sleepGate.shift()!();
            }
            void stopDone.then(
                () => {
                    done = true;
                    resolve();
                },
                () => {
                    done = true;
                    resolve();
                },
            );
            setTimeout(pump, 5);
        };
        pump();
    });
    await stopDone;
});

test("flaky daemon: three drops, exact-once resume from cursor", async () => {
    const daemon = await FakeDaemon.start();
    const events: ClientEvent[] = [];
    const client = makeClient(daemon, events);
    client.start();
    await waitFor(() => daemon.sseOpens === 1, "first attach");
    daemon.pushFrame({ id: 1, event: { type: "turn_started", turn: 1 } });
    daemon.pushFrame({ id: 2, event: { type: "turn_started", turn: 2 } });
    await waitFor(() => client.lastEventId() === 2, "cursor 2");
    daemon.killSse();
    await waitFor(() => daemon.sseOpens === 2, "reconnect 1");
    daemon.pushFrame({ id: 3, event: { type: "turn_started", turn: 3 } });
    await waitFor(() => client.lastEventId() === 3, "cursor 3");
    daemon.killSse();
    await waitFor(() => daemon.sseOpens === 3, "reconnect 2");
    daemon.pushFrame({ id: 4, event: { type: "turn_started", turn: 4 } });
    await waitFor(() => client.lastEventId() === 4, "cursor 4");
    daemon.killSse();
    await waitFor(() => daemon.sseOpens === 4, "reconnect 3");
    daemon.pushFrame({ id: 5, event: { type: "turn_started", turn: 5 } });
    await waitFor(() => client.lastEventId() === 5, "cursor 5");
    const ids = events
        .filter((event) => event.kind === "frame")
        .map((event) => (event.kind === "frame" ? event.frame.id : 0));
    assert.deepEqual(ids, [1, 2, 3, 4, 5]);
    assert.deepEqual(daemon.afters, [null, 2, 3, 4]);
    await client.stop();
    await daemon.stop();
});

test("gap in the frame ids forces a resume from the last cursor", async () => {
    const daemon = await FakeDaemon.start();
    const events: ClientEvent[] = [];
    const client = makeClient(daemon, events);
    client.start();
    await waitFor(() => daemon.sseOpens === 1, "attach");
    daemon.pushFrame({ id: 1, event: { type: "turn_started", turn: 1 } });
    await waitFor(() => client.lastEventId() === 1, "cursor 1");
    daemon.pushFrame({ id: 3, event: { type: "turn_started", turn: 3 } });
    await waitFor(() => daemon.sseOpens === 2, "gap-driven reconnect");
    await waitFor(() => client.lastEventId() === 3, "replayed missing frame");
    const ids = events
        .filter((event) => event.kind === "frame")
        .map((event) => (event.kind === "frame" ? event.frame.id : 0));
    assert.deepEqual(ids, [1, 3]);
    assert.deepEqual(daemon.afters, [null, 1]);
    await client.stop();
    await daemon.stop();
});

test("state_resync_required is accepted with its snapshot replacing deltas", async () => {
    const daemon = await FakeDaemon.start();
    daemon.pushFrame({ id: 1, event: { type: "turn_started", turn: 1 } });
    daemon.pushFrame({ id: 2, event: { type: "turn_started", turn: 2 } });
    daemon.pushFrame({ id: 3, event: { type: "turn_started", turn: 3 } });
    daemon.serveResyncInsteadOfDelta();
    const events: ClientEvent[] = [];
    const client = makeClient(daemon, events);
    client.start();
    await waitFor(
        () => events.some((event) => event.kind === "snapshot"),
        "snapshot",
    );
    const snapshot = events.find((event) => event.kind === "snapshot");
    assert.deepEqual(
        snapshot !== undefined && snapshot.kind === "snapshot"
            ? snapshot.frames.map((frame) => frame.id)
            : [],
        [1, 2, 3],
    );
    assert.equal(client.lastEventId(), 3);
    daemon.pushFrame({ id: 4, event: { type: "turn_started", turn: 4 } });
    await waitFor(() => client.lastEventId() === 4, "live after snapshot");
    await client.stop();
    await daemon.stop();
});

test("malformed server frame is surfaced and does not kill the stream", async () => {
    const daemon = await FakeDaemon.start();
    const events: ClientEvent[] = [];
    const client = makeClient(daemon, events);
    client.start();
    await waitFor(() => daemon.sseOpens === 1, "attach");
    daemon.pushRawLine("definitely not json");
    daemon.pushFrame({ id: 1, event: { type: "turn_started", turn: 1 } });
    await waitFor(() => client.lastEventId() === 1, "frame after garbage");
    const malformed = events.filter((event) => event.kind === "malformed");
    assert.equal(malformed.length, 1);
    assert.equal(daemon.sseOpens, 1);
    await client.stop();
    await daemon.stop();
});

test("unknown ask answer maps to a typed unknown_ask error", async () => {
    const daemon = await FakeDaemon.start();
    const client = makeClient(daemon, []);
    client.start();
    await waitFor(() => daemon.sseOpens === 1, "attach");
    const result = await client.answerAsk(99, "deny");
    assert.deepEqual(result, { ok: false, error: { kind: "unknown_ask" } });
    await client.stop();
    await daemon.stop();
});

test("stop resolves promptly after a live attach instead of hanging on a dead socket", async () => {
    const daemon = await FakeDaemon.start();
    const events: ClientEvent[] = [];
    const client = makeClient(daemon, events);
    client.start();
    await waitFor(() => events.some((event) => event.kind === "status" && event.status === "connected"), "connected");
    const outcome = await Promise.race([
        client.stop().then((): "stopped" => "stopped"),
        new Promise<"hung">((resolve) => {
            setTimeout(() => {
                resolve("hung");
            }, 3000);
        }),
    ]);
    assert.equal(outcome, "stopped");
    await daemon.stop();
});

test("clean server-side stream end surfaces as disconnected and reconnects", async () => {
    const daemon = await FakeDaemon.start();
    const events: ClientEvent[] = [];
    const client = makeClient(daemon, events);
    client.start();
    await waitFor(() => daemon.sseOpens === 1, "first attach");
    daemon.endSse();
    await waitFor(() => daemon.sseOpens === 2, "reconnect after clean end");
    const disconnected = events.some(
        (event) => event.kind === "status" && event.status === "disconnected",
    );
    assert.equal(disconnected, true);
    await client.stop();
    await daemon.stop();
});

