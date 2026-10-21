// Unit tests for the SPA store reducer (todo 37). The store is a
// total, non-throwing function over server messages: every input,
// including malformed frames, maps to a next state. Run:
//   node --test tests/js
import test from "node:test";
import assert from "node:assert/strict";
import { createStore } from "../../assets/store.js";
import { decode } from "../../assets/proto.js";

// The store consumes wire strings, exactly what the transport hands it.
function feed(store, msg) {
  store.dispatch(JSON.stringify(msg));
}

function frame(id, event) {
  return { type: "event", frame: { id, event } };
}

const USER = (text) => ({ type: "user", blocks: [{ type: "text", text }] });
const ASSISTANT = (text) => ({ type: "assistant", blocks: [{ type: "text", text }] });
const TOOL_CALL = { type: "tool_call", call_id: "t1", tool: "echo", arguments: { q: 1 } };
const TOOL_RESULT = (text) => ({
  type: "tool_result",
  call_id: "t1",
  blocks: [{ type: "text", text }],
});

test("store: happy stream renders transcript, HUD, diff, and ask lifecycle", () => {
  // Given a fresh store fed a complete happy-path stream in the exact
  // wire shapes the daemon emits (session events ride frames).
  const s = createStore();
  feed(s, { type: "hello_ack", version: 1, last_event_id: 0 });
  feed(s, frame(1, { type: "turn_started", turn: 0 }));
  feed(s, frame(2, { type: "recorded", seq: 1, event: USER("hello world") }));
  feed(s, frame(3, { type: "recorded", seq: 2, event: TOOL_CALL }));
  feed(s, {
    type: "permission_ask",
    ask: {
      ask_id: 7,
      tool: "echo",
      target: "src/main.rs",
      reason: "mutating tool",
      deadline_ms: 30000,
    },
  });
  feed(s, {
    type: "permission_resolved",
    ask_id: 7,
    resolution: { type: "allowed", by: "web-1" },
  });
  feed(s, frame(4, { type: "recorded", seq: 3, event: TOOL_RESULT("done") }));
  feed(s, frame(5, {
    type: "diff",
    path: "src/main.rs",
    before_hash: "",
    after_hash: "abc",
    hunks: ["@@ -1 +1 @@\n-old\n+new"],
  }));
  feed(s, frame(6, { type: "turn_finished", turn: 0, outcome: "task_completed" }));

  // Then the derived state is exactly what the UI renders.
  const st = s.getState();
  assert.equal(st.hello.version, 1);
  assert.equal(st.hud.turns, 1);
  assert.equal(st.hud.events, 6);
  assert.equal(st.hud.llmVisible, 3);
  assert.equal(st.hud.toolCalls.echo, 1);
  assert.equal(st.hud.results, 1);
  assert.deepEqual(st.hud.outcomes, ["task_completed"]);
  assert.equal(st.diffs.length, 1);
  assert.equal(st.diffs[0].path, "src/main.rs");
  assert.equal(Object.keys(st.asks).length, 0, "answered ask leaves the pending set");
  assert.equal(st.resolvedLog.length, 1);
  assert.equal(st.lastEventId, 6);
  assert.deepEqual(
    st.transcript.map((t) => t.role),
    ["user", "tool_call", "tool_result"]
  );
});

test("store: CJK and emoji text survive the reducer byte-for-byte", () => {
  const s = createStore();
  const text = "你好，世界 🌏 — naïve résumé";
  feed(s, frame(1, { type: "recorded", seq: 1, event: USER(text) }));
  assert.equal(s.getState().transcript[0].text, text);
});

test("store: malformed and unknown frames never throw; they count as anomalies", () => {
  const s = createStore();
  s.dispatch("this is not json");
  feed(s, { type: "nonsense" });
  feed(s, frame(1, { type: "recorded", seq: 1, event: USER("ok") }));
  const st = s.getState();
  assert.equal(st.hud.anomalies, 2);
  assert.equal(st.transcript.length, 1, "valid frames around garbage still land");
});

