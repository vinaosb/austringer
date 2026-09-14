---
name: Feature request
about: Propose a capability or a change in behavior
title: "[feat] "
labels: enhancement
assignees: ""
---

**The capability**

What you want to be possible that is not, or what should behave
differently.

**Why**

The concrete workflow or failure this unblocks. If another harness does
this today and it worked for you, name it — the project's receipts
discipline prefers precedents over speculation.

**Receipt / evidence**

This project lands mechanisms with evidence, not intuition (see
CONTRIBUTING.md, evidence culture). Any of these strengthens a proposal:

- A research-file section (`.omo/research/harness-research.md`, cited as
  "research §N") that motivates it, or a named harness where it succeeds
  or failed.
- A measurement (perf-gate report, bench numbers, eval artifact).
- A sketch of the acceptance test that would prove it.

**Guardrail check**

Does this touch a documented guardrail (no silent model substitution, no
embeddings for code search, no whole-file rewrites, no telemetry
default-on, no auto-run unverified plugins, Windows-first)? If yes, say
which one and how your proposal respects or argues to change it.
