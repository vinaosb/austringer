/**
 * The live editor-side session (plan todo 38): owns the daemon
 * client, projects its events onto the status-bar gauge, diff
 * decorations, the transcript webview, pending permission prompts,
 * the omnibar, and hunk commands. Decisions live in headless-tested
 * modules; this file is VS Code glue only.
 *
 * allow: SIZE_OK — 271 pure LOC; 246 pre-todo-21 with every
 * todo-21-extractable unit (ask dialogs, transcript projection,
 * webview panel) already split out. The remainder is one routing
 * switch + thin delegates; moving untouched units (omnibar, hunk
 * glue) is out of todo-21 scope.
 */

import * as vscode from "vscode";

import { HarnessClient, type ClientEvent } from "./client.ts";
import { AskDialogs } from "./askdialogs.ts";
import type { AskPayload } from "./protocol.ts";
import { createGaugeView, type GaugeInputs } from "./gauge.ts";
import { hunkAction, planDiff, gateHunkAccept, type HunkPlan, type HunkRange } from "./hunks.ts";
import { nodeHttpIo } from "./nodehttp.ts";
import { runOmnibar, type OmnibarDeps } from "./omnibar.ts";
import { buildTelemetryContext, type EditorSnapshot } from "./telemetry.ts";
import { transcriptEntryFromFrame } from "./transcript.ts";
import { TranscriptPanel } from "./webview.ts";

export class HarnessSession {
    private readonly client: HarnessClient;
    private readonly gauge: vscode.StatusBarItem;
    private readonly dialogs: AskDialogs;
    private readonly decoration = vscode.window.createTextEditorDecorationType({
        backgroundColor: new vscode.ThemeColor("diffEditor.insertedTextBackground"),
        isWholeLine: true,
    });
    private readonly plans = new Map<string, HunkPlan>();
    private readonly pendingAsks = new Map<number, AskPayload>();
    private readonly transcript = new TranscriptPanel();
    private readonly renderGaugeView = createGaugeView((state) => {
        this.gauge.text = state.text;
        this.gauge.tooltip = state.tooltip;
        this.gauge.backgroundColor = state.kind === "action"
            ? new vscode.ThemeColor("statusBarItem.warningBackground")
            : undefined;
        this.gauge.show();
    });
    private gaugeInputs: GaugeInputs = { status: "disconnected", attempt: 0, turn: null, pendingAsks: 0 };
    private turn: number | null = null;

    constructor(client: HarnessClient, gauge: vscode.StatusBarItem) {
        this.client = client;
        this.gauge = gauge;
        this.dialogs = new AskDialogs(client);
    }

    static async connect(gauge: vscode.StatusBarItem): Promise<HarnessSession> {
        const config = vscode.workspace.getConfiguration("austringer");
        const token = await readDaemonToken(config.get<string>("daemonTokenFile") ?? ".austringer-daemon/daemon-token");
        if (token === null) {
            throw new Error("daemon token file not found — start the austringer daemon first");
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
        this.transcript.dispose();
        await this.client.stop();
    }

    handle(event: ClientEvent): void {
        switch (event.kind) {
            case "status":
                this.gaugeInputs = { ...this.gaugeInputs, status: event.status, attempt: event.attempt };
                break;
            case "frame":
                this.handleFrame(event.frame.event);
                this.appendTranscript(event.frame);
                break;
            case "ask":
                this.pendingAsks.set(event.ask.ask_id, event.ask);
                void this.dialogs.promptPermission(event.ask);
                break;
            case "resolved":
                this.pendingAsks.delete(event.askId);
                break;
            case "fallback-ask":
                void this.dialogs.promptFallback(event.ask);
                break;
            case "fallback-resolved":
                this.transcript.append({
                    seq: Date.now(),
                    kind: "notification",
                    text: `model fallback resolved → ${event.chosen.provider}/${event.chosen.model}`,
                });
                break;
            case "plan-ask":
                void this.dialogs.promptPlan(event.ask);
                break;
            case "plan-resolved":
                this.transcript.append({
                    seq: Date.now(),
                    kind: "plan",
                    text: `plan ${event.planId} resolved (${Object.keys(event.outcome)[0]})`,
                });
                break;
            case "snapshot":
                for (const frame of event.frames) {
                    this.handleFrame(frame.event);
                    this.appendTranscript(frame);
                }
                break;
            case "protocol-error":
                if (event.code === "auth") {
                    void vscode.window.showErrorMessage(`Austringer: ${event.message}`);
                }
                break;
            case "bye":
                void vscode.window.showInformationMessage(`Austringer daemon said bye: ${event.reason}`);
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

    private appendTranscript(frame: import("./protocol.ts").Frame): void {
        const entry = transcriptEntryFromFrame(frame);
        if (entry !== null) {
            this.transcript.append(entry);
        }
    }

    showTranscript(): void {
        this.transcript.show();
    }

    renderGauge(): void {
        this.renderGaugeView(this.gaugeInputs);
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
        await this.dialogs.answer(askId, answer);
    }

    async omnibar(): Promise<void> {
        const input = await vscode.window.showInputBox({ prompt: "austringer omnibar — /help lists commands" });
        if (input === undefined) {
            return;
        }
        const telemetry = buildTelemetryContext(telemetryEnabled(), editorSnapshot());
        const context = telemetry === null ? "" : `\n\n${telemetry}`;
        const deps: OmnibarDeps = {
            submit: (text) => this.client.submit(`${text}${context}`),
            submitVerbatim: (text) => this.client.submit(text),
            steer: (text) => this.client.steer(text),
            cancelTurn: () => this.client.cancelTurn(),
            cancelAsk: (askId) => this.client.cancelAsk(askId),
            planMode: (on) => this.client.planMode(on),
            rewind: (steps) => this.client.rewind(steps),
            openAskIds: () => [...this.pendingAsks.keys()],
            showInfo: (message) => void vscode.window.showInformationMessage(message),
            showWarn: (message) => void vscode.window.showWarningMessage(message),
            showError: (message) => void vscode.window.showErrorMessage(message),
        };
        await runOmnibar(input, deps);
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
            void vscode.window.showErrorMessage(`Austringer: hunk command failed (${outcome.error.kind})`);
        }
    }
}

function telemetryEnabled(): boolean {
    return vscode.workspace.getConfiguration("austringer").get<boolean>("telemetry.enabled") ?? false;
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
