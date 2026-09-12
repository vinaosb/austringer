# Agent A — Gemini CLI Git Archaeology (Iteration 22)

**Repo**: google-gemini/gemini-cli | **HEAD at analysis**: `9c1b0a610534d6f8120964cf2672c07807d8fc90` (Sep 11 2026)
**Method**: `gh repo clone --filter=blob:none`, full history metadata, git log/show/ls-tree/tag analysis.
**Date of analysis**: 2026-09-12. All SHAs below verified locally via `git rev-parse`. PR numbers are from commit subjects; PR URLs constructed as `https://github.com/google-gemini/gemini-cli/pull/<N>` — UNVERIFIED individually.

---

## 1. Security-Hardening Arc (Jun–Sep 2026)

Post-cutoff, security fixes are the **dominant commit type** (~40% of non-robot commits since Jun 18). Verified sweep:

| Date | SHA | Fix | PR |
|------|-----|-----|-----|
| Sep 11 | `9c1b0a610534d6f8120964cf2672c07807d8fc90` | sandbox: harden filesystem boundaries + isolate runtime state | #29214 |
| Sep 11 | `bfb71fd2b8ed9ed94b0e1d79275b4c911a97fa27` | prevent indirect prompt injection via build-file modifications & untrusted flags (policy-engine + DiffRenderer/ToolConfirmation UI) | #29250 |
| Sep 08 | `c647533d6c017d032420e032f932953d7df900dc` | enforce envelope metadata provenance for untrusted tool outputs | #29215 |
| Sep 08 | `593db1684924a652b01703a3e41d1b78bfa29506` | isolate settings directory in sandbox containers | #29216 |
| Sep 08 | `24cab6830a9be146fdcb37a8b654f4c203b54863` | mitigate NTFS 8.3 short-name (SFN) path bypass in `safety/built-in.ts` | #29116 |
| Sep 04 | `85aca163f6c73ac6ce380b5447359146b8adcae4` | strict permission/ownership checks on system-wide config paths | #29115 |
| Sep 04 | `567afbbe8fc7927e38bd40733bf09cafc227ede4` | workspace path boundary checks + symlink resolution in command safety | #29170 |
| Sep 04 | `60d35f48a84a32115dc7095f2a59ab47ab467988` | extensions: consent on env changes + sanitize runtime-altering env vars | #28863 |
| Sep 03 | `87a9c71d57a4ec56c00f3ff628970fea8291d812` | **remove hardcoded Google CrUX API key** in chrome-devtools-mcp bundle | #29158 |
| Sep 03 | `f7f6501efe38cdd78761666e6f27cb08f786b0af` | harden path resolution/boundary validation in extension loader | #29169 |
| Sep 03 | `e148d088c1dd54a13aac82966f2a92f30aefc8df` | isolate temp dir for macOS Seatbelt sandbox | #29171 |
| Sep 02 | `55b495d6db1794bf5b7f37a9bc03ebcab5103673` | enforce RFC 9207 issuer identification in MCP OAuth flow | #29117 |
| Sep 01 | `4963a4456bd8386922c6c07855056e1783822f29` | destination validation + connection routing in web-fetch utilities | #29120 |
| Aug 28 | `0bd1d439751478771c45d3d0895a6a9760554bf4` | **fail-closed workspace trust** + filter mcpServers in restricted mode | #29099 |
| Aug 26 | `3c311beac2e78336816dd4a123db39743f9fbf85` | **prevent SSRF in MCP OAuth metadata discovery/auth** (643 insertions) | #29081 |
| Aug 24 | `af687798ae095199438253862763516790218769` | declare top-level safety checkers in write-policy config | #28961 |
| Aug 21 | `5411f113cafae26161b4969b0237b8e1e024e2c2` | isolate Docker/container runtime sockets + binaries in Seatbelt | #28935 |
| Aug 20 | `ba4296c6cbd2f68343722c6b28a85848e71781a6` | consistent symlink evaluation in ignore-path handling | #28915 |
| Jul 31 | `acae7124bdd849e554eaa5e090199a0cf08cd782` | mitigate infinite ReAct loops **and prompt-injection loops** | #28429 |
| Jul 21 | `c776c665b00a39d55c470beb788a2b9a77a2feb7` | a2a-server: enforce workspace trust + task isolation to **prevent RCE** | #28470 |
| Jul 17 | `a96259c9e5f36f5d4dd8c51a89374d9e53f514d6` | caretaker: sanitize issue title wrapped in `untrusted_context` | #28352 |
| Jul 09 | `69e0c2659e55a868235ed9189b7b4a79f335a0a5` | align permissive Seatbelt profiles with **deny-default** model | #28424 |
| Jul 07 | `892b35fcfb6ab8e192e51c603583279c99e8b0a4` | make `~/.gitconfig` read-only in macOS sandbox | #28221 |
| Jul 07 | `ff00dacd9f33d9842cfc3b5d6c53a1b2cbc44636` | resolve symlink directory escape in memory-import processor | #28233 |
| Jun 25 | `ae0a3aa7b928cc73bb09604bb9c2c020e6b647db` | **case-insensitive** sensitive-path blocklist + VS Code HITL | #27966 |
| Jun 21 | `bca5667fc65517b0a328ae3f0746fee274c059bf` | prevent path traversal during skill install | #27767 |

