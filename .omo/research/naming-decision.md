# Naming decision: austringer

**Date:** 2026-10-25
**Status:** decided and applied (see the rename commit in git history)
**Scope:** public name, crate prefix, binary names, env-var prefix, repo name, docs domain

## Decision

The project is **austringer**. Crates are `austringer-*`, binaries are
`austringer` (CLI) and `austringer-daemon`, environment variables are
`AUSTRINGER_*`, the repository is `vinaosb/austringer`, and the manual is
served from GitHub Pages (`https://vinaosb.github.io/austringer/`).

An *austringer* is the falconer who flies short-winged hawks (goshawks) — the
handler whose craft is letting a powerful, autonomous bird fly while staying
attached to it. The metaphor maps one-to-one onto the architecture: the daemon
is the attachment (permissions, budgets, sessions, ask-first fallback
mediation); the model is the bird. It extends the falconry vocabulary the
candidate search kept returning to (jess, creance, eyas are all falconry
terms), while being a real English noun that reads as a brand.

## Method

Same discipline as the code: nothing asserted without a check.

1. **Candidate generation** — 160+ names across 14 metaphor families
   (draft-team harness gear, locomotive servicing, rope-and-attach, water-flow
   regulation, clockwork escapements, musical accompaniment, household staff,
   editorial marks, weaving, records/memory, Roman stewardship, lean
   manufacturing, hub-and-spoke, workshop tooling). Every family was chosen
   because it maps onto a load-bearing concept of this repo (fan-out
   delegation, ask-first governance, cost caps, attach/REPL clients,
   event-sourced sessions).
2. **Registry availability** — calibrated RDAP queries (direct to Verisign /
   Google Registry / Identity Digital; a bootstrapper rate-limit and a
   resolver-interception false-positive were both caught and worked around),
   plus DNS-over-HTTPS cross-checks. All 94 first-wave names across
   `.com/.dev/.io/.sh`.
3. **Namespace collisions** — crates.io API (crate name free/taken) and
   GitHub org availability for every finalist.
4. **Live product collision search** — web search per finalist. This step
   killed two of the four "perfect availability" names and is the reason the
   search was worth doing.

## Kills (with evidence)

| Candidate | Why rejected |
| --- | --- |
| `creance` | Active agent platform renaming itself TO creance (github.com/tilsley/creance — inference gateway with a budget ledger; same concept space) |
| `evener` | Existing coding agent (github.com/prime-radiant-inc/evener — literally our product category) |
| `knarr` | LLM proxy (github.com/Getty/langertha-knarr) + GMU's well-known `knarr` JVM tracing tool |
| `fusee` | Switch homebrew scene (`Fusée Gelée`) + FUSEEProjectTeam C# 3D engine |
| `remontoir` | Regolith Linux `remontoire` keybinding tool (actively maintained, in nixpkgs) |
| `hatamoto` | Existing "Hatamoto" plugin-registry platform in the AI tooling space |
| `expediter` | cookpad's Ruby `expeditor` fault-tolerance library |
| `swingletree` | swingletree-oss CI quality-gate integrator |
| `reeve`, `jess`, `eyas`, `domovoi` | Best stories, but every good domain taken and/or crate name taken |

## Why austringer won

| Check | Result (2026-10-25) |
| --- | --- |
| Product collision search | none found |
| crates.io `austringer` | unregistered |
| GitHub org `austringer` | unregistered |
| `austringer.dev` $14.99 / `austringer.io` $53.99 / `austringer.org` $9.99 / `austringer.sh` $59.99 / `austringer.app` $14.99 (name.com live API, first-year registration, non-premium) | all available |
| Runner-up `becket` (clean, `.io`+`.sh` free) | chosen as fallback if austringer hits a late collision |

## Deliberate deferrals

- **Custom domain**: not purchased. GitHub Pages + crates.io are the canonical
  links for now; the TLDs above were verified available at decision time and
  can be registered when there is traffic to serve. Availability is
  perishable (see the `creance`/`evener` kills) — if the domain starts to
  matter, register first and reason later.
- **crates.io publish**: first publish of the `austringer-*` set is queued for
  the v0.1 series; publishing claims the names, and claiming before someone
  else's naming search lands on the same word is the point.

## Rename mechanics (for the record)

Rules applied repo-wide, in order: `harness-` → `austringer-` (except the
external project citation `harness-rs` and the category-named study file
`harness-research.md`), `harness_` → `austringer_`, `HARNESS_` →
`AUSTRINGER_`, capitalized wire tokens (`Harness-Session-Id` →
`Austringer-Session-Id`), code identifiers (`HarnessMcpServer`,
`ProviderKind::Harness`, `Vendor::Harness` → `.austringer` skills dir),
standalone product references (CLI invocations, table headers, titles), and
path moves (24 crate dirs + the VS Code extension + bin files + the runtime
dirs `.harness/` → `.austringer/` and `.harness-daemon/` →
`.austringer-daemon/`). Generic category prose ("what a harness is",
"another harness") was deliberately kept — the word names the industry, and
`harness-rs` is somebody else's proper noun.
