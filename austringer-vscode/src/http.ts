/**
 * Transport seam the daemon client consumes. The extension host binds
 * it to node:http (nodehttp.ts); tests bind it to whatever they need.
 */

export interface SseStream {
    close(): void;
}

export type SseOpen =
    | { ok: true; stream: SseStream }
    | { ok: false; status?: number; networkError?: string };

export type PostOutcome =
    | { ok: true; status: number }
    | { ok: false; status?: number; networkError?: string };

export interface HttpIo {
    openSse(
        url: string,
        callbacks: {
            onChunk(chunk: string): void;
            onEnd(): void;
            onError(detail: string): void;
        },
    ): Promise<SseOpen>;
    postJson(
        url: string,
        headers: Record<string, string>,
        body: string,
    ): Promise<PostOutcome>;
}
