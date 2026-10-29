import { test } from "node:test";
import assert from "node:assert/strict";

import { createGaugeView, gaugeState, type GaugeInputs } from "../src/gauge.ts";

const base: GaugeInputs = {
    status: "connected",
    attempt: 0,
    turn: null,
    pendingAsks: 0,
};

test("connected and idle shows the session is live", () => {
    const gauge = gaugeState(base);
    assert.match(gauge.text, /connected/i);
    assert.equal(gauge.kind, "ok");
});

test("mid-turn shows the turn number", () => {
    const gauge = gaugeState({ ...base, turn: 3 });
    assert.match(gauge.text, /turn 3/i);
});

test("a pending permission ask outranks the turn in the gauge", () => {
    const gauge = gaugeState({ ...base, turn: 3, pendingAsks: 2 });
    assert.match(gauge.text, /2.*ask|ask.*2/i);
    assert.equal(gauge.kind, "action");
});

test("connecting shows the retry attempt count", () => {
    const gauge = gaugeState({ ...base, status: "connecting", attempt: 4 });
    assert.match(gauge.text, /4/);
    assert.equal(gauge.kind, "warn");
});

test("disconnected says so plainly", () => {
    const gauge = gaugeState({ ...base, status: "disconnected" });
    assert.match(gauge.text, /disconnected|offline/i);
    assert.equal(gauge.kind, "error");
});

test("createGaugeView applies the first render even when inputs repeat", () => {
    const applied: string[] = [];
    const view = createGaugeView((state) => applied.push(state.text));
    view(base);
    view(base);
    assert.equal(applied.length, 1, "identical inputs after the first render are swallowed");
    assert.match(applied[0] ?? "", /connected/i);
});

test("createGaugeView early-returns while event churn keeps the state unchanged", () => {
    let applications = 0;
    const view = createGaugeView(() => applications++);
    for (let attempt = 0; attempt <= 5; attempt++) {
        view({ ...base, status: "connected", attempt });
    }
    assert.equal(applications, 1, "same text/kind at different attempt counts is one render");
});

test("createGaugeView re-applies when the visible state changes", () => {
    const kinds: string[] = [];
    const view = createGaugeView((state) => kinds.push(state.kind));
    view(base);
    view({ ...base, turn: 1 });
    view({ ...base, turn: 1, pendingAsks: 1 });
    view({ ...base, turn: 1, pendingAsks: 1 });
    assert.deepEqual(
        kinds,
        ["ok", "ok", "action"],
        "turn change alters text (re-applies), ask change alters kind, repeat is swallowed",
    );
});
