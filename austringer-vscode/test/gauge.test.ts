import { test } from "node:test";
import assert from "node:assert/strict";

import { gaugeState, type GaugeInputs } from "../src/gauge.ts";

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
