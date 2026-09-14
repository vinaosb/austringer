// Unit tests for client-side share links (todo 37): redaction,
// expiry (410 Gone semantics), and the URL-safe codec. Run:
//   node --test tests/js
import test from "node:test";
import assert from "node:assert/strict";
import { mintShare, parseShare } from "../../assets/share.js";

function fixtureState() {
  return {
    session: "demo",
    lastEventId: 12,
    transcript: [
      { role: "user", text: "please fix the parser" },
      { role: "assistant", text: "working on it" },
      {
        role: "tool_call",
        tool: "write_file",
        text: "SECRET-TARGET-PATH reveal",
        arguments: { path: "C:/secret/key.pem", mode: 0o777 },
      },
      { role: "tool_result", text: "SECRET-OUTPUT blob" },
      { role: "user", text: "x".repeat(400) },
    ],
    hud: { turns: 3, events: 12, toolCalls: { write_file: 1 } },
  };
}

test("share: roundtrip preserves session, hud summary, and transcript text", () => {
  const link = mintShare(fixtureState(), 60_000);
  const out = parseShare(link);
  assert.equal(out.status, "ok");
  assert.equal(out.data.session, "demo");
  assert.equal(out.data.hud.turns, 3);
  assert.deepEqual(
    out.data.transcript.map((t) => t.role),
    ["user", "assistant", "tool_call", "tool_result", "user"]
  );
  assert.equal(out.data.transcript[0].text, "please fix the parser");
});

test("share: link body is URL-safe base64 (fragment-safe, no padding)", () => {
  const link = mintShare(fixtureState(), 60_000);
  const body = link.replace("#share=", "");
  assert.match(body, /^[A-Za-z0-9_-]+$/);
});

test("share: expired links report gone (the UI renders 410)", () => {
  const link = mintShare(fixtureState(), -1); // already expired
  assert.equal(parseShare(link).status, "gone");
});

test("share: redaction strips tool arguments, outputs, and long text", () => {
  const out = parseShare(mintShare(fixtureState(), 60_000));
  const serialized = JSON.stringify(out.data);
  assert.ok(!serialized.includes("SECRET"), "secrets must not ride the link");
  assert.ok(!serialized.includes("key.pem"));
  assert.equal(out.data.transcript[4].text.length, 200, "texts truncate at 200 chars");
  assert.equal(out.data.transcript[2].arguments, undefined);
});

test("share: CJK survives the codec", () => {
  const state = fixtureState();
  state.transcript[0].text = "你好，世界 🌏";
  const out = parseShare(mintShare(state, 60_000));
  assert.equal(out.data.transcript[0].text, "你好，世界 🌏");
});

test("share: tampered or garbage links report invalid, never throw", () => {
  assert.equal(parseShare("#share=!!!not-base64!!!").status, "invalid");
  assert.equal(parseShare("").status, "invalid");
  assert.equal(parseShare("#share=YWJj").status, "invalid", "non-JSON payload");
});
