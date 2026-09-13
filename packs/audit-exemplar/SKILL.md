---
name: audit-exemplar
description: "Exemplar port (fixture-quality subset) of the maintainer's audit-squad skills: security, dependency, and test-coverage audits over a codebase with strict findings-schema discipline. A full port would carry all ~25 squads and the v3.2 trending fields."
license: MIT OR Apache-2.0
version: "0.1.0"
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
allowed-tools: ["view_window", "execute_command", "tool_search", "task_completed"]
triggers:
  - code audit
  - security audit
  - dependency audit
  - audit the codebase
  - audit this repository
  - audit this codebase
  - vulnerability scan
  - cve
  - sbom
  - license compliance
  - test coverage audit
  - audit findings
  - 审计代码
  - auditar dependências
do-not-trigger-on:
  - electrical panel
  - sprint process
  - lease agreement
  - contract review
  - trial balance
  - journal entries
  - cable sizing
  - structural load
  - financial audit
  - tax audit
context_fork: true
model-guidance:
  glm-5: Cite file:line evidence for every finding; self-assess false-positive risk before emitting; never collapse findings to counts.
---

# Audit exemplar pack

{{shared:pack-conventions}}

## Exemplar status

This is a **fixture-quality port** of the maintainer's audit-squad
library (25+ audit skills sharing one findings schema and `_shared`
template). The representative subset here covers the three load-bearing
squad patterns: security, dependencies, test coverage. A full port
would add the remaining squads (architecture, fault-tolerance,
observability, regulatory-*, …), the v3.2 trending fields, delta
tracking, and the consolidated-report orchestrator. Source of record:
`harness-skills/tests/fixtures/corpus/opencode/audit-*` +
`_shared/audit-template.md`.

This pack audits CODEBASES and their dependency trees. It is
deliberately negative-scoped against engineering audits (electrical,
structural), financial audits, tax audits, and legal document review —
those route to their own domains.

## Workflow

1. **Scope the target**: repository root, languages, and the audit
   class (security / dependencies / coverage). Record `files_examined`
   — negative space is itself a finding.
2. **Discover before judging**: enumerate the surface (entry points,
   dependency manifests, test tree) before evaluating any single file.
3. **Findings discipline**: every finding carries the full schema
   below — root cause, impact, evidence with file:line, a concrete fix,
   and a verification command. Findings are never collapsed to counts.
4. **Self-assess**: each finding carries a confidence and a
   false-positive risk note; ≥3 candidate false-negatives are
   enumerated explicitly.
5. **Context fork**: audits run as a forked subagent over a read-only
   view (`context_fork: true`); the parent receives the findings
   document only.

{{shared:audit-findings-schema}}

## Worked example

Request: *"audit the codebase for security issues before release"*

```json
{
  "id": "SEC-0007",
  "severity": "high",
  "title": "Token verification skipped in dev builds",
  "root_cause": "#[cfg(debug_assertions)] bypass around the JWT check",
  "impact": "anyone building locally can call admin endpoints",
  "evidence": "src/auth/middleware.rs:88 — 'if cfg!(debug_assertions) { return Ok(()) }'",
  "fix": "gate the bypass behind an explicit feature flag, fail closed in release",
  "verification_command": "cargo build --release && ./target/release/app --self-test-auth",
  "confidence": 0.95,
  "files_examined": ["src/auth/*", "src/api/*", "Cargo.toml"],
  "fp_risk_self_assessment": "low — bypass is unconditional in debug"
}
```

## Eval scenario

Prompt: `"audit the codebase for security issues before release"`.
Expected routing: `audit-exemplar` (trigger: `audit the codebase`).
First actions: enumerate entry points and auth surfaces, then findings
in the schema above with evidence, verification commands, and an FP/FN
self-assessment. Cross-domain: `"audit this electrical panel"` routes
to `datacenter-aec`; `"audit our sprint process"` matches nothing.