### Emergent threat model (synthesis)
Six attack-surface clusters, in priority order:
1. **Prompt injection as a filesystem attack**: indirect injection via build files (`bfb71fd2b`), untrusted tool-output provenance envelopes (`c647533d6`), injection loops (`acae7124b`), even the repo's own automation wraps issue titles in `untrusted_context` (`a96259c9e`). Model: repo content + tool output = attacker-controlled.
2. **MCP servers as hostile identity providers**: SSRF in OAuth metadata discovery (`3c311beac`), RFC 9207 issuer enforcement (`55b495d6d`), stored-client-ID token refresh, fail-closed trust filtering mcpServers in restricted mode (`0bd1d4397`).
3. **Windows promoted to first-class attack surface**: NTFS 8.3 SFN blocklist bypass (`24cab6830`) — path-obscurity tricks to defeat safety checks; case-insensitive blocklist (`ae0a3aa7b`).
4. **Symlink/sandbox escape everywhere**: memory-import escape (`ff00dacd9`), extension loader boundaries (`f7f6501ef`), skill-install traversal (`bca5667f`), settings isolation (`593db168`), config ownership checks (`85aca163`).
5. **OS-level containment over LLM trust**: Seatbelt deny-default profiles, Docker socket isolation (`5411f113c`), read-only `~/.gitconfig` (`892b35fcf`) — agent runtime files are attack targets.
6. **Secret hygiene in the supply chain**: a real hardcoded Google CrUX API key shipped in the chrome-devtools-mcp bundle (`87a9c71d5`, now guarded by `cruxKeyLeak.test.ts`).

**Net**: Gemini CLI post-cutoff is modeled as a **sandboxed execution engine under active attack by hostile repos, MCP servers, and extensions** — defense moved from "ask the user" to OS-enforced boundaries + policy engine (`packages/core/src/policy/policy-engine.ts` hardened in `bfb71fd2b`) + fail-closed trust.

## 2. Governance Arc

**Velocity collapse after the Jun 18 2026 consumer-OAuth kill (verified counts):**
- Jan 1 – Jun 18, 2026: **2,519 commits** (~29/day)
- Jun 18 – Sep 12, 2026: **148 commits** (~1.7/day) → **~94% drop**
- Total repo: 6,411 commits. Since Oct 1 2025: 3,800 (59% of all history is 11 months old).

