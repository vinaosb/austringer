/**
 * Omnibar grammar: the single input box maps typed input onto the
 * daemon's three session commands (submit / steer / cancel). Slash
 * commands are matched exactly; anything unknown is a typed usage
 * error, never a silent submit of "/typo fix the thing".
 */

export type OmnibarIntent =
    | { kind: "submit"; text: string }
    | { kind: "steer"; text: string }
    | { kind: "cancel" }
    | { kind: "empty" }
    | { kind: "unknown"; usage: string };

const USAGE = "commands: /steer <text> | /cancel — anything else submits a new turn";

export function parseOmnibar(input: string): OmnibarIntent {
    const trimmed = input.trim();
    if (trimmed === "") {
        return { kind: "empty" };
    }
    if (!trimmed.startsWith("/")) {
        return { kind: "submit", text: trimmed };
    }
    const [command, ...rest] = trimmed.split(/\s+/);
    const argument = rest.join(" ").trim();
    switch (command) {
        case "/steer":
            return argument === ""
                ? { kind: "unknown", usage: "usage: /steer <text>" }
                : { kind: "steer", text: argument };
        case "/cancel":
            return { kind: "cancel" };
        default:
            return { kind: "unknown", usage: USAGE };
    }
}