test("store: state_resync_required + snapshot replaces history entirely", () => {
  const s = createStore();
  feed(s, frame(1, { type: "recorded", seq: 1, event: USER("stale") }));
  feed(s, { type: "state_resync_required", last_event_id: 9 });
  feed(s, {
    type: "snapshot",
    frames: [
      { id: 8, event: { type: "recorded", seq: 1, event: USER("fresh-a") } },
      { id: 9, event: { type: "recorded", seq: 2, event: ASSISTANT("fresh-b") } },
    ],
  });
  const st = s.getState();
  assert.deepEqual(
    st.transcript.map((t) => t.text),
    ["fresh-a", "fresh-b"],
    "stale pre-resync history is gone"
  );
  assert.equal(st.lastEventId, 9);
  assert.equal(st.hud.resyncs, 1);
});

test("store: auth error raises the unauthorized flag; bye marks transport closed", () => {
  const s = createStore();
  feed(s, { type: "error", code: "auth", message: "bad token" });
  assert.equal(s.getState().unauthorized, true);
  feed(s, { type: "bye", reason: "idle read timeout" });
  assert.equal(s.getState().closedBy, "idle read timeout");
});

test("store: cancel-vote resolution withdraws the ask without allowing it", () => {
  const s = createStore();
  feed(s, {
    type: "permission_ask",
    ask: { ask_id: 3, tool: "write", reason: "r", deadline_ms: 1000 },
  });
  feed(s, {
    type: "permission_resolved",
    ask_id: 3,
    resolution: { type: "cancelled", by: "web-2" },
  });
  const st = s.getState();
  assert.equal(Object.keys(st.asks).length, 0);
  assert.equal(st.resolvedLog[0].resolution.type, "cancelled");
});

test("store: image blocks render as their typed text marker", () => {
  const s = createStore();
  feed(s, frame(1, {
    type: "recorded",
    seq: 1,
    event: {
      type: "user",
      blocks: [
        { type: "text", text: "look:" },
        {
          type: "image",
          mime: "image/png",
          width: 800,
          height: 600,
          media: { hash: "abcd1234efgh", len: 10 },
        },
      ],
    },
  }));
  assert.equal(
    s.getState().transcript[0].text,
    "look:\n[image: image/png 800x600 abcd1234]"
  );
});

test("store: team frames build workers, budget, and exhaustion state", () => {
  // Given a v2 stream carrying the todo-15 team frames (typed Child*
  // events plus the delegation budget events).
  const s = createStore();
  feed(s, frame(1, {
    type: "child_started",
    child: "p-child-task1",
    task: "quick",
    role: "worker",
    model: "claude-sonnet-4-6",
  }));
  feed(s, frame(2, {
    type: "child_progress",
    child: "p-child-task1",
    phase: "blocked",
    detail: "permission: edit_lines src/lib.rs",
  }));
  feed(s, frame(3, {
    type: "team_budget",
    spawned: 1,
    tokens_spent: 0,
    fan_out_cap: 4,
    token_budget: 1000,
  }));
  feed(s, frame(4, {
    type: "child_started",
    child: "p-child-task2",
    task: "deep",
    role: "worker",
    model: "claude-opus-4-7",
  }));
  feed(s, frame(5, {
    type: "child_finished",
    child: "p-child-task2",
    task: "deep",
    state: "success",
    detail: "ok",
  }));
  feed(s, frame(6, {
    type: "team_budget",
    spawned: 2,
    tokens_spent: 30,
    fan_out_cap: 4,
    token_budget: 1000,
  }));
  feed(s, frame(7, {
    type: "team_budget_exhausted",
    spent: 1000,
    budget: 1000,
  }));

  // Then the workers panel input is exactly the reduced team state.
  const st = s.getState();
  assert.equal(st.workers.length, 2);
  assert.equal(st.workers[0].state, "blocked");
  assert.equal(st.workers[0].detail, "permission: edit_lines src/lib.rs");
  assert.equal(st.workers[1].state, "success");
  assert.equal(st.budget.spawned, 2);
  assert.equal(st.budget.tokensSpent, 30);
  assert.equal(st.budget.fanOutCap, 4);
  assert.equal(st.budget.tokenBudget, 1000);
  assert.equal(st.budgetExhausted, true);
  assert.equal(st.hud.anomalies, 0, "team frames are known shapes, not anomalies");
});

test("proto: decode is total — garbage in, typed failure out", () => {
  assert.equal(decode("not json").ok, false);
  assert.equal(decode('{"type":42}').ok, false);
  const good = decode('{"type":"pong"}');
  assert.equal(good.ok, true);
  assert.equal(good.value.type, "pong");
});
