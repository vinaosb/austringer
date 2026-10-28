// Drift + parse tests for the web port of the shared command grammar
// (todo 23). The Rust table in austringer-daemon/src/grammar.rs is the
// source of truth; grammar.json is its byte-pinned serialization. This
// file FAILS if the JS table drifts from it in any direction.
//   node --test tests/js/grammar.test.mjs
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import {
  COMMANDS,
  ALLOW_SCOPES,
  VOCABULARY,
  cheatSheet,
  unknownHelp,
  localOnlyNote,
  parseCommand,
  routeCommand,
  modeOn,
  voteCancelFeedback,
  NO_OPEN_ASK,
} from "../../assets/grammar.js";

const grammarJson = JSON.parse(
  readFileSync(
    join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "austringer-daemon", "src", "grammar.json"),
    "utf8",
  ),
);

test("grammar: JS table is pinned byte-for-byte to grammar.json", () => {
  assert.deepEqual(grammarJson.commands, COMMANDS);
  assert.deepEqual(grammarJson.allow_scopes, ALLOW_SCOPES);
  assert.deepEqual(grammarJson.vocabulary, VOCABULARY);
});

test("grammar: cheat sheet renders one line per command with cli-only marks", () => {
  assert.deepEqual(cheatSheet().split("\n"), [
    "/help — show this command list",
    "/steer <text> — steer the running turn with more instructions",
    "/cancel — cancel the running turn",
    "/vote-cancel [ask-id] — withdraw an open permission ask (no id = the single open ask)",
    "/mode plan|off — enter or exit plan mode",
        "/rewind [steps] — rewind to a previous restore point (default 1)",
        "/skill <name> [args] — run a trusted skill through the submit lane",
        "/condense — condense the session context now",
        "/exit — quit this repl (cli only)",
    "/image <path> — attach an image to the next submit (cli only)",
  ]);
});

test("grammar: unknown-command help is the cross-frontend contract string", () => {
  assert.equal(unknownHelp("model"), "unknown command /model — /help lists commands");
  assert.equal(localOnlyNote("exit"), "/exit is cli-only");
});

test("grammar: parse recognizes every core command", () => {
  assert.deepEqual(parseCommand("fix the bug"), { kind: "chat", text: "fix the bug" });
  assert.deepEqual(parseCommand("/help"), { kind: "help" });
  assert.deepEqual(parseCommand("/help extra"), { kind: "help" });
  assert.deepEqual(parseCommand("/steer tighter types"), { kind: "steer", text: "tighter types" });
  assert.deepEqual(parseCommand("/cancel"), { kind: "cancel" });
  assert.deepEqual(parseCommand("/cancel now"), { kind: "cancel" });
  assert.deepEqual(parseCommand("/vote-cancel"), { kind: "vote-cancel", askId: null });
  assert.deepEqual(parseCommand("/vote-cancel 7"), { kind: "vote-cancel", askId: 7 });
  assert.deepEqual(parseCommand("/mode plan"), { kind: "mode", name: "plan" });
  assert.deepEqual(parseCommand("/mode off"), { kind: "mode", name: "off" });
  assert.deepEqual(parseCommand("/rewind"), { kind: "rewind", steps: null });
  assert.deepEqual(parseCommand("/rewind 3"), { kind: "rewind", steps: 3 });
  assert.deepEqual(parseCommand("/skill audit"), { kind: "skill", text: "/skill audit" });
  assert.deepEqual(parseCommand("/skill audit run twice"), { kind: "skill", text: "/skill audit run twice" });
  assert.deepEqual(parseCommand("/condense"), { kind: "condense" });
});

