/**
 * Opt-in editor telemetry: when enabled, the active file / cursor /
 * selection / diagnostics are folded into the prompt as a context
 * block, so "fix this" carries the where without a round-trip
 * question. Off by default; nothing leaves the editor otherwise.
 */

export type DiagnosticSnapshot = {
    severity: "error" | "warning" | "info";
    line: number;
    message: string;
};

export type EditorSnapshot = {
    activeFile: string | null;
    language: string | null;
    line: number | null;
    column: number | null;
    selectedText: string | null;
    diagnostics: DiagnosticSnapshot[];
};

const MAX_SELECTION_CHARS = 512;
const MAX_DIAGNOSTICS = 5;

function truncate(text: string, max: number): string {
    return text.length <= max ? text : `${text.slice(0, max)}…`;
}

export function buildTelemetryContext(enabled: boolean, snapshot: EditorSnapshot): string | null {
    if (!enabled || snapshot.activeFile === null) {
        return null;
    }
    const lines: string[] = [`[editor] ${snapshot.activeFile}`];
    if (snapshot.language !== null) {
        lines.push(`language: ${snapshot.language}`);
    }
    if (snapshot.line !== null && snapshot.column !== null) {
        lines.push(`cursor: ${snapshot.line}:${snapshot.column}`);
    }
    if (snapshot.selectedText !== null && snapshot.selectedText !== "") {
        lines.push(`selection: ${truncate(snapshot.selectedText, MAX_SELECTION_CHARS)}`);
    }
    for (const diagnostic of snapshot.diagnostics.slice(0, MAX_DIAGNOSTICS)) {
        lines.push(`${diagnostic.severity} ${diagnostic.line}: ${truncate(diagnostic.message, 200)}`);
    }
    return lines.join("\n");
}
