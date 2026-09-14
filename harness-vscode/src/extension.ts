/**
 * Extension entrypoint (plan todo 38): activation wiring only —
 * commands, status bar, code lenses, and the opt-in IDE bridge. The
 * session logic lives in session.ts; every decision module is
 * headless-tested. No model calls are bundled in the extension.
 */

import * as vscode from "vscode";

import { createIdeBridge, type DefinitionWhere, type IdeBridge, type IdeProviders } from "./bridge.ts";
import { HarnessSession, readDaemonToken, relativePath } from "./session.ts";

export async function activate(context: vscode.ExtensionContext): Promise<void> {
    const gauge = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
    const session = await connectOrReport(gauge);
    if (session === null) {
        return;
    }
    session.start();
    context.subscriptions.push(
        gauge,
        vscode.commands.registerCommand("harness.open", () => void session.omnibar()),
        vscode.commands.registerCommand("harness.hunkAccept", (path: string, header: string) =>
            void session.hunkCommand(path, header, true)),
        vscode.commands.registerCommand("harness.hunkReject", (path: string, header: string) =>
            void session.hunkCommand(path, header, false)),
        vscode.commands.registerCommand("harness.permission.allowOnce", (askId: number) =>
            void session.answer(askId, "allow_once")),
        vscode.commands.registerCommand("harness.permission.allowSession", (askId: number) =>
            void session.answer(askId, "allow_session")),
        vscode.commands.registerCommand("harness.permission.deny", (askId: number) =>
            void session.answer(askId, "deny")),
        vscode.commands.registerCommand("harness.reconnect", () =>
            void vscode.window.showInformationMessage("Harness: use 'Reload Window' to reconnect once the daemon is back")),
        vscode.languages.registerCodeLensProvider({ scheme: "file" }, new HunkLensProvider(session)),
        vscode.window.onDidChangeActiveTextEditor(() => session.decorate()),
        { dispose: () => void session.stop() },
    );
    const bridge = await maybeStartBridge(session);
    if (bridge !== null) {
        context.subscriptions.push({ dispose: () => void bridge.close() });
    }
}

export function deactivate(): void {
    // session teardown rides context.subscriptions disposal.
}

async function connectOrReport(gauge: vscode.StatusBarItem): Promise<HarnessSession | null> {
    try {
        return await HarnessSession.connect(gauge);
    } catch (error) {
        void vscode.window.showWarningMessage(`Harness: ${String(error)} — reconnect once the daemon is up`);
        return null;
    }
}

class HunkLensProvider implements vscode.CodeLensProvider {
    constructor(private readonly session: HarnessSession) {}

    provideCodeLenses(document: vscode.TextDocument): vscode.CodeLens[] {
        const path = relativePath(document.uri);
        const plan = this.session.planFor(document.uri);
        if (plan === undefined) {
            return [];
        }
        return plan.hunks.flatMap((hunk) => {
            const range = new vscode.Range(hunk.startLine - 1, 0, hunk.endLine - 1, 0);
            return [
                new vscode.CodeLens(range, {
                    title: "Accept hunk",
                    command: "harness.hunkAccept",
                    arguments: [path, hunk.header],
                }),
                new vscode.CodeLens(range, {
                    title: "Reject hunk",
                    command: "harness.hunkReject",
                    arguments: [path, hunk.header],
                }),
            ];
        });
    }
}

async function maybeStartBridge(session: HarnessSession): Promise<IdeBridge | null> {
    const config = vscode.workspace.getConfiguration("harness");
    if (!(config.get<boolean>("ideBridge.enabled") ?? false)) {
        return null;
    }
    const token = await readDaemonToken(config.get<string>("daemonTokenFile") ?? ".harness-daemon/daemon-token");
    if (token === null) {
        return null;
    }
    const bridge = createIdeBridge({ token, providers: vscodeProviders() });
    await bridge.listen();
    return bridge;
}

function vscodeProviders(): IdeProviders {
    return {
        search: async (query) => {
            const uris = await vscode.workspace.findFiles(`**/*${query}*`, null, 50);
            return {
                matches: uris.map((uri) => ({ path: relativePath(uri), line: 1, preview: "" })),
            };
        },
        definition: async (where: DefinitionWhere) => {
            const folder = vscode.workspace.workspaceFolders?.[0];
            if (folder === undefined) {
                throw new Error("no workspace");
            }
            const uri = vscode.Uri.joinPath(folder.uri, ...where.file.split("/"));
            const locations = (await vscode.commands.executeCommand(
                "vscode.executeDefinitionProvider",
                uri,
                new vscode.Position(where.line - 1, where.column - 1),
            )) as Array<vscode.Location> | undefined;
            const first = locations?.[0];
            if (first === undefined) {
                throw new Error("no definition");
            }
            return {
                path: relativePath(first.uri),
                line: first.range.start.line + 1,
                column: first.range.start.character + 1,
            };
        },
    };
}
