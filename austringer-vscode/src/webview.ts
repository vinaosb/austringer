/**
 * Transcript webview panel: VS Code glue over the pure transcript
 * renderer — plain HTML re-rendered per append (no framework, no
 * scripts inside the webview). Owns only panel lifecycle and a
 * bounded entry buffer.
 */

import * as vscode from "vscode";

import { renderTranscriptHtml, type TranscriptEntry } from "./transcript.ts";

const MAX_ENTRIES = 500;

export class TranscriptPanel {
    private panel: vscode.WebviewPanel | null = null;
    private readonly entries: TranscriptEntry[] = [];

    append(entry: TranscriptEntry): void {
        this.entries.push(entry);
        if (this.entries.length > MAX_ENTRIES) {
            this.entries.splice(0, this.entries.length - MAX_ENTRIES);
        }
        if (this.panel !== null) {
            this.panel.webview.html = renderTranscriptHtml(this.entries);
        }
    }

    show(): void {
        if (this.panel !== null) {
            this.panel.reveal();
            return;
        }
        this.panel = vscode.window.createWebviewPanel(
            "harnessTranscript",
            "Austringer transcript",
            vscode.ViewColumn.Beside,
            {},
        );
        this.panel.onDidDispose(() => {
            this.panel = null;
        });
        this.panel.webview.html = renderTranscriptHtml(this.entries);
    }

    dispose(): void {
        this.panel?.dispose();
        this.panel = null;
    }
}
