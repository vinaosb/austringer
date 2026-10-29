import { test } from "node:test";
import assert from "node:assert/strict";

import { appendBounded, createRenderScheduler, MAX_ENTRIES } from "../src/renderschedule.ts";

test("N pushes inside one window coalesce into exactly one flush", () => {
    const scheduler = createRenderScheduler({ windowMs: 200 });
    assert.equal(scheduler.push(1000), 1200, "first push arms a trailing-edge flush at now + windowMs");
    for (let t = 1010; t <= 1190; t += 10) {
        assert.equal(scheduler.push(t), null, "pushes inside the armed window never re-arm");
    }
    assert.equal(scheduler.takeFlush(1199), false, "flush is not due before the window ends");
    assert.equal(scheduler.takeFlush(1200), true, "flush due exactly at the window end");
    assert.equal(scheduler.takeFlush(1201), false, "flush is consumed — no second render");
});

test("a push after a consumed flush arms the next window", () => {
    const scheduler = createRenderScheduler({ windowMs: 200 });
    scheduler.push(1000);
    assert.equal(scheduler.takeFlush(1200), true);
    assert.equal(scheduler.push(1250), 1450, "post-flush push starts a fresh window");
    assert.equal(scheduler.takeFlush(1450), true);
});

test("push while hidden never arms a flush — no render until reveal", () => {
    const scheduler = createRenderScheduler({ windowMs: 200 });
    assert.equal(scheduler.setVisible(false, 1000), false, "hiding never triggers a render");
    assert.equal(scheduler.push(1000), null, "hidden push only marks dirty");
    assert.equal(scheduler.push(5000), null);
    assert.equal(scheduler.takeFlush(9000), false, "no flush can fire while hidden");
});

test("reveal with a dirty buffer flushes immediately — one snapshot burst, one render", () => {
    const scheduler = createRenderScheduler({ windowMs: 200 });
    scheduler.setVisible(false, 1000);
    scheduler.push(1000);
    scheduler.push(1100);
    scheduler.push(1200);
    assert.equal(scheduler.setVisible(true, 4000), true, "reveal renders the accumulated buffer at once");
    assert.equal(scheduler.takeFlush(4001), false, "nothing left to flush after the reveal render");
});

test("reveal with a clean buffer does not render", () => {
    const scheduler = createRenderScheduler({ windowMs: 200 });
    scheduler.push(1000);
    assert.equal(scheduler.takeFlush(1200), true);
    assert.equal(scheduler.setVisible(false, 1300), false);
    assert.equal(scheduler.setVisible(true, 1400), false, "nothing dirty on reveal — skip the iframe reload");
});

test("markRendered clears dirty state — the initial full render counts as a flush", () => {
    const scheduler = createRenderScheduler({ windowMs: 200 });
    scheduler.setVisible(false, 1000);
    scheduler.push(1000);
    scheduler.markRendered();
    assert.equal(scheduler.setVisible(true, 2000), false, "buffer already rendered on panel creation");
});

test("appendBounded keeps the ring capped at 500 entries, dropping oldest first", () => {
    const entries: number[] = [];
    for (let i = 0; i < MAX_ENTRIES + 100; i++) {
        appendBounded(entries, i, MAX_ENTRIES);
    }
    assert.equal(entries.length, MAX_ENTRIES);
    assert.equal(entries[0], 100, "the oldest 100 entries were dropped");
    assert.equal(entries[MAX_ENTRIES - 1], MAX_ENTRIES + 99, "the newest entry is retained");
});
