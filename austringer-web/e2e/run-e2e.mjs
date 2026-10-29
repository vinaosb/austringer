// Playwright e2e for the austringer web SPA (todo 37 acceptance:
// "attach, approve hunk, see HUD update" + QA channels). Hermetic:
// playwright-core resolves from the global npm tree (NODE_PATH) and
// chromium from the local ms-playwright cache — no network.
//
// Usage:
//   set NODE_PATH=C:\Users\vsbb1\AppData\Roaming\npm\node_modules
//   node e2e/run-e2e.mjs
// The fixture exe defaults to $CARGO_TARGET_DIR (or ./target when
// unset, e.g. CI) under debug/examples/e2e_daemon(.exe); override
// with AUSTRINGER_E2E_EXE. The browser defaults to the dev machine's
// LOCALAPPDATA ms-playwright cache; on any other machine (e.g. a CI
// runner) point PLAYWRIGHT_CHROMIUM at a chromium-family executable.
import { createRequire } from "node:module";
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import { mintShare } from "../assets/share.js";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright-core");

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const targetDir = process.env.CARGO_TARGET_DIR || join(repoRoot, "target");
const EXE =
  process.env.AUSTRINGER_E2E_EXE || join(targetDir, "debug", "examples", "e2e_daemon.exe");
if (!existsSync(EXE)) {
  assert.fail(
    `AUSTRINGER_E2E_EXE resolved to ${EXE} but it does not exist — build it with ` +
      `cargo build -p austringer-web --example e2e_daemon (or set AUSTRINGER_E2E_EXE)`,
  );
}
const CHROMIUM =
  process.env.PLAYWRIGHT_CHROMIUM ||
  (process.env.LOCALAPPDATA &&
    join(
      process.env.LOCALAPPDATA,
      "ms-playwright",
      "chromium-1234",
      "chrome-win64",
      "chrome.exe",
    ));
if (!CHROMIUM || !existsSync(CHROMIUM)) {
  assert.fail(
    `chromium not found at ${CHROMIUM || "(no PLAYWRIGHT_CHROMIUM set and no LOCALAPPDATA ms-playwright cache)"} ` +
      `— set PLAYWRIGHT_CHROMIUM to a chromium-family browser executable`,
  );
}

