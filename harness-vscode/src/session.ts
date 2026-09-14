/**
 * The live editor-side session (plan todo 38): owns the daemon
 * client, projects its events onto the status-bar gauge, diff
 * decorations, pending permission prompts, the omnibar, and hunk
 * commands. Decisions live in headless-tested modules; this file is
 * VS Code glue only.
 */

import * as vscode from "vscode";

import { HarnessClient, type ClientEvent } from "./client.ts";
import type { AskPayload } from "./protocol.ts";
import { gaugeState, type GaugeInputs } from "./gauge.ts";
import { hunkAction, planDiff, gateHunkAccept, type HunkPlan, type HunkRange } from "./hunks.ts";
import { nodeHttpIo } from "./nodehttp.ts";
import { parseOmnibar } from "./omnibar.ts";
import { buildTelemetryContext, type EditorSnapshot } from "./telemetry.ts";

export class HarnessSession {
    private readonly client: HarnessClient;
    private readonly gauge: vscode.StatusBarItem;
    private readonly decoration = vscode.window.createTextEditorDecorationType({
        backgroundColor: new vscode.ThemeColor("diffEditor.insertedTextBackground"),
        isWholeLine: true,
    });
    private readonly plans = new Map<string, HunkPlan>();
    private readonly pendingAsks = new Map<number, AskPayload>();
    private gaugeInputs: GaugeInputs = { status: "disconnected", attempt: 0, turn: null, pendingAsks: 0 };
    private turn: number | null = null;

    constructor(client: HarnessClient, gauge: vscode.StatusBarItem) {
        this.client = client;
        this.gauge = gauge;
    }

    static async connect(gauge: vscode.StatusBarItem): Promise<HarnessSession> {
        const config = vscode.workspace.getConfiguration("harness");
        const token = await readDaemonToken(config.get<string>("daemonTokenFile") ?? ".harness-daemon/daemon-token");
        if (token === null) {
            throw new Error("daemon token file not found — start the harness daemon first");
        }
        const session = new HarnessSession(
            new HarnessClient({
                baseUrl: config.get<string>("daemonBaseUrl") ?? "http://127.0.0.1:8787",
                token,
                sessionId: config.get<string>("sessionId") ?? "vscode",
                clientId: `vscode-${process.pid}`,
                http: nodeHttpIo(),
                onEvent: (event) => session.handle(event),
            }),
            gauge,
        );
        return session;
    }

    get daemonToken(): string | null {
        return this.client.token;
    }

    start(): void {
        this.client.start();
    }

    async stop(): Promise<void> {
        this.gauge.hide();
        this.gauge.dispose();
        this.decoration.dispose();
        await this.client.stop();
    }

    handle(event: ClientEvent): void {
        switch (event.kind) {
            case "status":
                this.gaugeInputs = { ...this.gaugeInputs, status: event.status, attempt: event.attempt };
                break;
            case "frame":
                this.handleFrame(event.frame.event);
                break;
            case "ask":
                this.pendingAsks.set(event.ask.ask_id, event.ask);
                void this.promptAsk(event.ask);
                break;
            case "resolved":
                this.pendingAsks.delete(event.askId);
                break;
            case "snapshot":
                for (const frame of event.frames) {
                    this.handleFrame(frame.event);
                }
                break;
            case "protocol-error":
                if (event.code === "auth") {
                    void vscode.window.showErrorMessage(`Harness: ${event.message}`);
                }
                break;
            case "bye":
                void vscode.window.showInformationMessage(`Harness daemon said bye: ${event.reason}`);
                break;
            case "malformed":
            case "resync":
                break;
        }
        this.gaugeInputs = { ...this.gaugeInputs, pendingAsks: this.pendingAsks.size, turn: this.turn };
        this.renderGauge();
    }

    private handleFrame(event: import("./protocol.ts").SessionEvent): void {
        switch (event.type) {
            case "turn_started":
                this.turn = event.turn;
                break;
            case "turn_finished":
                this.turn = null;
                break;
            case "diff": {
                const plan = planDiff(event);
                this.plans.set(plan.path, plan);
                this.decorate();
                break;
            }
            default:
                break;
        }
    }

    renderGauge(): void {
        const state = gaugeState(this.gaugeInputs);
        this.gauge.text = state.text;
        this.gauge.tooltip = state.tooltip;
        this.gauge.backgroundColor = state.kind === "action"
            ? new vscode.ThemeColor("statusBarItem.warningBackground")
            : undefined;
        this.gauge.show();
    }

    decorate(): void {
        for (const editor of vscode.window.visibleTextEditors) {
            const plan = this.plans.get(relativePath(editor.document.uri));
            if (plan === undefined) {
                continue;
            }
            editor.setDecorations(
                this.decoration,
                plan.hunks.map((hunk) => new vscode.Range(hunk.startLine - 1, 0, hunk.endLine, 0)),
            );
        }
    }

