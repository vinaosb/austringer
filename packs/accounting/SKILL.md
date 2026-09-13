---
name: accounting
description: "Bookkeeping and close-process workflow guidance: chart of accounts hygiene, journal entry discipline, trial balance reconciliation, accruals, depreciation schedules, and GAAP/IFRS close checklists — process guidance, not tax or audit opinions."
license: MIT OR Apache-2.0
version: "1.0.0"
languages: ["English", "Portuguese", "Spanish", "French", "German"]
allowed-tools: ["view_window", "edit_lines", "tool_search"]
triggers:
  - trial balance
  - journal entry
  - journal entries
  - general ledger
  - chart of accounts
  - accounts receivable
  - accounts payable
  - bank reconciliation
  - accrual
  - depreciation schedule
  - gaap
  - ifrs
  - closing entries
  - bookkeeping
  - buchungssätze
  - kontenplan
  - balancete
  - asientos contables
do-not-trigger-on:
  - react component
  - docker compose
  - codebase
  - electrical panel
  - derivative pricing
  - model deployment
  - pull request
  - lease agreement
---

# Accounting pack

{{shared:pack-conventions}}

> **Scope note**: workflow and checklist guidance for bookkeeping and
> the close process. Jurisdiction-specific tax treatments and formal
> audit opinions are out of scope; flag them for a qualified
> accountant/auditor.

## Workflow

1. **Ledger hygiene first**: before any entry, confirm the chart of
   accounts mapping (one account, one meaning; no undocumented suspense
   accounts).
2. **Journal entry discipline**: every entry states date, debit(s),
   credit(s), a reference (invoice/contract), and a one-line rationale.
   Debits = credits before posting; unbalanced entries never post.
3. **Bank reconciliation**: match statement ↔ ledger line by line;
   investigate unmatched items by AGE (outstanding > 30 days gets a
   written explanation); reconciling items are temporary by definition.
4. **Trial balance**: run after posting batches; a small residual means
   posting errors (transposition: divisible by 9; omission: hunt the
   reference), not "plug it".
5. **Accruals & prepayments**: cut-off testing around period end —
   goods/services received vs invoice received; record accruals with
   reversal flags for the next period.
6. **Depreciation**: schedule per asset with method, useful life, and
   residual value stated at acquisition; monthly run is mechanical from
   the schedule, never re-derived ad hoc.
7. **Close checklist**: standardized, ordered, with owners and evidence
   links (reconciliations done → accruals posted → depreciation run →
   TB balanced → variance explanations vs prior period → close).
8. **GAAP/IFRS note**: state which framework the entity reports under
   and do not mix conventions within one set of books.

## Worked example

Request: *"The trial balance is off by 540 — where do I look?"`

```text
Divisibility probe: 540 = 9 × 60 → suspect a TRANSPOSITION.
Scan ledger for entry pairs whose digits could transpose to 540:
  Found:  rent expense posted 610 vs invoice 70  (610-70=540). ✓
Correction (with reference):
  JE-2026-09-114  2026-09-12
    Dr Rent expense        610.00   (ref: INV-8812)
    Cr Accounts payable    610.00   rationale: restate misposted 70
Re-run TB → balanced. Evidence: TB before/after + invoice copy.
```

## Eval scenario

Prompt: `"Reconcile the trial balance and prepare journal entries"`.
Expected routing: `accounting` (triggers: `trial balance`,
`journal entries`). First actions: confirm posting completeness, run
the divisibility/omission probes, prepare referenced entries, re-run
the TB, and file the before/after evidence.
