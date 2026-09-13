---
name: ai-deployment
description: "Exemplar port (fixture-quality subset) of the microsoft-foundry AI deployment skills: model deployment capacity discovery, SKU choice, fine-tuning jobs, and evaluation workflows. A full port would carry the script library and per-region reference tables."
license: MIT OR Apache-2.0
version: "0.1.0"
languages: ["Python", "Bicep", "YAML", "Shell"]
allowed-tools: ["view_window", "edit_lines", "execute_command", "tool_search"]
triggers:
  - deploy a model
  - model deployment
  - deployment sku
  - provisioned throughput
  - ptu
  - tokens per minute
  - tpm quota
  - capacity planning for ai
  - fine-tune a model
  - fine-tuning job
  - sft
  - dpo
  - evals sweep
  - foundry project
  - ai agent deployment
  - implantar modelo
  - déployer un modèle
  - ajuste fino
do-not-trigger-on:
  - docker compose
  - nginx
  - kubernetes manifest
  - lease agreement
  - electrical panel
  - trial balance
  - journal entries
  - react component
---

# AI-deployment exemplar pack

{{shared:pack-conventions}}

## Exemplar status

Fixture-quality port of the maintainer's microsoft-foundry skills
(deploy-model + preset/customize/capacity, finetuning, agent
create/invoke/observe). The subset here keeps the deployment decision
workflow and capacity-discipline; a full port would add the Python
scripts (capacity queries, dataset validation, training-curve checks)
and the azd workflows. Source of record:
`harness-skills/tests/fixtures/corpus/agents/microsoft-foundry`.

This pack is about deploying MODELS onto managed AI platforms (SKU,
quota, throughput, fine-tuning, evals). Plain container/nginx/Kubernetes
deployments route to `devops`.

## Workflow

1. **Fix the workload before the region**: expected requests/s, tokens
   per request (prompt + completion), latency SLO, and peak-to-mean
   ratio. TPM needed = peak RPS × prompt+completion tokens × headroom.
2. **Capacity discovery before region choice**: query quota + available
   capacity per region per SKU (GlobalStandard vs Standard vs PTU);
   rank by fit, not by habit — a model you cannot get quota for is not
   an option.
3. **SKU decision**: serverless/pay-as-you-go for spiky or
   exploratory loads; provisioned (PTU) when steady demand makes the
   guaranteed throughput cheaper; document the crossover math.
4. **Deploy**: pin the model VERSION (never "latest" in production),
   name the deployment after its purpose, tag with workload owner;
   record capacity settings in code (Bicep/Terraform), not click-ops.
5. **Fine-tuning (when justified)**: start from prompt engineering
   evidence (documented failures a base model cannot fix); SFT for
   format/style competence, DPO/preference tuning for behavioral
   preferences, RFT when graders exist; validate the dataset BEFORE
   submitting (schema, dedup, class balance) — rejected jobs cost days.
6. **Evaluate before and after**: golden-set evals gate promotion;
   compare against the base model with identical settings; a fine-tune
   without an eval table is not done.
7. **Operate**: monitor token usage vs quota, latency percentiles, and
   content-filter rejections; plan the quota raise before the cliff.

## Worked example

Request: *"What TPM quota do I need for gpt-5.5 in west-europe?"`

```text
Workload (from the asking team):
  peak 12 RPS, ~2.5K prompt + ~400 completion tokens, headroom ×1.3
TPM needed = 12 × 60 × (2500 + 400) × 1.3 ≈ 2.28M TPM
Region/SKU probe (capacity discovery step):
  west-europe  Standard     quota 1M   → insufficient alone
  west-europe  GlobalStandard quota 4M → fits, global latency routing
  swedencentral Standard    quota 3M   → fits, EU data residency
Decision: GlobalStandard 4M (spiky load, no PTU crossover yet);
PTU crossover math recorded for the next capacity review.
Version pinned (not "latest"); deployment tagged owner=ask-team.
```

## Eval scenario

Prompt: `"What TPM quota do I need for gpt-5.5 in west-europe?"`.
Expected routing: `ai-deployment` (trigger: `tpm quota`). First
actions: extract the workload numbers (or ask for them), compute TPM
with headroom, run capacity discovery across regions/SKUs, and record
the SKU decision with its crossover math. Cross-domain: `"deploy an ai
model with docker compose"` routes to `devops` — this pack is
negative-scoped on `docker compose`.
