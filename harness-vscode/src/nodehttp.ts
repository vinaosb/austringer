/**
 * Node stdlib transport for the daemon client (SSE GET + JSON POST).
 * Pure `node:http` — no WebSocket dependency, no bundled model calls,
 * loopback URLs only (the daemon refuses anything else anyway).
 *
 * Every failure path funnels through exactly one `finishStream` /
 * single-settle resolve so no promise is ever left pending: clean
 * server close, socket death, client-initiated close, and POST
 * timeout all resolve their callers.
 */

import http from "node:http";

import type { HttpIo, PostOutcome, SseOpen } from "./http.ts";

const POST_TIMEOUT_MS = 15_000;

export function nodeHttpIo(postTimeoutMs = POST_TIMEOUT_MS): HttpIo {
    return {
        openSse: (
            url: string,
            callbacks: {
                onChunk(chunk: string): void;
                onEnd(): void;
                onError(detail: string): void;
            },
        ): Promise<SseOpen> => {
            return new Promise<SseOpen>((resolve) => {
                let openSettled = false;
                let streamFinished = false;
                const finishStream = (detail?: string): void => {
                    if (streamFinished) {
                        return;
                    }
                    streamFinished = true;
                    if (detail !== undefined) {
                        callbacks.onError(detail);
                    }
                    callbacks.onEnd();
                };
                const request = http.get(
                    url,
                    { headers: { accept: "text/event-stream" }, agent: false },
                    (res) => {
                        openSettled = true;
                        if (res.statusCode === undefined || res.statusCode !== 200) {
                            res.resume();
                            resolve(
                                res.statusCode === undefined
                                    ? { ok: false, networkError: "no status" }
                                    : { ok: false, status: res.statusCode },
                            );
                            return;
                        }
                        res.setEncoding("utf8");
                        res.on("data", (chunk: string) => {
                            callbacks.onChunk(chunk);
                        });
                        res.on("end", () => {
                            finishStream();
                        });
                        res.on("error", (error: Error) => {
                            finishStream(String(error));
                        });
                        resolve({
                            ok: true,
                            stream: {
                                close: () => {
                                    // destroy(error) makes the request emit
                                    // 'error', which funnels to finishStream;
                                    // a bare destroy() emits nothing the
                                    // client could ever observe.
                                    request.destroy(new Error("stream closed by client"));
                                },
                            },
                        });
                    },
                );
                request.on("error", (error) => {
                    if (!openSettled) {
                        openSettled = true;
                        resolve({ ok: false, networkError: String(error) });
                        return;
                    }
                    finishStream(String(error));
                });
            });
        },
        postJson: (
            url: string,
            headers: Record<string, string>,
            body: string,
        ): Promise<PostOutcome> => {
            return new Promise<PostOutcome>((resolve) => {
                let target: URL;
                try {
                    target = new URL(url);
                } catch {
                    resolve({ ok: false, networkError: "invalid url" });
                    return;
                }
                if (target.protocol !== "http:") {
                    resolve({ ok: false, networkError: "unsupported protocol" });
                    return;
                }
                const request = http.request(
                    target,
                    { method: "POST", headers, timeout: postTimeoutMs, agent: false },
                    (res) => {
                        let responseBody = "";
                        res.setEncoding("utf8");
                        res.on("data", (chunk: string) => {
                            responseBody += chunk;
                        });
                        res.on("end", () => {
                            resolve({ ok: true, status: res.statusCode ?? 0 });
                        });
                        res.on("error", (error: Error) => {
                            resolve({ ok: false, networkError: String(error) });
                        });
                    },
                );
                request.on("timeout", () => {
                    // destroy(error) -> 'error' handler below resolves the
                    // promise; a bare destroy() would leave it pending.
                    request.destroy(new Error("post timed out"));
                });
                request.on("error", (error) => {
                    resolve({ ok: false, networkError: String(error) });
                });
                request.end(body);
            });
        },
    };
}
