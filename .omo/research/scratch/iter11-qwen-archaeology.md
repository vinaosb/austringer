# Iteration 11 — Agent B: Git Archaeology of QwenLM/qwen-code

**Date:** 2026-09-11 · **Method:** local clone `git clone --filter=blob:none https://github.com/QwenLM/qwen-code`, then unshallowed; full-DAG analysis of 9,628 commits (HEAD `28df8b8a7897b0a8490220d00280c1c17d5ad002`, 2026-09-11). All SHAs below verified locally via `git rev-parse` / `cat-file`. Permalink pattern: `https://github.com/QwenLM/qwen-code/commit/<sha>`.

> **Clone caveat (reproducibility):** GitHub served the blob:none partial clone with a shallow boundary at the tip (`.git/shallow` = HEAD; HEAD's `%P` empty). `git fetch --unshallow origin` fixed it. If you reproduce and `git log` from HEAD returns 1 commit, check `.git/shallow` first.

---

## 1. Fork divergence: what survives from Gemini CLI

### 1.1 The DAG is NOT a straight history import — it's 3 stitching operations

The repo root is **`71f8c070` "Initial commit", 2025-06-26, Binyuan Hui (`binyuan.hby@alibaba-inc.com`), parentless, tree = only 2 files** (a near-empty root, GitHub-webflow-committed). The real gemini-cli history was attached later, in three different ways:

| # | Operation | Commit | Date | Technique | What entered the DAG |
|---|---|---|---|---|---|
| 1 | Repo boot | [`71f8c070`](https://github.com/QwenLM/qwen-code/commit/71f8c070ecc9ebf4c658536b60ea46a278f24548) | 2025-06-26 | parentless root (squash of gemini-cli main snapshot follows in next commits `c5dee4bb`, `a9d6965b` "pre-release commit") | nothing yet |
| 2 | Sync v0.1.15 | [`a0dbb40d`](https://github.com/QwenLM/qwen-code/commit/a0dbb40dae8c0c80b7f29a106ef08f30dc2a66e2) | 2025-08-02 | **true merge**, 2nd parent `d666bcda` | **the entire genuine gemini-cli commit graph**, root [`add233c50`](https://github.com/QwenLM/qwen-code/commit/add233c5043264d47ecc6d3339a383f41a241ae8) "Initial commit of Gemini Code CLI" (2025-04-15, Taylor Mullen @google.com) |
| 3 | Sync v0.2.1 | [`2572faf7`](https://github.com/QwenLM/qwen-code/commit/2572faf726b94023ff089cdfc5684dccd0f35bf7) "# 🚀 Sync Gemini CLI v0.2.1 (#483)" | 2025-09-01 | single-parent (squash-style) | tree delta only |
| 4 | Sync v0.3.x | [`b99de25e`](https://github.com/QwenLM/qwen-code/commit/b99de25e38cb0bcd76483e9a47699c6fb9325f10) "Merge PR #605 from chore/sync-gemini-cli-v0.3.4" | 2025-09-17 | **true merge** | genuine gemini-cli commits incl. `gemini-cli-robot` release commits v0.3.0–v0.3.4 (2025-09-01..06) **with original SHAs and committer identity** |
| 5 | Sync v0.8.2 | [`eb95c131`](https://github.com/QwenLM/qwen-code/commit/eb95c131beb27795ac38447de0a79093c6d40aa8) "Sync upstream Gemini-CLI v0.8.2 (#838)" | 2025-10-23 | single-parent squash | tree delta only — **last upstream sync** |

**Proof of graft:** `git merge-base --is-ancestor add233c50 HEAD` → true (via merge #2's second parent), but add233c50 is NOT on the first-parent chain; `gemini-cli-robot` v0.3.4 commit `20b6246d` (author AND committer = `gemini-cli-robot@google.com`) is an ancestor of HEAD but not first-parent → entered via merge #4. Counts: full history 9,628 commits vs **5,036 first-parent** — 4,592 commits ride merged side-branches (qwen PR bubbles + grafted gemini history).

### 1.2 Package/dir evolution (first-commit vs fork-tip vs HEAD)

- First gemini commit `add233c50`: **54 files**, `packages/cli` only.
- Imported gemini tip `a2a46c7c` (2025-06-27, upstream PR #2059 "Add privacy notice slash command"): **398 files** — `packages/cli` + `packages/core` + `packages/vscode-ide-companion` (the latter two were gemini-cli's own pre-fork restructure, PR#3917-era).
- HEAD: **8,278 files**, 19 packages. **64.3% (256/398) of fork-tip paths still exist at HEAD.**
- Package identity flip: fork tip = `@google/gemini-cli` + `@google/gemini-cli-core`; HEAD = `@qwen-code/qwen-code` + `@qwen-code/qwen-code-core`. Flip happened **2025-07-23**: `daad0461` "pre-release: fix ci (#1)" (first `@qwen-code` string in package.jsons), `40ee9364` "run preflight (#55)"; stragglers fixed `b5514fd0` 2025-08-13. Note PR numbering **restarts at #1** on 2025-07-23 → fresh GitHub repo, imported history kept only as git ancestry.
- **All qwen-new packages, with first-appearance commits:** sdk-typescript `e1ffaec4` 2025-11-23 · sdk-java `5779f7ab` 2025-12-23 · docs-site `e12a80b2` 2025-12-09 · web-templates `9e4c5ee8` 2026-02-26 · channels (Telegram) `3eedc432` 2026-03-24 · docs/design `bac0ba0c` 2026-03-30 · docs/plans `cd1be1c5` 2026-04-18 · sdk-python `e3843381` 2026-04-25 (#3494) · acp-bridge `f97cb680` 2026-05-19 · docs/verification `174e8de1` 2026-05-26 · patches `77458ad2` 2026-06-03 · web-shell `531a15dd` 2026-06-12 · audio-capture `b9c5e356` 2026-06-22 · cua-driver `adc2bce4` 2026-06-26 (vendored) · chrome-extension `5581424b` 2026-06-28 ("revive") · mobile-mcp `23c6c703` 2026-07-03 (vendored) · integrations `8f667f5b` 2026-07-25 · desktop-shell `cb2555c7` 2026-08-02 · node-repl `be891657` 2026-08-23 (!breaking) · live-host `50c55355` 2026-08-25 · qwen-live `8290c81e` 2026-09-01 (voice daemon M1+M2).

### 1.3 What survived vs rewritten
Survivors at HEAD include `.github/` scaffolding, CI, and the bulk of core CLI architecture (64% path survival). The entry file survived the longest: `packages/cli/src/gemini.tsx` existed at fork tip and was only renamed **`983fafd1` 2026-08-28 "refactor(core,cli): rename generic Gemini identifiers to Llm (#10124)"** as a pure `R099` rename → `llm.tsx`. **299 files under `packages/` still contain the string "gemini" (case-insensitive) at HEAD** — de-Gemini-fication is ~95% cosmetic-complete, not literal-complete. A remote branch literally named [`4063-rename-residue`](https://github.com/QwenLM/qwen-code/tree/4063-rename-residue) documents that cleanup was still in flight ~May 2026.

---

## 2. Deprecation / removal sweeps (`--diff-filter=D` + message mining; 3,427 deleted-path log lines)

- **Qwen OAuth free tier discontinued — exactly 2026-04 as briefed.** Announcement `53a9c0a2` 2026-04-13 "docs(readme): add announcement for Qwen OAuth free tier policy adjustment (#3207)"; cutover [`f6271c61`](https://github.com/QwenLM/qwen-code/commit/f6271c61b63508a5ed293ab4421ff283f9e18917) 2026-04-15 "feat(auth): discontinue Qwen OAuth free tier **(2026-04-15 cutoff)** (#3291)". Long tail of corpse-burial: `5743c2a0` 2026-05-31 "drop discontinued Qwen OAuth method (#4639)", `f4686e87` 2026-06-12 "time out Qwen OAuth refresh (#4829)", `b39fc0d8` 2026-06-17 "hide unconfigured discontinued OAuth model (#5167)", `442a697b` 2026-07-23 "hide discontinued oauth model (#7522)" — the dead auth method haunted ACP/CLI code for 3+ months after cutover.
- **Electron desktop app removed after being forked away:** [`ce72ddbe`](https://github.com/QwenLM/qwen-code/commit/ce72ddbe6cfe3c10fe51a0f7cd0116320b690529) 2026-08-25 "feat(desktop): remove packages/desktop **after OpenWork fork**; keep the Tauri upgrade bridge (#9085)"; cleanup `d526612e` 2026-08-27 (#10021, "drop stale packages/desktop references after Electron removal"); orphan rescue `50c55355` 2026-08-25 "restore Live Host after desktop removal (#9994)". Replacement: Tauri-based `desktop-shell` (added 2026-08-02, tags now at `desktop-v0.3.0`).
- **WebUI built and retired within ~8 months:** introduced (renamed into `packages/webui`) `ec0586b1` 2026-01-14; retired [`077c57d2`](https://github.com/QwenLM/qwen-code/commit/077c57d2005b18d43a11c72c16505d56ddc53a77) 2026-09-05 "refactor!: retire @qwen-code/webui (#9812)" — breaking change, mass deletion; CI guard removed `c516740a` 2026-09-05 (#11095); regression coverage re-landed `32f51a33` 2026-09-08 (#11101).
- **Tool/setting renames:** `verboseMode` → `compactMode` `746f67f4` 2026-04-10 (#3075); approval mode "Default" → "Ask permissions" `16c78652` 2026-06-03 (#4674); TodoWrite display name → "TodoList" `7cd49e06` 2026-06-19 (#5319); Gemini identifiers → `Llm` `983fafd1` 2026-08-28 (#10124); residue ids (memory/spinner/leaf) `b89d00ec` 2026-08-27 (#9900).
- Micro-retirements in final week: trusted hooks manager `ae78d5b8` 2026-09-11 (#11621); channels message-prefix filtering `b3ebe1e8` 2026-09-10 (#11571); BlockStreamer `cfb173ec` 2026-09-09 (#11381).

---

## 3. Reversals (66 commits matching revert/Revert; `all.txt` message mine)

- **Thinking-parsing flip-flop (Qwen3/GLM):** `e3076e29` 2026-07-03 "revert GLM tagged thinking parsing (#6248)" → `ca46d6dd` 2026-07-12 Revert "Preserve tagged thinking parsing for Qwen3 default provider (#6751)" (#6767) → finally re-landed as `359bda4b` 2026-09-01 "support hybrid thinking streams for Qwen3 (#9607)". A feature needed 3 attempts over 2 months.
- **Cluster revert-storm Jul 2026:** `ff7d48a6` 2026-07-13 Revert "retry malformed streamed responses (#6754)" (#6783) · `515a8311` 2026-07-14 Revert "mark non-zero exits as failed (#6869)" (#6875) · `cd9749a8` 2026-07-17 revert(vscode) undo Electron Node mode changes from #6866 (#7067) · `e3cf1c3b` 2026-07-26 revert stale-base un-park recovery (#7602→#7640) · `789a6691` 2026-08-18 revert(web-shell) restore pre-#8098 composer animations (#9349).
- **ACP feature whiplash:** `aea34fa2` 2026-06-07 Revert "enable /remember, /forget, /dream in ACP mode (#4811)" (#4818) — shipped and reverted in 4 days (4811→4818).
- **Architecture reverts:** `663967e1` 2026-06-23 "revert Protocol enum & model-identity decoupling (#5089) (#5745)".
- **In-place re-revert:** `c5bf2224` 2026-08-14 "fix(webui): revert #8882's transactional session switching to the loading-skeleton model (#9129)".
- CHANGELOG.md archaeology: hand-written from `df1479f8` 2025-08-15 ("Chore/release 0.0.7 (#343)"); current file is **auto-generated from GitHub Releases** ("Do not edit it by hand — run `npm run changelog`"), Keep-a-Changelog 1.1.0 format.

---

## 4. Cadence + governance

**Commits/month** (9,628 total; merges 816): 2025-04: 220 · 05: 329 · 06: 565 · 07: 504 · 08: 713 · **09: 121 · 10: 50** (sync-era trough, incl. v0.2.1→v0.8.2 rebases) · 11: 172 · 12: 480 · 2026-01: 597 · 02: 460 · **03: 871 (peak)** · 04: 567 · 05: 362 · 06: 764 · **07: 1,312** · 08: 1,074 · 09(partial): 467.

**Governance regime shift — merge commits stop dead after 2026-04:** merges/month: 2025-08: 40 · 09: 19 · 10: 6 · 11: 20 · 12: 101 · 2026-01: 164 · 02: 126 · **03: 263** · 04: 48 → **2026-05-01→today: 0 merges, 3,976 non-merge commits.** All PRs are squash-landed since May 2026 (verified `git rev-list --count --merges --since=2026-05-01` = 0). The blog's "320+ PRs in one week" is consistent with squash-flow at ~1,000+ commits/month; direct merge-count verification of that exact figure: UNVERIFIED.

**Tags:** 724 total. First qwen tags `v0.0.1-alpha.14` / `v0.0.2` on 2025-08-01. Tags/month: 43 (2025-08) … 67 (2026-01) … 76 (2026-07) · **109 (2026-08)** · 24 (2026-09 partial). Weekly stable + nightly tags confirmed (`v0.23.3-nightly.20260910.c46cb85cf2` style, SHA-embedded nightlies).

**Contributors (632 distinct author emails, non-merge):** top = Shaojin Wen (`shaojin.wensj@alibaba-inc.com`, 952 across two identities) · tanzhenxin 665 · jinye 419 · qqqys 333 · yiliang114 333 · mingholy.lmh (alibaba-inc) 236 · bots `qwen-code-dev-bot` 136 + `qwen-code-ci-bot` 60 (both `@service.alibaba.com`). Domain mix, all history: qq/126 23.7% · **alibaba 17.9%** · @google.com 9.6% (mostly imported era) · gh-noreply 12.1%. Qwen era only (since 2025-07-23): **alibaba 21.3%, qq/126 28.2%, google 2.5%** (187 — includes grafted gemini-robot commits + early goodwill commits like Allen Hutchison's `718f68d2` 2025-07-30 "/bug → Qwen-Code repo (#154)"). Imported-history handling preserved Google authorship verbatim (Taylor Mullen 125+87, Tommaso Sciortino 114, Brandon Keiji 103, Allen Hutchison 68 commits).

---

## 5. Five juiciest findings

1. **The repo wears a imported-history costume built from 3 different stitches** — a parentless Alibaba root (`71f8c070`, 2025-06-26, 2-file tree), one true merge that grafted the entire genuine gemini-cli graph (root `add233c50`, 2025-04-15) via the v0.1.15 sync (`a0dbb40d`, 2025-08-02), a true-merge v0.3.x sync (`b99de25e`, PR #605) that carried **original gemini-cli-robot commit objects** (v0.3.0–v0.3.4, original SHAs/committers) into the DAG, and squash-style syncs (v0.2.1 `2572faf7`, v0.8.2 `eb95c131`). Not a single clean import — a frankengraph assembled over 4 months. Evidence: §1.1 table; `merge-base --is-ancestor` results in §1.1.
2. **"Stopped syncing at v0.1" (brief/blog) is wrong — the last sync was v0.8.2 on 2025-10-23** (`eb95c131`, #838), and it wasn't even the only late one (v0.2.1 2025-09-01, v0.3.x 2025-09-17). The trough in 2025-09/10 (121/50 commits) is the sync churn period, not inactivity. Independence is therefore only ~11 months old, not ~15.
3. **The OAuth free tier death is precise and messy:** announced 2026-04-13 (`53a9c0a2` #3207), executed 2026-04-15 (`f6271c61` #3291, message literally contains "2026-04-15 cutoff"), then 4+ "hide/drop discontinued OAuth model" cleanup PRs through 2026-07-23 (`5743c2a0`, `f4686e87`, `b39fc0d8`, `442a697b`) — a discontinued auth method's UX ghosts lingered 100+ days.
4. **The desktop app was forked OUT of the repo mid-life:** `ce72ddbe` 2026-08-25 removed `packages/desktop` (Electron) "after **OpenWork fork**" — i.e., the app was spun out to a separate OpenWork project — while the repo pivoted to a Tauri-based `desktop-shell` (added 2026-08-02 `cb2555c7`, now at tag `desktop-v0.3.0`), and had to rescue the orphaned Live Host (`50c55355` #9994). An entire Electron app entered and left the repo within weeks.
5. **The Great De-Gemini finished only on 2026-08-28:** `gemini.tsx`→`llm.tsx` was a pure R099 rename in `983fafd1` (#10124) — **14 months after the fork** — preceded by residue hunts (`b89d00ec` #9900, branch `4063-rename-residue`) and still 299 files under `packages/` contain "gemini" at HEAD. Also in reversal-land: the Qwen3/GLM thinking-parser needed land→revert→revert→re-land over Jul–Sep 2026 (`e3076e29`→`ca46d6dd`→`359bda4b`).

---

## 6. Brief-claim verification

| Brief claim | Verdict | Evidence |
|---|---|---|
| created 2025-06-26 | ✅ | root `71f8c070` dated 2025-06-26 |
| "importing Gemini CLI v0.8.2 history" at creation | ❌ **corrected** | v0.8.2 was the *last* sync (`eb95c131`, 2025-10-23 #838); initial import = gemini-cli main snapshot ~2025-06-26 (upstream tip `a2a46c7c` 2025-06-27, upstream PR #2059, ≈v0.1.x era); gemini history grafted 2025-08-02 (`a0dbb40d`) |
| "stopped syncing with upstream at v0.1" | ❌ **contradicted by git** | syncs at v0.1.15, v0.2.1, v0.3.x, v0.8.2 (see §1.1); independence starts 2025-10-23 |
| releases ≈weekly + nightlies | ✅ | 724 tags; 50–109 tags/month through 2026; nightly tags embed SHA (`v0.23.3-nightly.20260910.c46cb85cf2`) |
| v0.23.3 current | ✅ | tag `v0.23.3` + CHANGELOG heading "0.23.3 - 2026-09-10" |
| 320+ PRs merged in one week | ⚠️ UNVERIFIED from git (merge commits don't exist post-2026-04; commit volume ~1,000–1,300/month Jul–Aug 2026 is consistent) | §4 |
| OAuth free-tier removal ~2026-04 | ✅ exactly 2026-04-15 | `f6271c61` #3291 |
| docs/design + docs/plans + docs/verification dirs | ✅ | first commits `bac0ba0c` 03-30 / `cd1be1c5` 04-18 / `174e8de1` 05-26 |
| tool renames / setting renames | ✅ | verboseMode→compactMode, Default→Ask permissions, TodoWrite→TodoList, Gemini→Llm (§2) |

*Analysis artifacts (dates, authors, deletions, merges, tags dumps) kept in `%TEMP%\opencode\*.txt`; clone at `%TEMP%\opencode\qwen-code`.*