**Who commits now** (post-cutoff, non-robot, n≈119): ~11 of 20 named authors carry **@google.com emails** — and the hyphenated usernames are all Googlers: `luisfelipe-alt`→luisfelipe@google.com, `joneba-google`→joneba@google.com, `jvargassanchez-dot`→jvargassanchez@google.com, `jesussamuel-byte`→jesussamuel@google.com, `villahernandez-coder`→villahernandez@google.com, `urielefrenvirtusa`→urielefren@google.com, `amelidev`→ameliesther@google.com, `Chad`→chaddiao@google.com. The original core team is **gone**: H1-2026 leaders Abhi (133), Tommaso Sciortino (129), N. Taylor Mullen (115), Sehoon Shon (108), Jacob Richman (107) have ~0 post-cutoff commits; only Gal Zahavi, Ramón Medrano Llamas, Adam Weidman, Sandy Tao persist. UNVERIFIED (staffing reason): whether this is rotation or reassignment.

**The repo now partially maintains itself:**
- `[SSR Agent] Issue Fix (NNNNN): ...` commits — 10+ commits Aug 13–18 alone by joneba@google.com driving an **automated issue-fixing agent** writing real fixes (e.g., `571851b10`, `9a15c45fb`).
- `tools/caretaker-agent` born **Jun 30, 2026** (`7f00c5fe592c1d41893b2053f7567381641abdbd`, #28015, "Cloud Run webhook ingestion service") — days after the OAuth cutoff. Contains `cloudrun/`, `workflows/`, `evals/`, `scripts/`; caretaker-evals add triage judge-runner (#28530/#28532).
- `pr-generator-*` packages (agent/core/db/orchestrator/infra, late Jul 2026): a Cloud Run **state machine that generates bug-fix PRs** with an "Antigravity agent runner" (`c5622fec2`, `b6b41f79e`, `fccc043bd`, `bde504f25`).
- Roadmap moved to a **live GitHub Project**: ROADMAP.md defers to `orgs/google-gemini/projects/11`; docs/ gained `admin/enterprise-controls.md` post-fork.

## 3. a2a-server Package (Agent-to-Agent)

- Born **Aug 26, 2025**: `bfef867ba7a5e785bab101d51a23e98698ee749c` (#6597, Adam Weidman) — wraps the CLI agent as an **A2A-protocol server** (agent.ts 785 lines, task.ts 930, GCS persistence) so other agents/services can drive Gemini CLI tasks over HTTP.
- Design RFC followed Sep 5, 2025 (#7817, Shreya Keshive, `20d0de74e`). 247 commits to the package since.
- Enterprise-shaped: GCS-backed persistence, extension config, Cloud Run deployable.
- **Security arc within it**: Jul 21, 2026 RCE-class fix `c776c665b` (workspace trust + task isolation; 936-line executor rewrite + a test literally named `rce_vulnerability.test.ts`); CRLF normalization `b94c9775b`; stale-cancellation fix `5dd4919e6`. Treats *each task as an untrusted tenant*.

## 4. Fork Divergence (Qwen base vs current HEAD)

- Qwen-era base: tag `v0.8.2` = `0612839ab6a1d33074f866f07b67d333393cc643`, **Oct 10, 2025**.
- **3,794 commits** between `v0.8.2..HEAD` (59% of the repo's entire history happened after the fork base).
- Packages at v0.8.2 (5): a2a-server, cli, core, test-utils, vscode-ide-companion.
- Packages at HEAD (7): **+ sdk** (Feb 12, 2026, `bed3eae0e160aa85cf1f06a841bc015fc59d4d2c` #18861, Michael Bleigh), **+ devtools** (Feb 18, 2026, `65ad78b9c048f3b362b00740602af1a4d0f9d8fe` #18936, Sandy Tao).
- Also post-fork: `evals/` (Jan 14, 2026, `8030404b0` #16047), `integration-tests/`, `memory-tests/`, `perf-tests/`, `docs/` (incl. enterprise-controls), `tools/caretaker-agent`, `schemas/`, `sea/`.
- Verdict: the Qwen fork diverged from a monorepo that no longer exists — SDK packaging, policy engine, safety checkers, caretaker automation, and the whole hardening wave are upstream-only.

## 5. Release Engineering

- **Tuesday stable cadence verified from tags**: v0.59.0 Tue Sep 8; v0.58.0 Tue Sep 1; v0.57.0 Tue Aug 25; v0.55.1 Tue Aug 11. Exceptions are patch trains (v0.56.0 Wed Aug 19; v0.54.4 Fri Aug 7; v0.53.1 Fri Jul 31).
- **Nightlies embed the commit SHA**: `v0.61.0-nightly.20260908.gc647533d6` (tag/bump `fa1cff800`), i.e., nightly ↔ HEAD SHA traceability by construction; robot bump commits `chore(release): bump version to X-nightly.YYYYMMDD.g<sha>`.
- `gemini-cli-robot` (gemini-cli-robot@google.com) executes all bumps + `Changelog for vX.Y.Z` commits (29 post-cutoff).
- 15 release workflows in `.github/workflows/`: 4-stage patch pipeline (`release-patch-0-from-comment` → `-3-release`), `release-rollback.yml`, `release-promote.yml`, `release-sandbox.yml`, `verify-release.yml`, `release-nightly.yml`, `release-manual.yml`.
- **Self-hosted AI release notes**: `release-notes.yml` explicitly "uses Gemini to generate release notes" and opens a PR on every published release. Patch releases are triggered **from comments** on issues (patch-0-from-comment).
- Supply-chain hygiene: `.allstar/` (Google Allstar security bot), `verify-release.yml`, `02c6c7732` "prevent workspace binary shadowing in release verification", `686314872` npm dist-tag deletion handling.

## 6. Five Juiciest Findings (SHAs)

1. **`87a9c71d57a4ec56c00f3ff628970fea8291d812`** — A **hardcoded real Google CrUX API key** shipped inside the chrome-devtools-mcp browser bundle; sanitized Sep 3, 2026 with a regression test named `cruxKeyLeak.test.ts`. Security posture caught its own secret leak. [#29158]
2. **`c776c665b00a39d55c470beb788a2b9a77a2feb7`** — a2a-server had an **RCE-class** flaw: remote A2A tasks could run without workspace trust; fix rewrote 936 lines of executor and ships `rce_vulnerability.test.ts`. [#28470]
3. **`24cab6830a9be146fdcb37a8b654f4c203b54863`** — **NTFS 8.3 short-name bypass**: Windows `FOO~1` aliases could dodge the sensitive-path blocklist in `safety/built-in.ts`; fixed Sep 8, 2026 alongside a case-insensitive blocklist (`ae0a3aa7b`). Upstream now treats Windows path obscurity as a real prompt-injection enabler.
4. **`7f00c5fe5` + `[SSR Agent]` pattern** — the repo **hires its own maintainers**: caretaker-agent Cloud Run service (Jun 30, 2026) triages issues, an SSR Agent lands fixes with issue-number-titled commits, and pr-generator-* runs a bug-fix PR state machine with an Antigravity agent runner. ~29 robot + 18 SSR-Agent-titled commits in the last 86 days.
5. **`3c311beac2e78336816dd4a123db39743f9fbf85`** — **SSRF in MCP OAuth discovery**: a malicious MCP server's metadata URLs could make the CLI's OAuth flow talk to attacker-chosen endpoints; fixed Aug 26, 2026 (643 insertions, co-authored by David Pierce) and followed by RFC 9207 issuer enforcement (`55b495d6d`) — MCP servers are formally treated as hostile IdPs.

## Verification Notes

- All short SHAs above were expanded and confirmed via `git rev-parse`; author emails read from commit objects directly.
- Commit *messages* verified; diffs spot-verified via `git show --stat` for the five juicy findings.
- UNVERIFIED: (a) individual PR-page contents (PR numbers from squash-merge subjects); (b) *why* the original core team stopped committing (rotation vs reassignment); (c) the exact commit that killed consumer OAuth — no commit in May 15–Jul 15 windows matches a "remove OAuth" title; the removal appears to have shipped via docs/announcements rather than an identifiable commit (consistent with "no grace period" enterprise-only messaging). Closest June-adjacent auth commits: `fbce3e51b` (Jun 16, GDC air-gapped service identity), `58ba19945` (Aug 11, Cloud Workstations OAuth redirect).
- Clone note: 4 long-filename snapshot files failed initial Windows checkout; `core.longpaths=true` fixed it (irrelevant to history data).
