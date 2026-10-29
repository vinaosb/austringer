// Web port of the shared command grammar (todo 23). The Rust table
// in austringer-daemon/src/grammar.rs is the source of truth; this module
// is pinned byte-for-byte to austringer-daemon/src/grammar.json by
// tests/js/grammar.test.mjs. Pure: no DOM, no transport.
export const COMMANDS = [
  { word: "help", usage: "/help", help: "show this command list", local: false },
  { word: "steer", usage: "/steer <text>", help: "steer the running turn with more instructions", local: false },
  { word: "cancel", usage: "/cancel", help: "cancel the running turn", local: false },
  {
    word: "vote-cancel",
    usage: "/vote-cancel [ask-id]",
    help: "withdraw an open permission ask (no id = the single open ask)",
    local: false,
  },
  { word: "mode", usage: "/mode plan|off", help: "enter or exit plan mode", local: false },
  { word: "rewind", usage: "/rewind [steps]", help: "rewind to a previous restore point (default 1)", local: false },
  {
    word: "skill",
    usage: "/skill <name> [args]",
    help: "run a trusted skill through the submit lane",
    local: false,
  },
  { word: "condense", usage: "/condense", help: "condense the session context now", local: false },
  { word: "exit", usage: "/exit", help: "quit this repl", local: true },
  { word: "image", usage: "/image <path>", help: "attach an image to the next submit", local: true },
];

export const ALLOW_SCOPES = ["y (once)", "a (session)", "n (deny)", "c (vote-cancel)"];

export const VOCABULARY = {
  agents: "agents",
  rewind_point: "rewind point",
  share_link: "redacted share link",
  state_replayed: "state replayed",
};

export const NO_OPEN_ASK = "no open ask to cancel";

function findCommand(word) {
  return COMMANDS.find((command) => command.word === word) || null;
}

export function cheatSheet() {
  return COMMANDS
    .map((command) =>
      command.local ? `${command.usage} — ${command.help} (cli only)` : `${command.usage} — ${command.help}`)
    .join("\n");
}

export function unknownHelp(word) {
  return `unknown command /${word} — /help lists commands`;
}

export function localOnlyNote(word) {
  return `/${word} is cli-only`;
}

export function modeOn(name) {
  if (name === "plan") return true;
  if (name === "off" || name === "exit") return false;
  return null;
}

export function modeNote(name) {
  return `/mode ${name}: daemon plan mode is the wired surface (/mode plan, /mode off); ` +
    "autonomy levels are engine-side";
}

// `/vote-cancel` without an id: the single open ask cancels; zero and
// multiple note honestly (multiple points at this frontend's bound
// surface, the vote-cancel button on each ask card).
export function voteCancelFeedback(askIds) {
  if (askIds.length === 0) return { action: "note", text: NO_OPEN_ASK };
  if (askIds.length === 1) return { action: "cancel", id: askIds[0] };
  return {
    action: "note",
    text: `multiple open asks (${askIds.length}) — use the vote-cancel button on the ask, ` +
      "or pass an id: /vote-cancel <ask-id>",
  };
}

function optionalCount(rest) {
  if (rest === "") return { ok: true, value: null };
  const parsed = /^(\d+)$/.exec(rest);
  return parsed === null ? { ok: false } : { ok: true, value: Number(parsed[1]) };
}

// Parse one composer line against the shared table. Returns a typed
// intent; malformed carries the table usage, unknown carries the word
// (render with unknownHelp).
export function parseCommand(input) {
  const trimmed = input.trim();
  if (trimmed === "") return { kind: "empty" };
  if (!trimmed.startsWith("/")) return { kind: "chat", text: trimmed };
  const [word, ...rest] = trimmed.slice(1).split(/\s+/);
  const argument = rest.join(" ").trim();
  const usage = findCommand(word)?.usage ?? "";
  switch (word) {
    case "help":
      return { kind: "help" };
    case "steer":
      return argument === "" ? { kind: "malformed", usage } : { kind: "steer", text: argument };
    case "cancel":
      return { kind: "cancel" };
    case "vote-cancel": {
      const count = optionalCount(argument);
      return count.ok ? { kind: "vote-cancel", askId: count.value } : { kind: "malformed", usage };
    }
    case "mode":
      return argument === "" ? { kind: "malformed", usage } : { kind: "mode", name: argument };
    case "rewind": {
      const count = optionalCount(argument);
      return count.ok ? { kind: "rewind", steps: count.value } : { kind: "malformed", usage };
    }
    case "skill":
      if (argument === "") return { kind: "malformed", usage };
      return { kind: "skill", text: `/skill ${argument}` };
    case "condense":
      return { kind: "condense" };
    case "exit":
    case "image":
      return { kind: "cli-only", word };
    default:
      return { kind: "unknown", word };
  }
}

// Route one composer line to wire messages + composer feedback
// (todo 23). Pure: the caller sends `sends` in order and renders
// `feedback` (null clears it). `openAskIds` resolves /vote-cancel
// without an id.
export function routeCommand(input, openAskIds) {
  const intent = parseCommand(input);
  switch (intent.kind) {
    case "empty":
      return { sends: [], feedback: null };
    case "chat":
      return { sends: [{ type: "command", command: { type: "submit", text: intent.text } }], feedback: null };
    case "help":
      return { sends: [], feedback: cheatSheet() };
    case "steer":
      return { sends: [{ type: "command", command: { type: "steer", text: intent.text } }], feedback: null };
    case "cancel":
      return { sends: [{ type: "command", command: { type: "cancel" } }], feedback: null };
    case "vote-cancel": {
      const resolved = intent.askId !== null
        ? { action: "cancel", id: intent.askId }
        : voteCancelFeedback(openAskIds);
      if (resolved.action !== "cancel") return { sends: [], feedback: resolved.text };
      return { sends: [{ type: "permission_cancel", ask_id: resolved.id }], feedback: null };
    }
    case "mode": {
      const on = modeOn(intent.name);
      if (on === null) return { sends: [], feedback: modeNote(intent.name) };
      return { sends: [{ type: "plan_mode", on }], feedback: null };
    }
    case "rewind":
      return { sends: [{ type: "rewind", steps: intent.steps ?? 1 }], feedback: null };
    case "skill":
      // /skill rides the submit lane verbatim (the daemon-side trust
      // gate parses it there).
      return { sends: [{ type: "command", command: { type: "submit", text: intent.text } }], feedback: null };
    case "condense":
      // /condense rides the submit lane verbatim too: the daemon-side
      // slash stage runs the compaction.
      return { sends: [{ type: "command", command: { type: "submit", text: "/condense" } }], feedback: null };
    case "cli-only":
      return { sends: [], feedback: localOnlyNote(intent.word) };
    case "malformed":
      return { sends: [], feedback: intent.usage };
    case "unknown":
      return { sends: [], feedback: unknownHelp(intent.word) };
  }
}
