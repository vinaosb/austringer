/**
 * VS Code port of the shared command grammar (todo 23). The Rust
 * table in harness-daemon/src/grammar.rs is the source of truth; this
 * module is pinned byte-for-byte to harness-daemon/src/grammar.json by
 * test/grammar.test.ts. Pure: no vscode imports.
 */

export interface SlashCommand {
    readonly word: string;
    readonly usage: string;
    readonly help: string;
    readonly local: boolean;
}

export const COMMANDS: readonly SlashCommand[] = [
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
    { word: "skill", usage: "/skill <name> [args]", help: "run a trusted skill through the submit lane", local: false },
    { word: "exit", usage: "/exit", help: "quit this repl", local: true },
    { word: "image", usage: "/image <path>", help: "attach an image to the next submit", local: true },
];

export const ALLOW_ONCE = "y (once)";
export const ALLOW_SESSION = "a (session)";
export const DENY = "n (deny)";
export const VOTE_CANCEL = "c (vote-cancel)";

export const ALLOW_SCOPES: readonly string[] = [ALLOW_ONCE, ALLOW_SESSION, DENY, VOTE_CANCEL];

export const VOCABULARY = {
    agents: "agents",
    rewind_point: "rewind point",
    share_link: "redacted share link",
    state_replayed: "state replayed",
} as const;

export const NO_OPEN_ASK = "no open ask to cancel";

function findCommand(word: string): SlashCommand | null {
    return COMMANDS.find((command) => command.word === word) ?? null;
}

export function cheatSheet(): string {
    return COMMANDS
        .map((command) =>
            command.local ? `${command.usage} — ${command.help} (cli only)` : `${command.usage} — ${command.help}`)
        .join("\n");
}

export function unknownHelp(word: string): string {
    return `unknown command /${word} — /help lists commands`;
}

export function localOnlyNote(word: string): string {
    return `/${word} is cli-only`;
}

export function modeOn(name: string): boolean | null {
    if (name === "plan") return true;
    if (name === "off" || name === "exit") return false;
    return null;
}

export function modeNote(name: string): string {
    return `/mode ${name}: daemon plan mode is the wired surface (/mode plan, /mode off); ` +
        "autonomy levels are engine-side";
}

/**
 * `/vote-cancel` without an id: the single open ask cancels; zero and
 * multiple note honestly (multiple points at this frontend's bound
 * surface, the permission prompt's vote-cancel pick).
 */
export function voteCancelFeedback(askIds: readonly number[]): { action: "cancel"; id: number } | { action: "note"; text: string } {
    if (askIds.length === 0) return { action: "note", text: NO_OPEN_ASK };
    const [first] = askIds;
    if (askIds.length === 1 && first !== undefined) return { action: "cancel", id: first };
    return {
        action: "note",
        text: `multiple open asks (${askIds.length}) — cancel from the permission prompt ` +
            "(c (vote-cancel)) or pass an id: /vote-cancel <ask-id>",
    };
}

function optionalCount(rest: string): { ok: true; value: number | null } | { ok: false } {
    if (rest === "") return { ok: true, value: null };
    return /^\d+$/.test(rest) ? { ok: true, value: Number(rest) } : { ok: false };
}

export type GrammarIntent =
    | { kind: "empty" }
    | { kind: "chat"; text: string }
    | { kind: "help" }
    | { kind: "steer"; text: string }
    | { kind: "cancel" }
    | { kind: "vote-cancel"; askId: number | null }
    | { kind: "mode"; name: string }
    | { kind: "rewind"; steps: number | null }
    | { kind: "skill"; text: string }
    | { kind: "cli-only"; word: string }
    | { kind: "malformed"; usage: string }
    | { kind: "unknown"; word: string };

export function parseCommand(input: string): GrammarIntent {
    const trimmed = input.trim();
    if (trimmed === "") return { kind: "empty" };
    if (!trimmed.startsWith("/")) return { kind: "chat", text: trimmed };
    const parts = trimmed.slice(1).split(/\s+/);
    const word = parts.shift() ?? "";
    const argument = parts.join(" ").trim();
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
        case "exit":
        case "image":
            return { kind: "cli-only", word };
        default:
            return { kind: "unknown", word };
    }
}
