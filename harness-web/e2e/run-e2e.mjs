// Playwright e2e for the harness web SPA (todo 37 acceptance:
// "attach, approve hunk, see HUD update" + QA channels). Hermetic:
// playwright-core resolves from the global npm tree (NODE_PATH) and
// chromium from the local ms-playwright cache — no network.
//
// Usage:
//   set HARNESS_E2E_EXE=<path to e2e_daemon.exe built with --example>
//   set NODE_PATH=C:\Users\vsbb1\AppData\Roaming\npm\node_modules
//   node e2e/run-e2e.mjs
import { createRequire } from "node:module";
import { spawn } from "node:child_process";
import { join } from "node:path";
import assert from "node:assert/strict";
import { mintShare } from "../assets/share.js";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright-core");

const EXE = process.env.HARNESS_E2E_EXE;
assert.ok(EXE, "HARNESS_E2E_EXE must point at the built e2e_daemon example exe");
const CHROMIUM =
  process.env.PLAYWRIGHT_CHROMIUM ||
  join(
    process.env.LOCALAPPDATA,
    "ms-playwright",
    "chromium-1234",
    "chrome-win64",
    "chrome.exe",
  );

async function startDaemon() {
  const child = spawn(EXE, ["--serve-for-e2e"], {
    stdio: ["ignore", "pipe", "inherit"],
  });
  let base = "";
  let token = "";
  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("daemon banner timeout")), 15000);
    let buffer = "";
    child.stdout.on("data", (chunk) => {
      buffer += String(chunk);
      const line = buffer.split("\n").find((l) => l.startsWith("E2E-DAEMON "));
      if (line) {
        const info = JSON.parse(line.slice("E2E-DAEMON ".length));
        base = info.base;
        token = info.token;
        clearTimeout(timer);
        resolve();
      }
    });
    child.on("exit", (code) => reject(new Error(`daemon exited early: ${code}`)));
  });
  return { child, base, token };
}

async function openSession(context, base, token, sid) {
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (err) => errors.push(String(err)));
  await page.goto(`http://${base}/s/${sid}?token=${token}`);
  await page.waitForSelector("[data-testid=hud]", { timeout: 10000 });
  return { page, errors };
}

const results = [];
async function scenario(name, fn) {
  try {
    await fn();
    results.push(`PASS ${name}`);
  } catch (err) {
    results.push(`FAIL ${name}: ${err.message}`);
    throw err;
  }
}

async function main() {
  const daemon = await startDaemon();
  const { base, token } = daemon;
  const browser = await chromium.launch({ executablePath: CHROMIUM });

  try {
    await scenario("happy: two browsers attach, approve gate, HUD + hunk update in both", async () => {
      const ctxA = await browser.newContext();
      const ctxB = await browser.newContext();
      const a = await openSession(ctxA, base, token, "e2e");
      const b = await openSession(ctxB, base, token, "e2e");

      await a.page.waitForSelector(".status-open", { timeout: 10000 });
      await b.page.waitForSelector(".status-open", { timeout: 10000 });

      const eventsBefore = await a.page.textContent("[data-testid=stat-events]");
      await a.page.fill("[data-testid=composer]", "hello from browser A");
      await a.page.click("button[type=submit]");
      await a.page.waitForSelector(".gate-ask", { timeout: 10000 });
      await b.page.waitForSelector(".gate-ask", { timeout: 10000 });

      await a.page.click(".gate-ask button:has-text(\"y (once)\")");

      for (const { page, label } of [{ page: a.page, label: "A" }, { page: b.page, label: "B" }]) {
        await page.waitForSelector(".diff", { timeout: 10000 });
        await page.waitForFunction(
          () => document.querySelector("[data-testid=transcript]")?.textContent?.includes("done: turn completed"),
          undefined,
          { timeout: 10000 },
        );
        const events = await page.textContent("[data-testid=stat-events]");
        assert.ok(Number(events) > Number(eventsBefore), `${label}: HUD events must increase`);
        assert.equal(await page.textContent("[data-testid=stat-turns]"), "1", `${label}: one turn`);
        assert.match(await page.textContent("[data-testid=transcript]"), /echo/, `${label}: tool call rendered`);
      }
      const eventsA = await a.page.textContent("[data-testid=stat-events]");
      const eventsB = await b.page.textContent("[data-testid=stat-events]");
      assert.equal(eventsA, eventsB, "both browsers converged to identical event count");
      assert.deepEqual(a.errors, [], "no page errors A");
      assert.deepEqual(b.errors, [], "no page errors B");
      await ctxA.close();
      await ctxB.close();
    });

    await scenario("failure: expired share link renders 410 Gone", async () => {
      const ctx = await browser.newContext();
      const page = await ctx.newPage();
      const expired = mintShare(
        { session: "gone", lastEventId: 1, transcript: [{ role: "user", text: "hi" }], hud: { turns: 1, events: 1, toolCalls: {} } },
        -1,
      );
      await page.goto(`http://${base}/${expired}`);
      await page.waitForSelector("[data-testid=share-gone]", { timeout: 5000 });
      assert.match(await page.textContent("[data-testid=share-gone]"), /410 Gone/);
      await ctx.close();
    });

    await scenario("failure: auth negatives (401/403) and wrong-token page never sees events", async () => {
      const ctx = await browser.newContext();
      const noToken = await ctx.request.get(`http://${base}/sessions`);
      assert.equal(noToken.status(), 401, "no token -> 401");
      const wrong = await ctx.request.get(`http://${base}/sessions`, {
        headers: { authorization: "Bearer wrong" },
      });
      assert.equal(wrong.status(), 401, "wrong token -> 401");
      const spoof = await ctx.request.get(`http://${base}/sessions`, {
        headers: { authorization: `Bearer ${token}`, host: "evil.example.com" },
      });
      assert.equal(spoof.status(), 403, "spoofed Host + valid token -> 403");

      const page = await ctx.newPage();
      await page.goto(`http://${base}/s/e2e-wrong?token=not-the-token`);
      await page.waitForSelector("[data-testid=hud]", { timeout: 5000 });
      await page.waitForFunction(
        () => ["reconnecting", "connecting"].includes(
          document.querySelector(".status")?.textContent || ""
        ),
        undefined,
        { timeout: 8000 },
      );
      const entries = await page.textContent("[data-testid=transcript]");
      assert.equal(entries.trim(), "no events yet — submit a turn below", "no events leak without valid auth");
      await ctx.close();
    });

    await scenario("failure: absent daemon -> reconnecting state", async () => {
      const ctx = await browser.newContext();
      const a = await openSession(ctx, base, token, "e2e-dying");
      await a.page.waitForSelector(".status-open", { timeout: 10000 });
      daemon.child.kill();
      await new Promise((resolve) => daemon.child.once("exit", resolve));
      await a.page.waitForFunction(
        () => {
          const status = document.querySelector(".status")?.textContent || "";
          return status === "reconnecting" || status === "connecting" || status === "error";
        },
        undefined,
        { timeout: 15000 },
      );
      results.push("PASS absent-daemon observed reconnecting");
      await ctx.close();
    });
  } finally {
    await browser.close();
    daemon.child.kill();
  }

  console.log(results.join("\n"));
  console.log(`E2E RESULT: ${results.every((r) => r.startsWith("PASS")) ? "ALL PASS" : "FAILURES"}`);
}

main().catch((err) => {
  console.error(results.join("\n"));
  console.error(`E2E RESULT: FATAL ${err.message}`);
  process.exitCode = 1;
});