async function startDaemon() {
  const child = spawn(EXE, ["--serve-for-e2e"], {
    stdio: ["ignore", "pipe", "inherit"],
  });
  let base = "";
  let token = "";
  await new Promise((resolve, reject) => {
    const fail = (err) => {
      clearTimeout(timer);
      child.kill();
      reject(err);
    };
    const timer = setTimeout(() => fail(new Error("daemon banner timeout")), 15000);
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
    child.on("exit", (code) => {
      clearTimeout(timer);
      reject(new Error(`daemon exited early: ${code}`));
    });
    // Both rejection paths above must never leave the child alive: a
    // live daemon keeps node's event loop spinning after exitCode is
    // set, hanging the CI step for hours (receipt: PR #43 frontends).
    child.on("error", (err) => fail(new Error(`daemon spawn failed: ${err}`)));
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

async function runAll() {
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
      // Convergence is a WAIT, not an instant read: the two HUDs render
      // on independent frames, so one can lag the other by a frame when
      // read back-to-back (CI receipt: PRs #37/#43 frontends, '14' !==
      // '15' twice). Poll until both counters agree, then assert.
      let eventsA = "";
      let eventsB = "";
      for (const deadline = Date.now() + 10_000; Date.now() < deadline; ) {
        eventsA = await a.page.textContent("[data-testid=stat-events]");
        eventsB = await b.page.textContent("[data-testid=stat-events]");
        if (eventsA === eventsB) break;
        await a.page.waitForTimeout(250);
      }
      assert.equal(eventsA, eventsB, "both browsers converged to identical event count");
      assert.deepEqual(a.errors, [], "no page errors A");
      assert.deepEqual(b.errors, [], "no page errors B");
      await ctxA.close();
      await ctxB.close();
    });

    // todo 20: dashboard drill — session history (fixture run verdict +
    // per-session cost from v2 usage frames) -> per-turn changed-files
    // timeline -> collapsed-by-default transcript focus view. The macro
    // -> micro path the acceptance criteria name. Runs on its own
    // fixture session (e2e-drill): the scripted stack serves one echo
    // turn per session, and the engine runs no inference after a
    // completed task — scenario 1 consumed session e2e's turn.
    await scenario("happy: history cost+verdict, timeline files per turn, workers, trends, focus view (todo 20)", async () => {
      const ctx = await browser.newContext();
      const a = await openSession(ctx, base, token, "e2e-drill");
      await a.page.waitForSelector(".status-open", { timeout: 10000 });

      // The e2e connection negotiates protocol version 2.
      await a.page.waitForSelector("[data-testid=proto-version]", { timeout: 5000 });
      assert.equal(
        await a.page.textContent("[data-testid=proto-version]"),
        "v2",
        "web client must negotiate protocol v2",
      );

      // Workers panel non-empty from the fixture children (team frames
      // retained in the attach snapshot).
      await a.page.waitForSelector("[data-testid=team] [data-child]", { timeout: 5000 });
      const children = await a.page.locator("[data-testid=team] [data-child]").count();
      assert.ok(children >= 2, `workers panel renders the fixture children (saw ${children})`);
      assert.match(
        await a.page.textContent("[data-testid=team-status]"),
        /in_progress/,
        "team aggregate respects the Warp precedence port",
      );

      // Run the session's turn: ask -> echo -> diff + two live v2
      // usage frames (usage broadcasts reach only live subscribers).
      await a.page.fill("[data-testid=composer]", "drill: history, timeline, focus");
      await a.page.click("button[type=submit]");
      await a.page.waitForSelector(".gate-ask", { timeout: 10000 });
      await a.page.click(".gate-ask button:has-text(\"y (once)\")");
      await a.page.waitForFunction(
        () => document.querySelector("[data-testid=transcript]")?.textContent?.includes("done: turn completed"),
        undefined,
        { timeout: 10000 },
      );

      // History: the fixture scheduled-run verdict (task-2 route)…
      await a.page.waitForSelector("[data-testid=history]", { timeout: 5000 });
      assert.match(
        await a.page.textContent("[data-testid=history]"),
        /confirmed/,
        "history shows the fixture run verdict",
      );
      // …and the per-session cost total fed by v2 usage frames (the
      // identity-bearing fixture prices each of the turn's two
      // inferences at 6720 micro-USD: exactly $0.013440).
      await a.page.waitForFunction(
        () => (document.querySelector("[data-testid=history-cost]")?.textContent || "").includes("$0.013440"),
        undefined,
        { timeout: 10000 },
      );

      // Timeline: changed files grouped under their turn.
      const timeline = await a.page.textContent("[data-testid=timeline]");
      assert.match(timeline, /turn 1/, "timeline groups changed files by turn");
      assert.match(timeline, /src\/demo\.rs/, "timeline lists the changed file");

      // Trends: fed by the v2 usage frames (model identity + cost).
      const trends = await a.page.textContent("[data-testid=eval-trends]");
      assert.match(trends, /claude-sonnet-4-5/, "trends names the serving model");
      assert.match(trends, /\$/, "trends carries cost");

      // Focus view: transcript collapses by default and expands on demand.
      const firstEntry = a.page.locator("[data-testid=transcript-entry]").first();
      assert.ok(
        await firstEntry.evaluate((n) => n.classList.contains("entry-collapsed")),
        "transcript entries render collapsed by default",
      );
      await firstEntry.locator("button:has-text(\"focus\")").click();
      assert.ok(
        await firstEntry.evaluate((n) => n.classList.contains("entry-open")),
        "focus expands the entry",
      );

      assert.deepEqual(a.errors, [], "no page errors");
      await ctx.close();
    });

    // todo 20 failure QA: an empty history renders the empty-state, not
    // a spinner that never resolves.
    await scenario("failure: empty history renders an empty-state, not a spinner", async () => {
      const ctx = await browser.newContext();
      const a = await openSession(ctx, base, token, "e2e-empty");
      await a.page.waitForSelector("[data-testid=history]", { timeout: 5000 });
      await a.page.waitForSelector("[data-testid=history-empty]", { timeout: 10000 });
      assert.match(
        await a.page.textContent("[data-testid=history-empty]"),
        /no scheduled runs/,
        "empty history names its empty-state",
      );
      const busy = await a.page
        .locator("[data-testid=history] [aria-busy=true], [data-testid=history] .loading")
        .count();
      assert.equal(busy, 0, "no spinner may be stuck on the history panel");
      assert.deepEqual(a.errors, [], "no page errors");
      await ctx.close();
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

// One full-run retry, never a per-scenario one: the fixture daemon
// serves a single scripted echo turn per session, so a partial retry
// on the same daemon would starve its turn and fail differently. A
// fresh daemon resets every session. Receipt: PR #37 — the
// two-browser convergence assertion flaked once under runner load
// and passed on rerun; this makes that class self-healing while
// still reporting the flake in the log.
async function main() {
  try {
    await runAll();
    return;
  } catch (first) {
    console.error(`E2E attempt 1 failed (${first.message}) — one retry with a fresh daemon`);
  }
  try {
    await runAll();
    console.log("E2E FLAKY: passed on retry (attempt 2)");
  } catch (second) {
    console.error(`E2E RESULT: FATAL ${second.message}`);
    process.exitCode = 1;
  }
}

main();