    planFor(uri: vscode.Uri): HunkPlan | undefined {
        return this.plans.get(relativePath(uri));
    }

    hunkAt(path: string, header: string): HunkRange | undefined {
        return this.plans.get(path)?.hunks.find((hunk) => hunk.header === header);
    }

    pendingAskFor(path: string): AskPayload | null {
        for (const ask of this.pendingAsks.values()) {
            if (ask.target === path) {
                return ask;
            }
        }
        return null;
    }

    async answer(askId: number, answer: "allow_once" | "allow_session" | "deny"): Promise<void> {
        const result = await this.client.answerAsk(askId, answer);
        if (!result.ok && result.error.kind !== "unknown_ask") {
            void vscode.window.showErrorMessage(`Harness: answer failed (${result.error.kind})`);
        }
    }

    private async promptAsk(ask: AskPayload): Promise<void> {
        const pick = await vscode.window.showWarningMessage(
            `${ask.tool} → ${ask.target ?? "?"}: ${ask.reason}`,
            "Allow once",
            "Allow for session",
            "Deny",
        );
        const answer = pick === "Allow once" ? "allow_once" : pick === "Allow for session" ? "allow_session" : "deny";
        await this.answer(ask.ask_id, answer);
    }

    async omnibar(): Promise<void> {
        const input = await vscode.window.showInputBox({ prompt: "harness omnibar — /steer <text> | /cancel | text submits" });
        if (input === undefined) {
            return;
        }
        const intent = parseOmnibar(input);
        const telemetry = buildTelemetryContext(telemetryEnabled(), editorSnapshot());
        const context = telemetry === null ? "" : `\n\n${telemetry}`;
        const outcome = intent.kind === "submit"
            ? await this.client.submit(`${intent.text}${context}`)
            : intent.kind === "steer"
                ? await this.client.steer(intent.text)
                : intent.kind === "cancel"
                    ? await this.client.cancelTurn()
                    : null;
        if (outcome === null) {
            void vscode.window.showWarningMessage(intent.kind === "unknown" ? intent.usage : "nothing to send");
            return;
        }
        if (!outcome.ok) {
            void vscode.window.showErrorMessage(`Harness: command failed (${outcome.error.kind})`);
        }
    }

    async hunkCommand(path: string, header: string, accept: boolean): Promise<void> {
        const hunk = this.hunkAt(path, header);
        if (hunk === undefined) {
            return;
        }
        const editor = vscode.window.visibleTextEditors.find((candidate) => relativePath(candidate.document.uri) === path);
        const gate = gateHunkAccept({ path, dirty: editor?.document.isDirty ?? false });
        if (gate.action === "confirm") {
            const proceed = await vscode.window.showWarningMessage(gate.reason, "Apply anyway");
            if (proceed === undefined) {
                return;
            }
        }
        const decision = hunkAction(this.pendingAskFor(path), hunk, accept ? "allow_once" : "deny");
        const outcome = decision.kind === "answer"
            ? await this.client.answerAsk(decision.ask_id, decision.answer)
            : await this.client.steer(decision.text);
        if (!outcome.ok) {
            void vscode.window.showErrorMessage(`Harness: hunk command failed (${outcome.error.kind})`);
        }
    }
}

function telemetryEnabled(): boolean {
    return vscode.workspace.getConfiguration("harness").get<boolean>("telemetry.enabled") ?? false;
}

function editorSnapshot(): EditorSnapshot {
    const editor = vscode.window.activeTextEditor;
    if (editor === undefined) {
        return { activeFile: null, language: null, line: null, column: null, selectedText: null, diagnostics: [] };
    }
    const selection = editor.selection;
    return {
        activeFile: relativePath(editor.document.uri),
        language: editor.document.languageId,
        line: selection.active.line + 1,
        column: selection.active.character + 1,
        selectedText: editor.document.getText(selection),
        diagnostics: vscode.languages
            .getDiagnostics(editor.document.uri)
            .map((diagnostic) => ({
                severity: diagnostic.severity === vscode.DiagnosticSeverity.Error ? "error"
                    : diagnostic.severity === vscode.DiagnosticSeverity.Warning ? "warning"
                        : "info",
                line: diagnostic.range.start.line + 1,
                message: diagnostic.message,
            })),
    };
}

export function relativePath(uri: vscode.Uri): string {
    const relative = vscode.workspace.asRelativePath(uri, false);
    return relative.split("\\").join("/");
}

export async function readDaemonToken(tokenFile: string): Promise<string | null> {
    const folder = vscode.workspace.workspaceFolders?.[0];
    if (folder === undefined) {
        return null;
    }
    const uri = vscode.Uri.joinPath(folder.uri, tokenFile);
    try {
        const bytes = await vscode.workspace.fs.readFile(uri);
        return new TextDecoder().decode(bytes).trim();
    } catch {
        return null;
    }
}
