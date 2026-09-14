import { test } from "node:test";
import assert from "node:assert/strict";

import { createIdeBridge, type IdeProviders } from "../src/bridge.ts";

const providers: IdeProviders = {
    search: (query) =>
        Promise.resolve({
            matches: [{ path: "src/lib.ts", line: 10, preview: `...${query}...` }],
        }),
    definition: () => Promise.resolve({ path: "src/other.ts", line: 3, column: 1 }),
};

async function withBridge(
    run: (baseUrl: string) => Promise<void>,
): Promise<void> {
    const bridge = createIdeBridge({ token: "bridge-token", providers });
    const url = await bridge.listen();
    try {
        await run(url);
    } finally {
        await bridge.close();
    }
}

async function postJson(
    url: string,
    path: string,
    token: string | null,
    body: unknown,
): Promise<{ status: number; json: unknown }> {
    const response = await fetch(`${url}${path}`, {
        method: "POST",
        headers: {
            "content-type": "application/json",
            ...(token === null ? {} : { "x-harness-token": token }),
        },
        body: JSON.stringify(body),
    });
    const text = await response.text();
    return { status: response.status, json: text === "" ? null : JSON.parse(text) };
}

test("search reaches the IDE provider and returns its matches", async () => {
    await withBridge(async (url) => {
        const outcome = await postJson(url, "/ide/search", "bridge-token", { query: "parseOmnibar" });
        assert.equal(outcome.status, 200);
        const payload = outcome.json as { matches: Array<{ path: string }> };
        assert.deepEqual(payload.matches.map((match) => match.path), ["src/lib.ts"]);
    });
});

test("definition bridging answers with the IDE's language server result", async () => {
    await withBridge(async (url) => {
        const outcome = await postJson(
            url,
            "/ide/definition",
            "bridge-token",
            { file: "src/lib.ts", line: 10, column: 3 },
        );
        assert.equal(outcome.status, 200);
        assert.deepEqual(outcome.json, { path: "src/other.ts", line: 3, column: 1 });
    });
});

test("missing or wrong token is refused 401 before touching the provider", async () => {
    await withBridge(async (url) => {
        const missing = await postJson(url, "/ide/search", null, { query: "x" });
        assert.equal(missing.status, 401);
        const wrong = await postJson(url, "/ide/search", "nope", { query: "x" });
        assert.equal(wrong.status, 401);
    });
});

test("malformed body is a typed 400, provider never called", async () => {
    let providerCalls = 0;
    const counting: IdeProviders = {
        search: (query) => {
            providerCalls += 1;
            return providers.search(query);
        },
        definition: (where) => {
            providerCalls += 1;
            return providers.definition(where);
        },
    };
    const bridge = createIdeBridge({ token: "t", providers: counting });
    const url = await bridge.listen();
    try {
        const outcome = await postJson(url, "/ide/search", "t", { nope: true });
        assert.equal(outcome.status, 400);
        assert.equal(providerCalls, 0);
    } finally {
        await bridge.close();
    }
});

test("unknown route is 404, wrong method is 405", async () => {
    await withBridge(async (url) => {
        const unknown = await postJson(url, "/ide/nothing", "bridge-token", {});
        assert.equal(unknown.status, 404);
        const response = await fetch(`${url}/ide/search?x=1`, {
            headers: { "x-harness-token": "bridge-token" },
        });
        assert.equal(response.status, 405);
    });
});
