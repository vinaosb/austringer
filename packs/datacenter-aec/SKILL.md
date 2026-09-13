---
name: datacenter-aec
description: "Exemplar port (fixture-quality subset) of the datacenter/AEC engineering skills: electrical distribution, cable sizing, structural and thermal loads, UPS/rack power, fiber routes, and BIM authoring. A full port would carry the domain calculators and standards libraries."
license: MIT OR Apache-2.0
version: "0.1.0"
languages: ["English", "Portuguese", "Spanish"]
allowed-tools: ["view_window", "edit_lines", "execute_command", "tool_search"]
triggers:
  - electrical panel
  - cable sizing
  - load calculation
  - single-line diagram
  - panel schedule
  - structural load
  - beam design
  - concrete cover
  - rebar
  - hvac load
  - cooling load
  - ups sizing
  - rack power
  - pue
  - fiber route
  - optical loss budget
  - itu-t
  - bim model
  - ifc
  - revit
  - quadro elétrico
  - dimensionamento de cabos
  - cálculo de cargas
  - 配电柜
do-not-trigger-on:
  - code audit
  - pull request
  - react component
  - docker compose
  - lease agreement
  - journal entries
  - trial balance
  - financial model
  - model deployment
  - integral
---

# Datacenter / AEC exemplar pack

{{shared:pack-conventions}}

## Exemplar status

Fixture-quality port of the maintainer's datacenter/AEC skills
(electrical-engineering, civil-engineering, structural-engineering,
thermal-engineering, engineering-compliance, bim-authoring,
itu-fiber-validator). The subset here keeps the workflow skeletons and
cross-domain negative scoping; a full port would add the calculation
scripts, standards reference tables (IEC/ITU-T/ACI), and the ITU fiber
validator. Source of record: the bare-markdown engineering skills under
`harness-skills/tests/fixtures/corpus/opencode/`.

## Workflow

1. **Classify the discipline first**: electrical / structural / thermal
   / fiber / BIM. Each has its own inputs, codes, and units; never mix
   calculation bases.
2. **Electrical**: build the load inventory (panel schedule) before any
   sizing — connected load vs demand factor vs diversity; cable sizing
   follows ampacity, voltage drop, and fault-rating constraints
   SIMULTANEOUSLY (the binding constraint is the design driver);
   single-line diagram stays in sync with the schedule.
3. **Datacenter power chain**: IT rack power → PDU → UPS (sized with
   autonomy time and battery derating) → generator; redundancy topology
   (N, N+1, 2N) declared per tier; PUE estimated with its measurement
   boundary stated.
4. **Structural**: loads (dead, live, equipment, seismic) combined per
   code before member design; beam design and rebar/concrete cover per
   the governing code edition; deflection and vibration checked for
   sensitive equipment.
5. **Thermal**: cooling load from envelope + IT heat + distribution
   losses; airflow management (hot/cold aisle) before tonnage; sensible
   vs latent split respected.
6. **Fiber**: route → loss budget per ITU-T math (splices, connectors,
   attenuation) → margin; the budget, not the route length, is the
   verdict.
7. **BIM**: element classification and level-of-development declared
   per deliverable; IFC exchanges validated against the schema before
   sharing.
8. **Compliance note**: engineering deliverables carry the governing
   standard edition and require sign-off by a licensed engineer — this
   pack prepares the workflow, it does not stamp drawings.

## Worked example

Request: *"Size the feeders for a 42-circuit panel serving 12 racks."`

```text
Load inventory (panel schedule):
  12 racks × 5 kW IT (nameplate 8 kW, diversity 0.63) = 60 kW
  cooling fans + lighting + aux          = 6 kW
  Connected: 102 kW (nameplate) | Demand: 66 kW @ 415V 3φ
Feeder current: I = 66,000 / (√3 × 415 × 0.95 pf) ≈ 96.6 A
Constraints (all must hold):
  ampacity (termination, 75°C):  ≥ 96.6 A
  voltage drop (3% to panel):    conductor size from run length
  fault rating:                  matches upstream protection
Driver: voltage drop on the 85 m run → 35 mm² Al (illustrative);
recheck ampacity + fault at that size → passes ⇒ record driver.
Units and demand factors traceable in the schedule; PE sign-off flagged.
```

## Eval scenario

Prompt: `"audit this electrical panel"`. Expected routing:
`datacenter-aec` (trigger: `electrical panel`) while `audit-exemplar`
stays out (negative scope: `electrical panel`). First actions: build
the panel schedule from nameplate data, apply demand factors, then run
ampacity/voltage-drop/fault checks with the binding constraint recorded.
