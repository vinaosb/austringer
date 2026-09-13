---
name: finance-investors
description: "Founder and investor financial workflow guidance: financial model structure, unit economics, cap tables, term sheet review process, burn/runway tracking, and due-diligence data room preparation — process guidance, not investment advice."
license: MIT OR Apache-2.0
version: "1.0.0"
languages: ["English", "Portuguese", "Spanish", "French", "German"]
allowed-tools: ["view_window", "edit_lines", "tool_search"]
triggers:
  - financial model
  - revenue projection
  - investor pitch
  - pitch deck
  - cap table
  - term sheet
  - due diligence
  - burn rate
  - runway
  - series a
  - series b
  - valuation
  - ebitda
  - unit economics
  - cash flow forecast
  - sensitivity analysis
  - levée de fonds
  - ronda de inversión
  - avaliação de empresa
do-not-trigger-on:
  - react component
  - docker compose
  - codebase
  - electrical panel
  - differential equation
  - journal entries
  - trial balance
  - pull request
---

# Finance-investors pack

{{shared:pack-conventions}}

> **Scope note**: workflow and structure guidance for building and
> reviewing founder/investor financial material. It is not investment
> advice, valuation opinion, or a recommendation.

## Workflow

1. **Model structure before numbers**: assumptions sheet (drivers any
   reviewer can dispute) → engine (calculations, no hard-coded values)
   → statements (P&L, cash flow, balance) → outputs (KPIs, scenarios).
   Every output cell traces to assumptions.
2. **Unit economics**: CAC, LTV (with cohort survival, not revenue
   multiples), payback period, contribution margin. State churn and
   margin definitions explicitly — inconsistent definitions are the
   most common review failure.
3. **Burn & runway**: net burn from the cash flow statement (not the
   P&L); runway = cash ÷ net burn with the raise-arrival assumption
   stated; track the J-curve against plan monthly.
4. **Scenarios & sensitivity**: base / downside / upside with the ONE
   driver that kills the business varied (churn, CAC, or ramp), not a
   uniform ±10%.
5. **Cap table**: current fully-diluted view (all instruments,
   options pool, convertibles/SAFEs with their conversion mechanics);
   model the round's post-money dilution before and after the pool
   top-up.
6. **Term sheet review**: process, not verdict — flag economics
   (price, pool, liquidation preference >1x, participation), control
   (board, vetoes, information rights), and process terms (exclusivity,
   expiry). Each flag lists the standard range and the question for
   counsel.
7. **Data room**: index by workstream (corporate, financial, IP,
   employment, contracts); every document dated and versioned; gaps
   listed rather than hidden.

## Worked example

Request: *"Sanity-check our runway before the raise."`

```text
Cash (2026-09-01):                1.90M
Net burn (last 3 mo avg, CF stmt): 165K/mo
Runway: 1.90M / 165K ≈ 11.5 months
Assumption stated: no revenue growth beyond signed contracts.
Downside (churn +2pp): burn 190K → runway 10.0 mo
Flag: Series-A close modeled 2027-03 leaves <3 mo buffer under
downside → either cut burn 15% or move raise start to 2026-11.
Traceability: every figure links to the CF statement rows.
```

## Eval scenario

Prompt: `"Build a financial model with revenue projections for
investors"`. Expected routing: `finance-investors` (triggers:
`financial model`, `revenue projection`). First actions: fix the
assumptions/engine/statements/output layering, define churn and margin
conventions, build base+downside scenarios, and wire every output to
traceable drivers.
