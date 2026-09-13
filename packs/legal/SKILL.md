---
name: legal
description: "Contract and legal-document WORKFLOW guidance: intake checklists, clause inventories, redline process, obligation tracking, and escalation to licensed counsel. Process checklists only — not legal advice, not jurisdiction-specific opinions."
license: MIT OR Apache-2.0
version: "1.0.0"
languages: ["English", "Portuguese", "Spanish", "French", "German"]
allowed-tools: ["view_window", "edit_lines", "tool_search"]
triggers:
  - lease agreement
  - contract review
  - nda
  - memorandum of understanding
  - master service agreement
  - statement of work
  - force majeure
  - indemnification clause
  - limitation of liability
  - data processing agreement
  - terms of service
  - privacy policy
  - legal review
  - vendor contract
  - contrato de arrendamento
  - revisión de contratos
  - clause abusive
  - vertragsprüfung
do-not-trigger-on:
  - pull request
  - electrical panel
  - react component
  - docker compose
  - cable sizing
  - model deployment
  - codebase
model-guidance:
  glm-5: Never assert jurisdiction-specific outcomes; present checklists and flag every question that requires a licensed professional.
---

# Legal pack

{{shared:pack-conventions}}

> **Scope disclaimer**: this pack provides workflow, process, and
> checklist scaffolding for working WITH legal documents. It is not
> legal advice and does not produce jurisdiction-specific opinions.
> Binding interpretation always escalates to licensed counsel.

## Workflow

1. **Intake**: identify instrument type (NDA, MSA, SOW, lease, DPA,
   ToS), parties and their roles, governing law CHOSEN IN THE DOCUMENT,
   term, and renewal/termination mechanics. Record these as a summary
   table before reading clauses.
2. **Clause inventory**: walk the document against a standard frame —
   definitions, obligations of each party, payment, IP, confidentiality,
   data protection, indemnification, limitation of liability, force
   majeure, termination, dispute resolution, governing law. Note ABSENT
   clauses, not just present ones.
3. **Flag, don't decide**: mark clauses as `standard / unusual / risky
   for <party>` with a plain-language reason quoting the clause text.
   Anything that changes money, exclusivity, liability caps,
   indemnities, or termination rights is a flag.
4. **Redline process**: one issue per tracked change; every redline
   carries a comment stating the concern and the fallback position;
   version the drafts; never edit a signed instrument.
5. **Obligation register**: extract post-signature obligations into a
   tracked list (who, what, deadline, evidence) — deadlines for
   notices, renewals, audits, and data-protection duties.
6. **Escalation**: any question of enforceability, jurisdictional
   effect, tax treatment, or regulatory exposure goes to counsel with
   the flag list and the relevant excerpts prepared.

## Worked example

Request: *"Review this 2-year vendor MSA before we sign."*

Output shape (workflow artifact, not advice):

```text
SUMMARY: MSA, Acme<->Us, 2y auto-renew (90d notice), GovLaw: as written
FLAGS:
  [unusual] §7.2  Liability cap = 6 months fees — quote: "…"
            (comment: below our fallback; redline to 12m or carve-outs
            for confidentiality breaches → counsel to confirm approach)
  [risky-for-us] §11  IP assignment of "all deliverables including
            pre-existing materials" — quote: "…"
            (comment: request carve-out for background IP)
  [absent] No data-protection annex; DPA needed if PII in scope.
OBLIGATIONS: notice of renewal 90d (2028-06-30); annual insurance
            certificate each January.
ESCALATE: liability cap position, IP carve-out wording → counsel.
```

## Eval scenario

Prompt: `"legal review of this lease"`. Expected routing: `legal`
(trigger: `legal review`). First actions: intake summary (parties,
term, rent mechanics, notice deadlines), clause inventory walk with
flags, obligation register — with jurisdictional questions listed for
counsel rather than answered.
