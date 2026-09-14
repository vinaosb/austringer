/**
 * Opt-in IDE bridge: a loopback HTTP endpoint that exposes this
 * editor's search and go-to-definition to the harness daemon, so the
 * harness prefers the IDE's already-running language servers and
 * ripgrep over spawning its own. Off unless configured; bearer token
 * = the daemon token; every route refuses before touching the IDE if
 * the token does not match.
 */

import http from "node:http";
import type { AddressInfo } from "node:net";

export type SearchMatch = { path: string; line: number; preview: string };
export type SearchResult = { matches: SearchMatch[] };
export type DefinitionWhere = { file: string; line: number; column: number };
export type DefinitionResult = { path: string; line: number; column: number };

export type IdeProviders = {
    search(query: string): Promise<SearchResult>;
    definition(where: DefinitionWhere): Promise<DefinitionResult>;
};

export type IdeBridge = {
    listen(): Promise<string>;
    close(): Promise<void>;
};

const MAX_BODY_BYTES = 1024 * 1024;

interface RouteShape {
    fields: readonly string[];
    handle(providers: IdeProviders, body: Record<string, unknown>): Promise<unknown>;
}

const ROUTES: Record<string, RouteShape> = {
    "/ide/search": {
        fields: ["query"],
        handle: (providers, body) => providers.search(String(body["query"])),
    },
    "/ide/definition": {
        fields: ["file", "line", "column"],
        handle: (providers, body) =>
            providers.definition({
                file: String(body["file"]),
                line: Number(body["line"]),
                column: Number(body["column"]),
            }),
    },
};

export function createIdeBridge(config: { token: string; providers: IdeProviders }): IdeBridge {
    const server = http.createServer((req, res) => {
        void route(config, req, res);
    });
    return {
        listen: () =>
            new Promise<string>((resolve, reject) => {
                server.once("error", reject);
                server.listen(0, "127.0.0.1", () => {
                    const address = server.address() as AddressInfo;
                    resolve(`http://127.0.0.1:${address.port}`);
                });
            }),
        close: () =>
            new Promise<void>((resolve, reject) => {
                server.closeAllConnections();
                server.close((error) => {
                    if (error === undefined) {
                        resolve();
                    } else {
                        reject(error);
                    }
                });
            }),
    };
}

async function route(
    config: { token: string; providers: IdeProviders },
    req: http.IncomingMessage,
    res: http.ServerResponse,
): Promise<void> {
    const finish = (status: number, payload: unknown): void => {
        res.writeHead(status, { "content-type": "application/json" });
        res.end(JSON.stringify(payload));
    };
    const path = (req.url ?? "").split("?")[0] ?? "";
    const shape = ROUTES[path];
    if (shape === undefined) {
        finish(404, { error: "unknown route" });
        return;
    }
    if (req.method !== "POST") {
        finish(405, { error: "post only" });
        return;
    }
    const presented = req.headers["x-harness-token"];
    if (typeof presented !== "string" || presented !== config.token) {
        finish(401, { error: "unauthorized" });
        return;
    }
    const body = await readBody(req);
    if (body === null) {
        finish(400, { error: "malformed body" });
        return;
    }
    for (const field of shape.fields) {
        if (!(field in body)) {
            finish(400, { error: `missing field ${field}` });
            return;
        }
    }
    try {
        finish(200, await shape.handle(config.providers, body));
    } catch (error) {
        finish(502, { error: String(error) });
    }
}

function readBody(req: http.IncomingMessage): Promise<Record<string, unknown> | null> {
    return new Promise((resolve) => {
        let body = "";
        req.setEncoding("utf8");
        req.on("data", (chunk: string) => {
            body += chunk;
            if (body.length > MAX_BODY_BYTES) {
                req.destroy();
                resolve(null);
            }
        });
        req.on("end", () => {
            try {
                const parsed: unknown = JSON.parse(body === "" ? "{}" : body);
                resolve(
                    typeof parsed === "object" && parsed !== null && !Array.isArray(parsed)
                        ? (parsed as Record<string, unknown>)
                        : null,
                );
            } catch {
                resolve(null);
            }
        });
        req.on("error", () => {
            resolve(null);
        });
    });
}
