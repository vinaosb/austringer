// Unit tests for the WS attach cursor (wave-5 reconnect fix). The
// WebSocket attach URL must carry the `after` replay cursor on
// reconnect — exactly like the SSE fallback and the TUI client
// (austringer-tui/src/client.rs ws_attach_url) — so the daemon replays
// a ~0-frame delta instead of forcing a full Resync snapshot on every
// WS reconnect. Run (explicit files; `node --test <dir>` fails on this
// Node):
//   node tests/js/proto.test.mjs
import test from "node:test";
import assert from "node:assert/strict";
import { connect, wsAttachUrl } from "../../assets/proto.js";
import { createStore } from "../../assets/store.js";

function frame(id, event) {
  return { type: "event", frame: { id, event } };
}

const USER = (text) => ({ type: "user", blocks: [{ type: "text", text }] });

// Minimal WebSocket double: captures every connect URL, records sends,
// and lets tests drive open/message/close by hand.
class FakeWebSocket {
  static created = [];
  constructor(url) {
    this.url = url;
    this.readyState = 0; // CONNECTING
    this.sent = [];
    this.listeners = new Map();
    FakeWebSocket.created.push(this);
  }
  addEventListener(type, fn) {
    if (!this.listeners.has(type)) this.listeners.set(type, []);
    this.listeners.get(type).push(fn);
  }
  dispatch(type, ev = {}) {
    for (const fn of this.listeners.get(type) ?? []) fn(ev);
  }
  send(raw) {
    this.sent.push(raw);
  }
  close() {
    this.readyState = 3; // CLOSED
  }
  static reset() {
    FakeWebSocket.created = [];
  }
}

// Stub the WebSocket global for one test (Node 25 ships a real client;
// the daemon never sees these fake connects). sessionStorage is stubbed
// too because clientId() persists per-tab state in browsers.
function stubBrowserGlobals(t) {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const realWebSocket = globalThis.WebSocket;
  const realSessionStorage = globalThis.sessionStorage;
  globalThis.WebSocket = FakeWebSocket;
  globalThis.sessionStorage = { getItem: () => null, setItem: () => {} };
  FakeWebSocket.reset();
  t.after(() => {
    globalThis.WebSocket = realWebSocket;
    globalThis.sessionStorage = realSessionStorage;
  });
}

function openAndSet(ports, id) {
  const socket = FakeWebSocket.created[id];
  socket.readyState = 1; // OPEN
  socket.dispatch("open");
  return socket;
}

test("proto: wsAttachUrl mirrors the TUI contract — cursorless at 0, &after=N past 0", () => {
  assert.equal(
    wsAttachUrl("127.0.0.1:9000", "s1", "tok", 0),
    "ws://127.0.0.1:9000/s/s1/attach?token=tok",
    "first attach stays cursor-less (daemon Resync contract)"
  );
  assert.equal(
    wsAttachUrl("127.0.0.1:9000", "s1", "tok", 41),
    "ws://127.0.0.1:9000/s/s1/attach?token=tok&after=41"
  );
  assert.equal(
    wsAttachUrl("127.0.0.1:9000", "s/1", "t ok", 2),
    "ws://127.0.0.1:9000/s/s%2F1/attach?token=t%20ok&after=2",
    "session and token stay encodeURIComponent-encoded"
  );
});

test("proto: ws reconnect carries the lastEventId cursor", (t) => {
  stubBrowserGlobals(t);
  const statuses = [];
  const controller = connect({
    base: "127.0.0.1:9000",
    token: "tok",
    session: "s1",
    onMessage: () => {},
    onStatus: (state) => statuses.push(state),
  });
  controller.start();
  assert.equal(FakeWebSocket.created.length, 1);
  assert.equal(
    FakeWebSocket.created[0].url,
    "ws://127.0.0.1:9000/s/s1/attach?token=tok",
    "first connect is cursor-less"
  );

  // Connect, observe an event at id 7, then drop the socket.
  const first = openAndSet(FakeWebSocket.created, 0);
  first.dispatch("message", {
    data: JSON.stringify(frame(7, { type: "turn_started", turn: 0 })),
  });
  first.dispatch("close");
  assert.ok(statuses.includes("reconnecting"));

  t.mock.timers.runAll(); // fire the 1s reconnect backoff
  assert.equal(FakeWebSocket.created.length, 2);
  assert.ok(
    FakeWebSocket.created[1].url.endsWith("&after=7"),
    `reconnect URL must carry the cursor: ${FakeWebSocket.created[1].url}`
  );
});

test("proto: reconnect with a delta leaves the store transcript intact (no snapshot reset)", (t) => {
  stubBrowserGlobals(t);
  const store = createStore();
  const controller = connect({
    base: "127.0.0.1:9000",
    token: "tok",
    session: "s1",
    onMessage: (raw) => store.dispatch(raw),
    onStatus: () => {},
  });
  controller.start();

  // Seed pre-drop history: ids 1..3, three transcript entries.
  const first = openAndSet(FakeWebSocket.created, 0);
  for (const [id, text] of [[1, "a"], [2, "b"], [3, "c"]]) {
    first.dispatch("message", {
      data: JSON.stringify(frame(id, { type: "recorded", seq: id, event: USER(text) })),
    });
  }
  assert.equal(store.getState().transcript.length, 3);

  // Drop + reconnect: cursor must be 3 so the daemon deltas, not resyncs.
  first.dispatch("close");
  t.mock.timers.runAll();
  const second = FakeWebSocket.created[1];
  assert.ok(second.url.endsWith("&after=3"));

  // Reconnect with a delta: hello_ack + a non-transcript event frame.
  second.readyState = 1;
  second.dispatch("open");
  second.dispatch("message", {
    data: JSON.stringify({ type: "hello_ack", version: 2, last_event_id: 3 }),
  });
  second.dispatch("message", {
    data: JSON.stringify(frame(4, { type: "turn_started", turn: 1 })),
  });

  const st = store.getState();
  assert.equal(st.transcript.length, 3, "delta reconnect must not reset the transcript");
  assert.deepEqual(st.transcript.map((entry) => entry.text), ["a", "b", "c"]);
  assert.equal(st.lastEventId, 4, "delta frames still advance the cursor");
});
