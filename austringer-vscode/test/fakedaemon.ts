import http from "node:http";
import type { AddressInfo } from "node:net";

import type { Frame, ServerMsg } from "../src/protocol.ts";

/**
 * In-process fake daemon speaking the real HTTP surface (SSE events +
 * JSON commands + session list) so client tests exercise actual
 * sockets, actual SSE framing, and actual replay semantics — no live
 * austringer-daemon, no live VS Code.
 */
export class FakeDaemon {
    private readonly server: http.Server;
    private readonly sseClients = new Set<http.ServerResponse>();
    private history: Frame[] = [];
    private nextId = 1;
    private killOnOpen = 0;
    private forceResync = false;
    private hanging = false;
    readonly posts: { url: string; body: string }[] = [];
    sseOpens = 0;

    readonly token: string;

    private constructor(token: string) {
        this.token = token;
        this.server = http.createServer((req, res) => {
            this.route(req, res);
        });
    }

    static async start(token = "test-token"): Promise<FakeDaemon> {
        const daemon = new FakeDaemon(token);
        await new Promise<void>((resolve, reject) => {
            daemon.server.once("error", reject);
            daemon.server.listen(0, "127.0.0.1", () => {
                resolve();
            });
        });
        return daemon;
    }

    get url(): string {
        const address = this.server.address() as AddressInfo;
        return `http://127.0.0.1:${address.port}`;
    }

    async stop(): Promise<void> {
        for (const client of this.sseClients) {
            client.destroy();
        }
        this.sseClients.clear();
        this.server.closeAllConnections();
        await new Promise<void>((resolve) => {
            this.server.close(() => {
                resolve();
            });
        });
    }

    /** Auto-id the frame from history and broadcast. */
    push(event: ServerMsg): void {
        if (event.type === "event") {
            this.pushFrame(event.frame);
            return;
        }
        this.broadcast(JSON.stringify(event));
    }

    pushFrame(frame: Frame): void {
        this.history.push(frame);
        this.nextId = Math.max(this.nextId, frame.id + 1);
        this.broadcast(
            JSON.stringify({ type: "event", frame } satisfies ServerMsg),
        );
    }

    /** Next auto-id (for building raw frames with gaps). */
    get autoId(): number {
        return this.nextId;
    }

    /** Kill every live SSE connection (crash/flake simulation). */
    killSse(): void {
        for (const client of this.sseClients) {
            client.destroy();
        }
        this.sseClients.clear();
    }

    /** End every live SSE stream cleanly (server idle-recycle). */
    endSse(): void {
        for (const client of this.sseClients) {
            client.end();
        }
        this.sseClients.clear();
    }

    /** Accept POSTs but never answer them (black-hole command sink). */
    hangPosts(): void {
        this.hanging = true;
    }

    /** Destroy the next N SSE connections right after upgrade. */
    killNextSseConnections(count: number): void {
        this.killOnOpen = count;
    }

    /** Serve resync+snapshot on connect instead of delta replay. */
    serveResyncInsteadOfDelta(): void {
        this.forceResync = true;
    }

    /** Push a raw SSE data payload verbatim (malformed-frame tests). */
    pushRawLine(data: string): void {
        this.broadcast(data);
    }

    /** The `after` cursor of each SSE open (null = absent). */
    readonly afters: (number | null)[] = [];

    /** The proposed protocol `version` of each SSE open (null = absent). */
    readonly versions: (number | null)[] = [];

    private broadcast(data: string): void {
        for (const client of this.sseClients) {
            client.write(`data: ${data}\n\n`);
        }
    }

    private authorized(url: URL, req: http.IncomingMessage): boolean {
        const queryToken = url.searchParams.get("token");
        const header = req.headers["x-austringer-token"];
        return (
            queryToken === this.token ||
            (typeof header === "string" && header === this.token)
        );
    }

    private route(req: http.IncomingMessage, res: http.ServerResponse): void {
        const url = new URL(req.url ?? "/", "http://127.0.0.1");
        if (!this.authorized(url, req)) {
            res.writeHead(401, "unauthorized");
            res.end();
            return;
        }
        if (req.method === "GET" && url.pathname === "/sessions") {
            res.writeHead(200, { "content-type": "application/json" });
            res.end(JSON.stringify({ sessions: ["vscode"] }));
            return;
        }
        const sse = url.pathname.match(/^\/s\/([^/]+)\/events$/);
        if (req.method === "GET" && sse) {
            this.openSse(url, res);
            return;
        }
        const cmd = url.pathname.match(/^\/s\/([^/]+)\/cmd$/);
        if (req.method === "POST" && cmd) {
            this.takeCommand(req, res);
            return;
        }
        res.writeHead(404, "not found");
        res.end();
    }

    private openSse(url: URL, res: http.ServerResponse): void {
        this.sseOpens += 1;
        const after = url.searchParams.get("after");
        this.afters.push(after === null ? null : Number(after));
        const version = url.searchParams.get("version");
        this.versions.push(version === null ? null : Number(version));
        res.writeHead(200, {
            "content-type": "text/event-stream",
            "cache-control": "no-cache",
        });
        if (this.killOnOpen > 0) {
            this.killOnOpen -= 1;
            res.destroy();
            return;
        }
        this.sseClients.add(res);
        res.write(": keep-alive\n\n");
        if (this.forceResync) {
            res.write(
                `data: ${JSON.stringify({
                    type: "state_resync_required",
                    last_event_id: this.history.length,
                } satisfies ServerMsg)}\n\n`,
            );
            res.write(
                `data: ${JSON.stringify({
                    type: "snapshot",
                    frames: this.history,
                } satisfies ServerMsg)}\n\n`,
            );
        } else if (after !== null) {
            const afterId = Number(after);
            for (const frame of this.history.filter((frame) => frame.id > afterId)) {
                res.write(
                    `data: ${JSON.stringify({ type: "event", frame } satisfies ServerMsg)}\n\n`,
                );
            }
        }
        const lastId = this.history.reduce((max, frame) => Math.max(max, frame.id), 0);
        res.write(
            `data: ${JSON.stringify({
                type: "hello_ack",
                version: 2,
                last_event_id: lastId,
            } satisfies ServerMsg)}\n\n`,
        );
    }

    private takeCommand(req: http.IncomingMessage, res: http.ServerResponse): void {
        let body = "";
        req.on("data", (chunk: Buffer) => {
            body += chunk.toString("utf8");
        });
        req.on("end", () => {
            if (this.hanging) {
                return;
            }
            this.posts.push({ url: req.url ?? "", body });
            const parsed: unknown = JSON.parse(body === "" ? "{}" : body);
            const type =
                typeof parsed === "object" && parsed !== null
                    ? (parsed as { type?: unknown }).type
                    : null;
            if (type === "permission_answer" || type === "permission_cancel") {
                const askId =
                    (parsed as { ask_id?: unknown }).ask_id ?? null;
                const known = typeof askId === "number" && askId === 1;
                if (!known) {
                    res.writeHead(404, `unknown ask ${String(askId)}`);
                    res.end();
                    return;
                }
            }
            res.writeHead(202, "accepted");
            res.end();
        });
    }
}
