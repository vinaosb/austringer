/**
 * Omnibar grammar + dispatch (todo 23): the single input box maps
 * typed input onto the shared daemon grammar. Every core command
 * parses typed; anything unknown is the shared help string, never a
 * silent submit of "/typo fix the thing". `runOmnibar` is pure over
 * its deps (headless-testable); session.ts only wires vscode surfaces.
 */

import { cheatSheet, localOnlyNote, modeNote, modeOn, parseCommand, unknownHelp, voteCancelFeedback } from "./grammar.ts";

export type OmnibarIntent =
    | { kind: "submit"; text: string }
    | { kind: "steer"; text: string }
    | { kind: "cancel" }
    | { kind: "vote-cancel"; askId: number | null }
    | { kind: "mode"; on: boolean }
    | { kind: "rewind"; steps: number }
    | { kind: "skill"; text: string }
    | { kind: "help" }
    | { kind: "empty" }
    | { kind: "malformed"; usage: string }
    | { kind: "unknown"; help: string }
    | { kind: "cli-only"; word: string };

export function parseOmnibar(input: string): OmnibarIntent {
    const parsed = parseCommand(input);
    switch (parsed.kind) {
        case "empty":
            return { kind: "empty" };
        case "chat":
            return { kind: "submit", text: parsed.text };
        case "help":
            return { kind: "help" };
        case "steer":
            return { kind: "steer", text: parsed.text };
        case "cancel":
            return { kind: "cancel" };
        case "vote-cancel":
            return { kind: "vote-cancel", askId: parsed.askId };
        case "mode": {
            const on = modeOn(parsed.name);
            return on === null
                ? { kind: "malformed", usage: modeNote(parsed.name) }
                : { kind: "mode", on };
        }
        case "rewind":
            return { kind: "rewind", steps: parsed.steps ?? 1 };
        case "skill":
            return { kind: "skill", text: parsed.text };
        case "cli-only":
            return { kind: "cli-only", word: parsed.word };
        case "malformed":
            return { kind: "malformed", usage: parsed.usage };
        case "unknown":
            return { kind: "unknown", help: unknownHelp(parsed.word) };
    }
}

/** Typed command outcome (the austringer client's CommandResult shape). */
export interface OmnibarOutcome {
    ok: boolean;
    error?: { kind: string };
}

/** Surfaces `runOmnibar` needs: the daemon client plus vscode toasts. */
export interface OmnibarDeps {
    submit(text: string): Promise<OmnibarOutcome>;
    submitVerbatim(text: string): Promise<OmnibarOutcome>;
    steer(text: string): Promise<OmnibarOutcome>;
    cancelTurn(): Promise<OmnibarOutcome>;
    cancelAsk(askId: number): Promise<OmnibarOutcome>;
    planMode(on: boolean): Promise<OmnibarOutcome>;
    rewind(steps: number): Promise<OmnibarOutcome>;
    openAskIds(): number[];
    showInfo(message: string): void;
    showWarn(message: string): void;
    showError(message: string): void;
}

async function report(result: OmnibarOutcome, deps: OmnibarDeps): Promise<void> {
    if (!result.ok && result.error !== undefined) {
        deps.showError(`Austringer: command failed (${result.error.kind})`);
    }
}

export async function runOmnibar(input: string, deps: OmnibarDeps): Promise<void> {
    const intent = parseOmnibar(input);
    switch (intent.kind) {
        case "empty":
            return;
        case "submit":
            await report(await deps.submit(intent.text), deps);
            return;
        case "steer":
            await report(await deps.steer(intent.text), deps);
            return;
        case "cancel":
            await report(await deps.cancelTurn(), deps);
            return;
        case "vote-cancel": {
            const resolved = intent.askId !== null
                ? { action: "cancel" as const, id: intent.askId }
                : voteCancelFeedback(deps.openAskIds());
            if (resolved.action === "cancel") {
                await report(await deps.cancelAsk(resolved.id), deps);
            } else {
                deps.showWarn(resolved.text);
            }
            return;
        }
        case "mode":
            await report(await deps.planMode(intent.on), deps);
            return;
        case "rewind":
            await report(await deps.rewind(intent.steps), deps);
            return;
        case "skill":
            // /skill rides the submit lane verbatim (the daemon-side
            // trust gate parses it there); no telemetry context.
            await report(await deps.submitVerbatim(intent.text), deps);
            return;
        case "help":
            deps.showInfo(cheatSheet());
            return;
        case "malformed":
            deps.showWarn(intent.usage);
            return;
        case "unknown":
            deps.showWarn(intent.help);
            return;
        case "cli-only":
            deps.showWarn(localOnlyNote(intent.word));
            return;
    }
}
