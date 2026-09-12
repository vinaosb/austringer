# Iteration 16 — Agent B: Git Archaeology of zed-industries/zed

**Method**: `git clone --filter=blob:none --no-checkout` (blobless, full history) into temp; excavated via `git log --grep`, reverse log on crate paths, tag-date extraction, and GitHub API cross-checks. All SHAs below resolved to full 40-char hashes and verified against the local clone on 2026-09-11.

---

## 1. Agent-Feature Archaeology

### Panel lineage: `assistant2` → `agent` (the founding rename)
- `1de02cf6e5` — 2023-12-06 — "Start wiring up assistant2". The agent panel began as a dormant experiment crate alongside the original Assistant.
- Explosion of `assistant2:` commits Apr 29–May 2, 2024 (#11191–#11315: chat tools, create-buffer tool, storybook) — then it goes quiet; reactivation wave visible from Jan 2025 (`f096a28a19` 2025-01-27 `assistant2: Don't block ThreadStore initialization…` #23728).
- **The rename moment**: `dc83f1ad38cd5b3cd3c3a5d165937bed8201e9ce` — 2025-04-01 — "Rename `assistant2` to `agent` (#27887)". Nomenclature commits cluster Mar 31–Apr 1, 2025 (`e63df58adc` #27859 key context → `AgentPanel`; `133d3f052f` #27864 text → "agent"). Same-day `Agent Panel` bug-report template added (`85b811a783` 2025-04-07 #28226).
- This is the clearest single commit showing Zed's "assistant → agent" product pivot: April 1, 2025, ~3 months before the public agentic-editing launch.

### Subagents / spawn_agent lineage
- First landing (NOT the PRs in the task brief): `acbb32d19fd2cbdcc3cd6fe6af11ae1eab534209` — 2026-01-08 — "Subagents PR 1: Feature flag + tool skeleton (#46186)"; `756637fef5` 2026-01-12 "Subagents PR 2: Thread spawning + execution (#46187)"; `581e559002` 2026-01-14 "Render subagents in thread (#46188)".
- `66bdb116d0023f48f413dde7ff312bd666c4db05` — 2026-02-20 — "agent: Change tool name to spawn_agent (#49741)"; warning removed `77e68b4e41` 2026-02-23 (#49903).
- **Feature-flag removal (GA)**: `12b786dffc39bbc0eb109498e5a3f5149e6bd5de` — 2026-03-02 — "agent: Remove subagents feature flag (#50493)". Jan flag → Mar GA: 8 weeks.
- Follow-ups: retained/background threads unified `55e47138e9` 2026-04-13 (#53737); `ThreadMetadataStore` cache `09b0388312` 2026-03-19 (#51923); `AgentConnection` trait abstraction `291611e3db` 2026-01-12 (#46631); subagent edit regeneration fix `c56914d8f2`-adjacent `c78bd36fd8` 2026-06-10 (#59060).

### The three PRs from the task brief — **all CLOSED, none merged** (verified via API)
| PR | Author | Opened | State | Note |
|---|---|---|---|---|
| #54084 "sub-agent delegation depth configurable with depth-aware thread store" | GitGlimpse895 | 2026-04-16 | **closed, unmerged** | |
| #54142 "enforce MAX_SUBAGENT_DEPTH constant… ThreadStore subagent-tree visibility" | GitGlimpse895 | 2026-04-17 | **closed, unmerged** | |
| #54213 "Add create_thread and list_agents_and_models agent tools" | eholk (Eric Holk, Zed staff) | 2026-04-17 | **closed, unmerged** | Superseded by own rework |
- The create_thread idea DID land, but as **#57987** — merge commit `59f32d2135a9c9816d46e4e98cc55e7103423aa4`, merged 2026-06-01T23:47Z, author eholk. Six-week incubation from first attempt to merge.
- **UNVERIFIED**: the "noted as exemplary docs" praise for #54084/#54142/#54213 — plausible (they are detailed design PRs) but I found no in-repo text saying so; treat as external hearsay.
- Related governance signal: `f36aec822be697df9049fed020b593147c93b4cf` — 2026-08-21 — "Disable ask_user by default (#63036)" and `b9301f5c94` 2026-07-29 "agent: Instruct agents to request approval before creating threads (#61898)" — the tool surface is actively re-scoped by defaults, not just additions.

### Parallel Agents — April 2026 landing confirmed
- `a0d0195ca9130b43409704898d27a9770b0d3edb` — 2026-04-07 — "Add onboarding for parallel agents (#52940)".
- Copy iteration `a706e93ff8` 2026-04-08 (#53423); onboarding simplification `74de476364` 2026-04-15 (#53854); telemetry `e69b112755` 2026-04-15 (#52919); **docs** `2d30f95676a812643ff3d89de9c4a0f636fa1969` 2026-04-22 "Add Parallel Agents documentation (#53535)"; conditional announcement `33ebeb9850` 2026-04-23 (#54571, suppressed when AI disabled); skills toast reuses announcement infra `1ddf7407e9` 2026-05-19 (#56873).
- Cadence signature: 6 UX/docs/telemetry commits in 17 days around launch — textbook launch choreography.

### Terminal Threads
- Surface: `fd5d42dd55` 2026-06-16 "Add terminal thread init command (#59374)"; `4aa8ad9742` 2026-07-02 "Make terminal threads searchable (#60292)"; sidebar renaming `9785475caa` 2026-08-31 (#63494); panel renaming `251854020a` 2026-09-11 (#63660, today — still actively landing).
- Related regression: duplicate-terminal bug fixed `af56cb75a8` 2026-07-10 (#59586), then that fix **reverted** `7da35f5774` 2026-07-15 (#61042) — see §2.

### ACP (Agent Client Protocol) — git timeline vs. public launch
- Crate first appears: `495ec7a109c4e87511f16b52d943f95390ca246f` — 2025-07-09 — "ACP (#34030)" (crates/acp, "Update to acp 0.0.6" `08ffd9884a` same day).
- Rapid internal buildout: `8e4555455c` 2025-07-17 "Claude experiment (#34577)"; `cfe1adc792` 2025-07-18 "E2E Claude tests (#34702)"; `405244d422` 2025-07-21 "Display ACP plans (#34816)"; `2d0f10c48a` 2025-07-24 "Refactor to use new ACP crate (#35043)"; `c2fc70eef7` 2025-07-28 "ACP over MCP server impl (#35196)".
- **Insight**: the protocol was prototyped internally ~6–7 weeks before the public Aug 2025 launch, against Claude Code as first external agent. Continues: `8ba35e5eac` 2026-06-22 "acp: Update agent client protocol crate to 0.15.0 (#59593)". Non-Anthropic ecosystem now visible: `8c259313dc` 2026-08-05 "Document Poolside external agent setup (#62225)", OpenCode bell plugin guard `bc99075373` 2026-07-11 (#60507).
- **UNVERIFIED (external date)**: exact Aug 2025 public-protocol announcement day; git only proves internal commits from 2025-07-09.

---

## 2. Reversals / Deprecations (2025–2026)

### True reverts of substance (2026, ~1/month on user-visible surface)
- `4b407d0fe4c6a8a129cc9bb82801c0af4d75f077` — 2026-08-05 — "editor: Revert punctuation word movement changes (#62213)"; re-landed differently `101ca00a13` 2026-08-06 (#62224, "Align selections… punctuation as separate word boundaries").
- `6a747984d3f37ba0ccfcedd96b8048877a7941a0` — 2026-05-25 — "Revert 'Rename agent threads from the sidebar' (#57654)" — then re-shipped properly as `9785475caa` 2026-08-31 (#63494). Revert→re-land after 3 months.
- `cf76418cf4f1eec5d91e416d35b76b7af53e17e5` — 2026-06-23 — "collab: Revert livekit changes (#59733)" — **evidence collab/live-sync substrate is still maintained and experimenting with LiveKit**.
- `492acd6c81` 2026-08-10 — Revert "terminal: Actually close process groups…"; `b5c2d8a13f` 2026-07-02 — Revert "Fix hanging updates after system sleep (#60301)" (re-fixed via `7a7c3e1d2f` 2026-08-19 "Restart interrupted update downloads after system wake (#60366)"); `5c215b3613` 2026-07-20 — Revert untitled-buffer language detection; `eeac1cfa21` 2026-07-21 gpui_web context-menu revert; `da87558cc2` 2026-05-29 — Revert "Improve ChatGPT subscription response resilience"; `4d94097df0` 2026-06-21 — Revert Finder default_open_behavior; `edd906859c` 2026-07-27 — Revert force hardmark doc hovers; `fca2ccd403`/`ec7c11c65c` macOS 27 Beta titlebar-click ping-pong (fix, revert, re-fix within 12 days).

### Feature removals / retirements
- `b3bcfc6060` 2026-04-09 — "Remove `Fix with Assistant` (#53521)" (cole-miller) — old assistant UX surface deleted post-pivot.
- Hosted-model retirement surfaced in docs: `71ffaeb817` 2026-02-18 "Update AI docs for retired hosted models (#49486)" — **UNVERIFIED detail**: which models; commit title only.
- Inline assistant rework: `5cd30e5106` 2025-12-05 "inline assistant: Use tools and remove insertion mode (#44248)" — removal of a UX mode, not just code.
- Language-server-level removals: none found in 2025–2026 window via grep; **no** mass LSP removal detected (gap: only subject-line search — deep diff audit not performed).

### Old Zed collaboration state
- `crates/collab` is **alive** (touched 2026-09-11, `6fff327a90` "Bump Rust to 1.98.1"). No "live share" removal found. LiveKit experiments reverted rather than the feature dropped — Zed keeps its own collaboration stack rather than deprecating it.

---

## 3. Performance Discipline as Git History

- **Frame-time lineage**: `de64de22a3` 2024-01-22 "Introduce a `ZED_MEASUREMENTS` env var… to measure frame time" → `914b0117fb` 2025-12-16 "Optimize editor rendering when clipped by parent containers (#44995)" → `07a0bd1220bddb940028d610e40319a459dd49fb` 2026-08-17 "gpui: Frame time debug overlay (#62749)". A 2.5-year continuous measurement thread.
- **Benchmark infrastructure ramp (Aug 2026 burst)**: `7316cf7745` 2026-08-21 "gpui: Report foreground executor work in bench reports (#63035)"; `1f7eae7fb7` 2026-08-26 "gpui: Separate benchmark support from test support (#63039)"; `fecc3273ed` 2026-08-26 "gpui_platform: Expose headless renderer through bench support (#63244)"; headless Metal renderer fixes `fb38178d74` 2026-09-11 (#64099). Bench infra is being productized right now.
- **Subject-line counts (past 365 days, precise, subject-only)**: `perf` word ≈ 18; `optimize` ≈ 19; `faster/latency/frame time` ≈ 10. (Earlier raw full-message grep hit ~2,014 — misleading; `git log --grep` matches bodies. Subject-only counts are the honest number: **~40–50 perf-subject commits/yr**, i.e., steady weekly-ish drumbeat rather than a campaign.)
- Recent examples of the drumbeat: `d7f2889916` 2026-09-09 "language: Cache buffer syntax highlighting per row chunk (#63145)"; `318c664ec3` 2026-09-09 markdown cache; `24254879b7` 2026-08-31 editorconfig re-resolution; `3f00b5d75d` 2026-08-31 row-highlight viewport limit; `cb1352a29d` 2026-08-20 configurable inline-completion debounce (#61568).

---

## 4. Zeta → edit_prediction Lineage

- **Origin**: `77b8296fbbf2f68d2ae9bdcb04d222de3f69cd59` — 2024-12-09 — "Introduce staff-only inline completion provider (#21739)" — first `crates/zeta` commit. Staff-gated from day one; token refresh `96499b7b25` and word-wise diff `4300ef840b` follow within 24h.
- **Data-collection eligibility work (2025)**: license-file detection of permissiveness — `4e97968bcb` 2025-08-08 (Apache 2.0 added, #35900), BSD `515282d719` 2025-08-29 (#37194), zlib `ad746f25f2` 2025-08-30 (#37238); `0e33a3afe0` 2025-09-07 "zeta: Check whether data collection is allowed for recent edit history (#37680)". So: opt-in/out is license-scoped at the repo level.
- **Crate reorg**: `42583c1141b68f655335769f4770b3ceea84c263` — 2025-12-04 — "Reorganize edit prediction code and remove old experiments (#44187)" — `zeta` the brand becomes `edit_prediction` the architecture (LSP-based context retrieval added next day, `76167109db` 2025-12-04 #44036).
- **Consent surface (2026)**: `9743fe7dfd` 2026-02-17 "Put back logic for passing data collection choice to cloud (#49426)" (note: it had been *dropped* — a silent regression caught and restored); `67c2c34b94` 2026-03-04 repo_url included when collection enabled (#50745); org-level kill-switch `eb76db4050` 2026-04-10 (#53639); user-facing `allow_data_collection` setting `a710669e0385c770c23a0ffc92c132bb96b38fa5` 2026-04-23 (#51389).
- Model plumbing breadth (2026): self-hosted "Sweep" next-edit models `790dcefb01` 2026-07-30 (#51139); Ollama support `d1446d66e0` 2026-09-01 (#63519); bespoke LSP request path `4a3e0af532` 2026-07-14 (#60947); v4 cursor marker fixes `f851d82e88`/`4f047acc17` 2026-07-30/31.

---

## 5. Release Cadence + Governance

- **Lineage confirmed by tags**: last v0 tag `v0.234.6-pre` 2026-04-24 (v0.233.x patches through ~2026-04); **`v1.0.0` → 2026-04-29** (`5ec84a926e`); `v1.19.2` → 2026-09-09 (= latest release, published 2026-09-09T17:41Z; `v1.20.0-pre` cut same minute). 44 `v1.*` non-pre tags in ~4.5 months.
- **Weekly discipline**: v1.9.0 Jul 1 → v1.10.0 Jul 8 → v1.11.3 Jul 15 → v1.12.0 Jul 22 → v1.13.1 Jul 29 → v1.14.2 Aug 5 → v1.15.0 Aug 12 → v1.16.1 Aug 19 → v1.17.2 Aug 26 → v1.18.0 Sep 2 → v1.19.x Sep 4–9. Lockstep `-pre` tags + same-day pre/stable publishes (e.g., 2026-09-09: both v1.19.2 and v1.20.0-pre at 17:41). Patch spikes (v1.10.1–3 within 4 days) show hotfix pressure without breaking the weekly train.
- **Guild governance is real infrastructure**, not marketing: `.github/workflows/guild_assignment_status.yml`, `guild_new_pr_notify.yml`, `guild_stale_assignments.yml`, `guild_weekly_shipped.yml`, plus PR-labeler guild checks — history: POC `53498357e8` 2026-04-29 (#55205, self-described "broken POC"), labeler integration `8909ede328` 2026-03-19 (#51914), board automation `b3d5ead59f` 2026-07-02 (#60266) → role-check switch `7dc634124c` 2026-07-08 (#60606) → fix `a0bb97cad5` 2026-07-16 (#61115) → extra role `f99aad788b` 2026-08-10 (#62411).
- **Community-contributor reality check**: the subagent-depth PRs (#54084/#54142, GitGlimpse895, Apr 2026) were closed unmerged; the same ideas landed via staff rework (#57987, eholk, Jun 2026). Guild appears designed to give external contributors a managed lane — **UNVERIFIED**: whether "Zed Guild" credit appears in release notes; the string does not occur in tracked `.md` in-repo (it lives in GitHub release notes, outside git).

---

## 6. Five Juiciest Findings (SHA + date)

1. **`dc83f1ad38cd5b3cd3c3a5d165937bed8201e9ce` — 2025-04-01** — "Rename `assistant2` to `agent` (#27887)". The entire agent era traces to a single April Fools'-day rename of an 18-month-old experiment crate (`1de02cf6e5`, 2023-12-06). Product strategy visible as a `git mv`.
2. **`495ec7a109c4e87511f16b52d943f95390ca246f` — 2025-07-09** — "ACP (#34030)". The Agent Client Protocol was prototyped ~6 weeks pre-launch with Claude Code E2E tests a week later (`cfe1adc792`, 2025-07-18) — protocol-first agent strategy built in the open, backwards from a summer hack sprint.
3. **`59f32d2135a9c9816d46e4e98cc55e7103423aa4` — 2026-06-01** — create_thread tools (#57987) landed 6 weeks after community PRs #54084/#54142/#54213 were closed unmerged — the clearest pattern of "community proposes, staff re-lands" in the agent surface (with governance automation, §5, built to scale exactly this lane).
4. **`42583c1141b68f655335769f4770b3ceea84c263` — 2025-12-04** — "Reorganize edit prediction code and remove old experiments (#44187)" + same-day LSP context retrieval (#44036). Zeta→edit_prediction pivot, followed by the consent-paperwork trail (#49426, #50745, #53639, #51389) — training-data governance evolved commit-by-commit in public.
5. **`07a0bd1220bddb940028d610e40319a459dd49fb` — 2026-08-17** — "gpui: Frame time debug overlay (#62749)" landing simultaneously with the Aug 2026 bench-infra burst (#63035/#63039/#63244) and collab LiveKit revert `cf76418cf4` — performance tooling and honest rollback discipline are both first-class, ongoing 30 months after `de64de22a3` (2024-01-22) introduced frame-time measurement.

---

*Verification status: every commit hash above was resolved in a local blobless clone (checked out only metadata; content greps limited to `git grep` on tracked trees where noted). Items marked UNVERIFIED rely on external sources or could not be confirmed from git alone.*
