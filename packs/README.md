# Shipped domain packs (registry content)

This directory is the austringer **pack registry content source** (plan todo 33).
Packs are DATA loaded by the T30 skills loader at runtime — they are never
compiled into any binary (plan guardrail: "Must NOT hardcode packs into
binary"). The todo-32 registry serves and installs from here.

## Layout

```
packs/
  manifest.json              # packs + versions + categories (sync-checked by tests)
  README.md                  # this file
  _shared/                   # {{shared:name}} partials interpolated at invoke time
    pack-conventions.md      # included by every pack body
    audit-findings-schema.md # findings schema used by audit-exemplar
  <pack-name>/SKILL.md       # one pack per directory
```

Programming packs: `dotnet` (seeded from the maintainer's real skill library:
dotnet-clean-arch, aspnetcore-health, postgresql-efcore, distributed-caching,
http-error-handling, signalr-realtime, blazor-mudblazor, quartz-scheduling,
keycloak-oidc, microsoft-foundry), `frontend`, `backend`, `devops`.

Domain packs (authored from scratch, plan D6): `legal`, `maths`,
`accounting`, `finance-investors`.

Exemplar ports (fixture-quality representative subsets, NOT full ports —
each documents its source skills and what a full port would add):
`audit-exemplar` (audit-squad), `datacenter-aec` (datacenter/AEC
engineering), `ai-deployment` (microsoft-foundry deployment skills).

## Install path (until the todo-32 registry lands)

Copy or symlink a pack into a project and the loader picks it up:

```
<project>/.austringer/skills/<pack-name>/SKILL.md
<project>/.austringer/skills/_shared/*.md
```

Vendor precedence is `.austringer` > `.opencode` > `.claude` > `.agents` >
`.cursor`; installing a pack under `.austringer/skills` outranks all others.
The todo-32 registry automates this with pin + content-hash verification.

## Authoring rules (enforced by `austringer-skills/tests/packs_acceptance.rs`)

Every pack MUST carry, in its SKILL.md frontmatter:

- `name` (lowercase kebab, = directory name), `description` (<1024 chars),
  `version` (matches `manifest.json`);
- `triggers:` — explicit phrases, including non-English ones where natural
  for the domain (routing is case-insensitive containment; single ASCII
  words respect word boundaries);
- `do-not-trigger-on:` — negative scope for cross-domain isolation; a hit
  blocks the pack even when a positive trigger matched;
- `languages:` and `allowed-tools:` hints;
- `model-guidance:` per-model sections where meaningful.

Every pack body MUST contain `## Worked example` (one representative
interaction or output) and `## Eval scenario` (the routing-regression
prompt), and includes the `{{shared:pack-conventions}}` partial.
