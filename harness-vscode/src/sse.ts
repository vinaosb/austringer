/**
 * Incremental SSE framing: buffers arbitrary chunk splits, drops
 * comment blocks (keep-alives carry no data), joins multi-line `data:`
 * fields per the event-stream grammar, and only emits complete
 * `\n\n`-terminated events.
 */

export function createSseParser(onPayload: (payload: string) => void): {
    push(chunk: string): void;
} {
    let buffer = "";
    return {
        push(chunk: string): void {
            buffer += chunk;
            for (;;) {
                const boundary = buffer.indexOf("\n\n");
                if (boundary === -1) {
                    return;
                }
                const block = buffer.slice(0, boundary);
                buffer = buffer.slice(boundary + 2);
                const dataLines = block
                    .split("\n")
                    .filter((line) => line.startsWith("data:"))
                    .map((line) => line.slice("data:".length).replace(/^ /, ""));
                if (dataLines.length > 0) {
                    onPayload(dataLines.join("\n"));
                }
            }
        },
    };
}
