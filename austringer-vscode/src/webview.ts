/**
 * Transcript webview panel: VS Code glue over the pure transcript
 * renderer — plain HTML, no framework, no scripts inside the webview.
 * Appends coalesce through renderschedule.ts (assigning `webview.html`
 * destroys and recreates the iframe, so it happens at most once per
 * window and never while the panel is hidden; reveal flushes once).
 */

import * as vscode from "vscode";

import { renderTranscriptHtml, type TranscriptEntry } from "./transcript.ts";
import { appendBounded, createRenderScheduler, MAX_ENTRIES } from "./renderschedule.ts";

const RENDER_WINDOW_MS = 200;

export class TranscriptPanel {
    private panel: vscode.WebviewPanel | null = null;
    private readonly entries: TranscriptEntry[] = [];
    private readonly scheduler = createRenderScheduler({ windowMs: RENDER_WINDOW_MS });
    private timer: ReturnType<typeof setTimeout> | null = null;

    append(entry: TranscriptEntry): void {
        appendBounded(this.entries, entry, MAX_ENTRIES);
        if (this.panel === null) {
            return; // no panel yet: show() renders the full buffer on creation
        }
        const due = this.scheduler.push(Date.now());
        if (due === null) {
            return;
        }
        if (this.timer !== null) {
            clearTimeout(this.timer);
        }
        this.timer = setTimeout(() => {
            this.timer = null;
            this.flush();
        }, Math.max(0, due - Date.now()));
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
            if (this.timer !== null) {
                clearTimeout(this.timer);
                this.timer = null;
            }
            this.panel = null;
        });
        this.panel.onDidChangeViewState((event) => {
            if (this.scheduler.setVisible(event.webviewPanel.visible, Date.now())) {
                this.render();
            }
        });
        this.render();
        this.scheduler.markRendered();
    }

    dispose(): void {
        if (this.timer !== null) {
            clearTimeout(this.timer);
            this.timer = null;
        }
        this.panel?.dispose();
        this.panel = null;
    }

    private flush(): void {
        if (this.scheduler.takeFlush(Date.now())) {
            this.render();
        }
    }

    private render(): void {
        if (this.panel !== null) {
            this.panel.webview.html = renderTranscriptHtml(this.entries);
        }
    }
}