test("grammar: malformed and unknown commands are typed, never silent submits", () => {
  assert.deepEqual(parseCommand("/steer"), { kind: "malformed", usage: "/steer <text>" });
  assert.deepEqual(parseCommand("/mode"), { kind: "malformed", usage: "/mode plan|off" });
  assert.deepEqual(parseCommand("/rewind x"), { kind: "malformed", usage: "/rewind [steps]" });
  assert.deepEqual(parseCommand("/vote-cancel x"), { kind: "malformed", usage: "/vote-cancel [ask-id]" });
  assert.deepEqual(parseCommand("/skill"), { kind: "malformed", usage: "/skill <name> [args]" });
  assert.deepEqual(parseCommand("/model sonnet"), { kind: "unknown", word: "model" });
  assert.equal(parseCommand("/model sonnet").word, "model");
});

test("grammar: cli-local words are known-but-local everywhere", () => {
  assert.deepEqual(parseCommand("/exit"), { kind: "cli-only", word: "exit" });
  assert.deepEqual(parseCommand("/image pic.png"), { kind: "cli-only", word: "image" });
});

test("grammar: mode arguments map and vote-cancel feedback is typed", () => {
  assert.equal(modeOn("plan"), true);
  assert.equal(modeOn("off"), false);
  assert.equal(modeOn("exit"), false);
  assert.equal(modeOn("auto"), null);
  assert.deepEqual(voteCancelFeedback([]), { action: "note", text: NO_OPEN_ASK });
  assert.deepEqual(voteCancelFeedback([5]), { action: "cancel", id: 5 });
  const many = voteCancelFeedback([5, 9]);
  assert.equal(many.action, "note");
  assert.match(many.text, /^multiple open asks \(2\) — /);
});

test("grammar: routing maps every command onto its wire message (todo 23)", () => {
  assert.deepEqual(routeCommand("fix the bug", []), {
    sends: [{ type: "command", command: { type: "submit", text: "fix the bug" } }],
    feedback: null,
  });
  assert.deepEqual(routeCommand("/help", []), { sends: [], feedback: cheatSheet() });
  assert.deepEqual(routeCommand("/steer go", []), {
    sends: [{ type: "command", command: { type: "steer", text: "go" } }],
    feedback: null,
  });
  assert.deepEqual(routeCommand("/cancel", []), {
    sends: [{ type: "command", command: { type: "cancel" } }],
    feedback: null,
  });
  assert.deepEqual(routeCommand("/vote-cancel 7", []), {
    sends: [{ type: "permission_cancel", ask_id: 7 }],
    feedback: null,
  });
  assert.deepEqual(routeCommand("/vote-cancel", [3]), {
    sends: [{ type: "permission_cancel", ask_id: 3 }],
    feedback: null,
  });
  assert.deepEqual(routeCommand("/vote-cancel", []), { sends: [], feedback: NO_OPEN_ASK });
  assert.deepEqual(routeCommand("/mode plan", []), {
    sends: [{ type: "plan_mode", on: true }],
    feedback: null,
  });
  assert.deepEqual(routeCommand("/mode off", []), {
    sends: [{ type: "plan_mode", on: false }],
    feedback: null,
  });
  assert.deepEqual(routeCommand("/rewind", []), {
    sends: [{ type: "rewind", steps: 1 }],
    feedback: null,
  });
  assert.deepEqual(routeCommand("/rewind 3", []), {
    sends: [{ type: "rewind", steps: 3 }],
    feedback: null,
  });
  assert.deepEqual(routeCommand("/skill audit now", []), {
    sends: [{ type: "command", command: { type: "submit", text: "/skill audit now" } }],
    feedback: null,
  });
  assert.deepEqual(routeCommand("/condense", []), {
    sends: [{ type: "command", command: { type: "submit", text: "/condense" } }],
    feedback: null,
  });
  assert.deepEqual(routeCommand("/exit", []), { sends: [], feedback: "/exit is cli-only" });
  assert.deepEqual(routeCommand("/steer", []), { sends: [], feedback: "/steer <text>" });
  assert.deepEqual(routeCommand("/model x", []), {
    sends: [],
    feedback: "unknown command /model — /help lists commands",
  });
});
