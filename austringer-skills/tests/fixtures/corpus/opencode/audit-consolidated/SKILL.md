---
name: audit-consolidated
description: >-
  Consolidated audit report aggregation: standard output format for all squads,
  deduplication, severity resolution, and configurable report generation.
  Language-agnostic output layer that consumes findings from all other audit squads.
  v3.0 adds: critical-thinking validation layer (FP/FN detection), hard rule against
  collapsing non-critical findings to counts, full-detail carry-forward mandate.
  v3.1 adds: delta tracking across runs (fingerprint + status), fix dependency DAG,
  coverage heat map (files_examined), output emitters (JSON + Markdown + SARIF +
  GitHub comment), baseline suppression, and CI gating (--fail-on).
  v3.2 adds: resilience layer — squad-level disk persistence + checkpoint protocol +
  `audit resume` subcommand + chunked atomic writes. A failed orchestrator no longer
  loses squad work; recovery is a 5-10 minute resume, not a 45 minute restart.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "3.2.0"
---

# Consolidated Audit Report Squad (v3.2)

Aggregation + critical-thinking validator + trending engine + resilience layer. Consume findings from all squads, **independently verify sample vs actual codebase**, deduplicate, resolve severity conflicts, surface FPs + FNs, **compute fix DAG**, **render coverage heat map**, **diff vs previous runs**, **emit JSON/Markdown/SARIF/GitHub-comment**, **survive orchestrator death via disk persistence + checkpoint + resume**.

**v3.0**: HARD RULE — ALL findings (every severity) full detail. Count-only collapse FORBIDDEN (was v2.0 failure mode). §5 validation layer. §6 FP detection. §7 FN detection. Schema v3.0 (10 new required fields).

**v3.1**: §13 delta tracking (fingerprint + status + aging). §14 fix DAG (computed leverage). §15 coverage heat map. §16 output emitters (JSON/Markdown/SARIF/GitHub comment). §17 baseline + CI gating.

**v3.2**: §18 resilience (checkpoint + incremental emit + chunked writes). §19 resume protocol. Failed orchestrator ≠ lost squad work. 45-min restart → 5-10 min resume.

---

## Section 1: USE FOR / DO NOT USE FOR

### USE FOR

- Aggregate findings from multiple audit squads into single report
- Deduplicate overlapping findings across squads
- Generate summary statistics and severity breakdowns
- Validate all squad outputs conform to expected schema
- Cross-reference findings to identify systemic hotspots
- Report quality validation: provenance, naming, consistency checks

### DO NOT USE FOR

- Code-level audits (use domain-specific squads)
- Security scanning (use `audit-security`)
- Architecture review (use `audit-architecture`)
- Financial model validation (use `audit-financial-model`)

---

## Section 2: Target Discovery

Agent MUST auto-detect project structure. Do NOT hardcode repository names or paths.

### 2.1 Language Detection

| Signal | File Pattern | Detected Language |
|--------|-------------|-------------------|
| C# source files | `*.cs`, `*.csproj`, `*.sln`, `*.slnx` | .NET/C# |
| Python source files | `*.py`, `*.pyi`, `pyproject.toml`, `requirements.txt` | Python |
| JavaScript source files | `*.js`, `*.mjs`, `*.cjs` | JavaScript |
| TypeScript source files | `*.ts`, `*.tsx`, `tsconfig.json` | TypeScript |
| Go source files | `*.go`, `go.mod` | Go |
| Java source files | `*.java`, `pom.xml`, `build.gradle` | Java |
| Rust source files | `*.rs`, `Cargo.toml` | Rust |

### 2.2 Output Path Configuration

| Config Source | Priority | How to Read |
|---------------|----------|-------------|
| `AUDIT_OUTPUT_DIR` env var | 1 (highest) | `os.environ["AUDIT_OUTPUT_DIR"]` |
| `audit.config.json` in project root | 2 | Parse JSON, read `outputDir` key |
| `.auditrc` or `.auditrc.json` in project root | 3 | Parse as JSON or key=value |
| Default: `<project_root>/.audit/reports/` | 4 (fallback) | Construct from detected project root |

### 2.3 Discovery Procedure

1. **Detect project root**: Look for `.git/`, solution files, `go.mod`, `package.json`, or `Cargo.toml`.
2. **Identify primary language** by file count.
3. **Resolve output directory**: Check config sources in priority order.
4. **Collect squad outputs**: Scan output directory for files matching `audit-*.json`.
5. **Validate schema**: Each file must conform to per-squad report schema.
6. **Proceed with aggregation**.

---

## Section 3: Standard Output Format — DETAILED FINDINGS (MANDATORY)

### 3.0 HARD RULES (v3.0 — read these FIRST, violations are skill failures)

1. **NO COUNT-ONLY COLLAPSE**: Every finding of EVERY severity (critical, high, medium, low) MUST appear in full detail in the consolidated report's `findings[]` array. It is FORBIDDEN to emit `"high_findings_count": 44` while omitting the 44 finding objects. The v2.0 failure mode (only critical findings detailed, others as bare counts) is a CRITICAL skill violation.
2. **FULL DETAIL CARRY-FORWARD**: Every required v3.0 finding field (`root_cause`, `impact`, `fix_examples`, `fix_guide`, `effort_estimate`, `verification_command`, `references`, `confidence`, `fp_risk_self_assessment`) MUST be preserved end-to-end. Truncation = skill failure.
3. **VALIDATE OR REJECT**: Findings missing required v3.0 fields are REJECTED, not silently dropped. The rejection MUST appear in `provenance.validation_errors` with the squad name + missing fields. Do NOT swallow.
4. **NEVER INFLATE/DEFLATE COUNTS**: `total_findings` MUST equal `len(findings[])`. `severity_breakdown` MUST sum to `total_findings`. Mismatch = validation error, must be fixed before report ships.
5. **NO DUPLICATES IN findings[]**: Same `file+line+category` from multiple squads = ONE merged finding with `corroborating_squads: [...]`. Do not list the same issue twice.
6. **VALIDATION LAYER IS MANDATORY**: The orchestrator MUST run Section 5 (Critical-Thinking Validation) on AT LEAST 20% of findings (minimum: all critical findings + a random sample of high/medium/low). Skipping validation = skill failure.

### 3.1 Per-Squad Report Schema (v3.0 — see `_shared/audit-template.md` Section 7.1)

Every squad produces findings in the v3.0 schema. The consolidated report inherits all fields verbatim and adds aggregation metadata. See `_shared/audit-template.md` Section 7.1 for the canonical schema — it is the source of truth and is REQUIRES reading before running aggregation.

**Required fields per finding** (recap, see template for full format):
`id`, `severity`, `file`, `line`, `endLine` (opt), `title`, `description`, `root_cause`, `impact`, `recommendation`, `fix_examples.{before,after}`, `fix_guide`, `effort_estimate.{minutes,human_readable,confidence,breakdown}`, `technology`, `category`, `detection_method.{tool,query}`, `verification_command`, `references` (≥1), `related_findings` (opt), `confidence.{score,rationale}`, `fp_risk_self_assessment.{risk_level,reasoning,verified_against_conventions,convention_files_checked}`.

### 3.2 Schema Validation (STRICT)

- Reject findings missing any required field. Log to `provenance.validation_errors` as `{squad, finding_id, missing_fields: [...], action: "rejected"}`.
- Reject `references: []` (empty list). At least one reference required.
- Reject `confidence.score` outside [0.0, 1.0].
- Reject `effort_estimate.minutes` ≤ 0.
- Flag (not reject) findings where `fp_risk_self_assessment.verified_against_conventions == false`.
- Findings with `confidence.score < 0.5` → routed to `suspected_false_positives[]` for orchestrator re-verification (see Section 5).

---

## Section 4: Aggregation Rules — FULL FINDINGS CARRIED FORWARD (v3.0)

### 4.1 Step 1 — Collect Individual Reports

Gather all squad reports. Filter by: project match, valid schema, recent timestamp (within audit window).

**Validation per finding** (reject on failure, log to `provenance.validation_errors`):
- All required v3.0 fields present (see `_shared/audit-template.md` Section 7.1).
- `references` array non-empty.
- `confidence.score` ∈ [0.0, 1.0].
- `effort_estimate.minutes` > 0.
- `fp_risk_self_assessment` present with all 4 sub-fields.

**Auto-fill fallback** (only for low-severity findings where the squad omitted detail): synthesize `root_cause`, `impact`, `fix_examples` from `description` + `recommendation` if possible; otherwise REJECT.

### 4.2 Step 2 — Merge Findings (CARRY ALL DETAIL)

Combine all findings arrays into one list. ALL v3.0 detail fields MUST be preserved verbatim — no truncation, no summarization, no field dropping.

Sort by:
1. Severity (critical → high → medium → low)
2. File path (alphabetical)
3. Line number (ascending)

### 4.3 Step 3 — Deduplicate (MERGE DETAILS)

When multiple squads flag the same `file` + overlapping `line range` + same `category`:

- Keep highest-severity version as the base finding.
- Add `corroborating_squads: ["audit-X", "audit-Y"]` array.
- **MERGE** `root_cause` from all squads — prefix each with `[{squad}]: ` and join with `\n---\n`.
- **MERGE** `fix_examples.before/after` into `fix_examples.variants: [{squad, before, after}, ...]` array (keep base in primary `before/after`).
- **MERGE** `fix_guide` steps into a combined numbered list (de-duplicate identical steps).
- **MERGE** `references` (union, de-duplicate).
- **MERGE** `confidence` into `confidence.corroboration_boost`: if ≥2 squads agree, +0.1 (cap 0.99).
- **MERGE** `fp_risk_self_assessment`: if any squad rated `high` FP risk, surface that prominently.
- Set `merged_from: [{squad, original_id}, ...]` for traceability.

### 4.4 Step 4 — Generate Consolidated Report (v3.0 schema)

The consolidated report MUST carry ALL findings (every severity) in full detail.

```json
{
  "report_version": "3.0",
  "type": "consolidated",
  "project": "detected-project-name",
  "timestamp": "2026-06-04T15:00:00Z",
  "technologies": [".NET/C#", "Python"],
  "squads_run": ["audit-code-quality", "audit-security"],
  "squads_missing": ["audit-dependencies"],
  "squads_with_errors": [
    { "squad": "audit-regulatory-sector", "error": "No matching regulatory framework found for this project type — skipped" }
  ],
  "total_findings": 26,
  "severity_breakdown": { "critical": 1, "high": 5, "medium": 12, "low": 8 },
  "themes": [
    {
      "theme": "Multi-tenant encryption isolation broken",
      "root_cause": "Shared DEK cache introduced in commit abc123 without isolation review",
      "finding_count": 3,
      "critical_count": 2,
      "files": ["src/ModuleA/File.cs", "src/ModuleB/File.cs"],
      "finding_ids": ["sec-001", "sec-002", "arch-005"]
    }
  ],
  "top_files": [
    {
      "file": "src/Modules/Billing/Calculator.cs",
      "finding_count": 4,
      "finding_ids": ["cq-001", "cq-002", "sec-003", "arch-002"],
      "squads": ["audit-code-quality", "audit-security"],
      "highest_severity": "high",
      "themes": ["High cyclomatic complexity", "Unvalidated input"]
    }
  ],
  "findings": [
    {
      "id": "cq-001",
      "severity": "critical",
      "file": "src/Modules/Billing/Calculator.cs",
      "line": 45,
      "endLine": 78,
      "title": "God function Calculate() — cyclomatic complexity 18, 4 nested if/for",
      "description": "Fn cyclomatic complexity = 18 (threshold 10). ...",
      "root_cause": "...",
      "impact": "...",
      "recommendation": "...",
      "fix_examples": { "before": "...", "after": "..." },
      "fix_guide": "...",
      "effort_estimate": { "minutes": 240, "human_readable": "4 hours", "confidence": "high", "breakdown": ["..."] },
      "technology": ".NET",
      "category": "maintainability",
      "detection_method": { "tool": "manual-review", "query": "..." },
      "verification_command": "...",
      "references": ["..."],
      "related_findings": ["cq-002"],
      "confidence": { "score": 0.92, "rationale": "..." },
      "fp_risk_self_assessment": { "risk_level": "low", "reasoning": "...", "verified_against_conventions": true, "convention_files_checked": ["AGENTS.md"] },
      "source_squad": "audit-code-quality",
      "corroborating_squads": ["audit-code-quality", "audit-refactoring"],
      "validation_status": "verified",
      "validation_evidence": "Read lines 45-78 directly; complexity claim confirmed via lizzard CLI output"
    }
  ],
  "report_sections": {
    "critical_findings_detailed": ["finding objects with severity=critical"],
    "high_findings_detailed": ["finding objects with severity=high"],
    "medium_findings_detailed": ["finding objects with severity=medium"],
    "low_findings_detailed": ["finding objects with severity=low"]
  },
  "provenance": {
    "generated_by": "audit-consolidated",
    "schema_version": "3.0",
    "source_reports": ["audit-code-quality-20260604.json", "audit-security-20260604.json"],
    "squad_count": 5,
    "clean_squads": ["audit-ai-cost", "audit-regulatory-sector"],
    "validation_errors": [],
    "validation_sample_size": 12,
    "validation_sample_percent": 46.2,
    "fp_detected_count": 2,
    "fn_detected_count": 5
  }
}
```

---

## Section 5: Critical-Thinking Validation Layer (v3.0 MANDATORY)

**Purpose**: The orchestrator (build agent running `/audit`) MUST independently verify a sample of findings against the actual codebase. This catches false positives that squads self-assessed as `low` FP risk but are actually FPs, and validates that high-severity findings are real before they consume reviewer attention.

### 5.1 Sample Selection (Minimum Coverage)

The orchestrator MUST validate AT LEAST:

| Tier | What to validate | Minimum % |
|------|------------------|-----------|
| **All critical findings** | 100% — every critical finding must be independently verified | 100% |
| **High findings** | Stratified random sample | ≥30% |
| **Medium findings** | Random sample | ≥15% |
| **Low findings** | Random sample | ≥5% |
| **Suspect queue** (`confidence.score < 0.5` OR `fp_risk_self_assessment.risk_level == "high"`) | 100% — every suspect finding re-verified | 100% |

If the total sample < 20% of all findings, expand the medium/low samples until ≥20%.

### 5.2 Validation Procedure (per sampled finding)

For each sampled finding, the orchestrator performs:

1. **Read the cited code** — open `file:line` to `file:endLine`, confirm the pattern exists as claimed. If the cited code does not exist or doesn't match the description → mark `validation_status: "fp-confirmed"` and move to Section 6 (FP handling).
2. **Run `verification_command`** — execute the squad-provided command from project root. Capture output. If the command produces no hits (or hits that contradict the finding) → `validation_status: "fp-confirmed"`.
3. **Check conventions** — read `AGENTS.md`, `CONTRIBUTING.md`, `.sisyphus/plans/*conventions*.md`, `.sisyphus/decisions/*.md` for explicit overrides that allow the pattern. If a convention explicitly permits it → `validation_status: "fp-convention-override"` (still record for visibility, but tag as allowed).
4. **Cross-reference squads** — if ≥2 independent squads flagged the same finding with high confidence, FP risk drops substantially. If 1 squad says "issue" and another squad audited the same area and found nothing, that's a contradiction — investigate which is right.
5. **Verify version claims** (dependency findings only) — run `cargo tree -i <pkg> --target all` / `npm ls <pkg>` / `pip show <pkg>` to confirm the version + feature flags match the finding's premise.
6. **Set `validation_status`** to one of:
   - `verified` — independently confirmed real.
   - `fp-confirmed` — verified as false positive (with `fp_reason` field).
   - `fp-convention-override` — pattern exists but repo explicitly allows it (record for visibility, exclude from fix queue).
   - `partially-verified` — some claims verified, others not (e.g., complexity number was wrong but the function is still too long).
   - `unverified` — orchestrator couldn't verify (e.g., cited file doesn't exist on this branch, command requires unavailable tooling). Surface for human review.
7. **Attach `validation_evidence`** — short note documenting what was checked: "Read lines 6241-9791 of rust_codegen.rs; confirmed two god-functions via `wc -l` and ast-grep match block pattern."

### 5.3 Validation Outputs

After validation, each sampled finding gets two new fields:

```json
{
  "validation_status": "verified|fp-confirmed|fp-convention-override|partially-verified|unverified",
  "validation_evidence": "What the orchestrator did to verify. Concrete: commands run, files read, outputs observed.",
  "fp_reason": "Required if status is fp-confirmed or fp-convention-override. Cite the FP category from Section 6."
}
```

Aggregate counts go to `provenance`:

```json
{
  "validation_sample_size": 32,
  "validation_sample_percent": 65.3,
  "validation_results": {
    "verified": 26,
    "fp-confirmed": 3,
    "fp-convention-override": 1,
    "partially-verified": 1,
    "unverified": 1
  }
}
```

### 5.4 Validation Discipline Rules

- **Never skip validation** because the squad rated itself as `low` FP risk. That's circular reasoning. The whole point of independent validation is to catch self-assessment errors.
- **Bias toward skepticism on critical findings**. Critical findings drive action; an unverified critical wastes reviewer cycles and erodes trust.
- **Document negative results too**. If validation confirmed a finding as real, attach a one-line evidence note. Future audits benefit from knowing it was checked.
- **Re-verify findings the squad flagged as `high` FP risk**. They were honest; respect that by double-checking.

---

## Section 6: False Positive Detection & Dismissal

### 6.1 FP Categories (see `_shared/audit-template.md` Section 11.1 for full table)

Recap of FP categories the orchestrator must consider when validating:
- **FP-1: Convention Override** — repo explicitly permits the pattern.
- **FP-2: Version Mismatch** — claim depends on a version that doesn't match.
- **FP-3: Scope Misunderstanding** — pattern is intentional in this context.
- **FP-4: Stale Claim** — code has changed since the pattern was added.
- **FP-5: Heuristic Over-Fire** — pattern matched but doesn't apply (generated/vendored files).
- **FP-6: Tool False Positive** — scanner noise (dev-only CVE rated critical).
- **FP-7: Already Mitigated** — compensating control covers the risk.
- **FP-8: Out of Scope** — squad flagged outside its lane (should cross-reference, not duplicate).
- **FP-9: Severity Mis-Calibration** — real issue but wrong severity bucket.
- **FP-10: Phantom Pattern** — squad hallucinated code that doesn't exist.

### 6.2 FP Output Schema

The consolidated report MUST include a `false_positives` array (even if empty):

```json
"false_positives": [
  {
    "original_finding_id": "sec-014",
    "original_squad": "audit-security",
    "fp_category": "FP-1",
    "fp_reason": "AGENTS.md line 19 explicitly permits unwrap() in test code under tests/ directory; this finding flagged tests/parser_test.rs:42 which is in the exempted scope",
    "convention_source": "AGENTS.md:19 + CONTRIBUTING.md:34",
    "recommendation": "Dismiss; optionally add a per-file lint allowlist comment for clarity"
  }
]
```

### 6.3 FP Discipline Rules

- **Every sampled finding that fails validation** MUST appear in `false_positives` with reasoning.
- **FPs are not deletions** — they remain in `findings[]` with `validation_status: "fp-confirmed"` so reviewers can see the FP analysis, but they are EXCLUDED from `recommended_first_actions_ordered_by_leverage`.
- **Severity mis-calibration (FP-9)** is a soft FP — finding stays active but with severity adjusted and `original_severity` recorded.
- **Convention overrides (FP-1)** should prompt a recommendation to make the convention more visible (e.g., add a `// allow: per AGENTS.md §X` comment) so future audits don't waste cycles.

---

## Section 7: False Negative Detection & Coverage Gaps

**Purpose**: FPs are visible noise; FNs are invisible holes. Both erode trust. The orchestrator MUST enumerate likely FNs that no squad caught.

### 7.1 FN Sources (where to look for likely FNs)

The orchestrator enumerates FNs from 4 sources:

1. **Aggregation of squad `falseNegativesIdentified`** — each squad's v3.0 summary MUST include ≥3 likely FNs in its domain. Consolidate them here.
2. **Cross-squad gaps** — areas where two adjacent squads both assumed the other would cover it (e.g., "supply-chain signing" lives between security/cicd/license). The orchestrator notices when no squad touched a file/pattern.
3. **Top-N largest unaudited files** — sort repo by LOC, identify files with zero findings, check whether they should have had any.
4. **Top-N most-modified files (recent git activity)** — recent churn is a leading indicator of new issues. If a heavily-churned file has zero findings, that's suspicious.

### 7.2 FN Enumeration Procedure

1. **Collect** each squad's `summary.falseNegativesIdentified` array.
2. **Cross-reference** against `files_with_findings` map — for each FN candidate, did any other squad actually cover it? If yes, downgrade. If no, surface.
3. **Scan large unaudited files**:
   - Run `find . -type f \( -name '*.rs' -o -name '*.cs' -o -name '*.py' -o -name '*.ts' -o -name '*.go' -o -name '*.java' \) | xargs wc -l | sort -rn | head -20`.
   - For each file in top-20 with zero findings, ask: "Should a squad have flagged something here?" If yes, add to FN list.
4. **Scan recent churn** (last 30 days of git history):
   - `git log --since='30 days ago' --name-only --pretty=format: | sort | uniq -c | sort -rn | head -20`.
   - For each heavily-churned file with zero findings, add to FN list.
5. **Identify cross-cutting gaps** — for each pair of adjacent squads, list 1-3 patterns neither would naturally cover.

### 7.3 FN Output Schema

```json
"false_negatives": [
  {
    "area": "crates/buff-lang-cli/src/ui_dev/server.rs — WebSocket handler",
    "reason": "No squad audited WebSocket-specific attack surface (CSWSH, origin validation, message size limits). audit-security checklist does not include WS; audit-fault-tolerance did not flag connection lifecycle.",
    "source": "cross-squad-gap + file-size-scan (1,840 LOC, zero findings)",
    "suggestedSquad": "audit-security (with WS-specific checklist extension)",
    "severity_if_found": "high",
    "suggestedVerificationCommand": "rg -n 'ws::|WebSocket|on_message' crates/buff-lang-cli/src/ui_dev/",
    "file_loc": 1840,
    "git_churn_30d": 12
  }
]
```

### 7.4 FN Discipline Rules

- **Minimum FN count**: The consolidated report MUST list AT LEAST 5 likely false negatives. If fewer than 5, the orchestrator hasn't looked hard enough.
- **Every FN must have a `suggestedVerificationCommand`** so the user can investigate themselves.
- **Every FN must be routed** — either to an existing squad (`suggestedSquad`) or to the consistency-coherence squad (`cross-cutting-gap` type).
- **FNs are not failures** — they are honest accounting of audit coverage. Documenting them is a sign of rigor, not weakness.

---

## Section 8: Report Quality Checklist (v3.0)

Before finalizing consolidated report, the orchestrator MUST verify ALL of the following. Failure on any item blocks the report from shipping:

### Aggregation integrity
- [ ] **All-severity carry-forward**: `findings[]` contains ALL findings of ALL severities (not just critical). Counts in `severity_breakdown` match `len(findings[severity=X])` exactly.
- [ ] **No count-only collapse**: There is NO field like `high_findings_count` without corresponding finding objects. (The v2.0 failure mode.)
- [ ] **All required v3.0 fields present** on every finding: `id`, `severity`, `file`, `line`, `title`, `description`, `root_cause`, `impact`, `recommendation`, `fix_examples.{before,after}`, `fix_guide`, `effort_estimate`, `technology`, `category`, `detection_method.{tool,query}`, `verification_command`, `references` (≥1), `confidence.{score,rationale}`, `fp_risk_self_assessment.{risk_level,reasoning,verified_against_conventions,convention_files_checked}`.
- [ ] **Deduplication applied**: No two findings in `findings[]` have same `file+line+category` unless they are genuinely distinct. Corroborating findings have `corroborating_squads` populated.
- [ ] **Severity counts consistent**: `total_findings` equals sum of `severity_breakdown` values equals `len(findings[])`.

### Validation layer (Section 5)
- [ ] **Validation sample ≥20%** of total findings (and 100% of critical findings).
- [ ] **Every sampled finding has `validation_status`** set (not blank).
- [ ] **Every non-`verified` finding has `fp_reason` or `validation_evidence`** explaining the deviation.
- [ ] **`provenance.validation_results`** populated with the 5 status buckets.

### FP/FN accounting
- [ ] **`false_positives[]` array present** (empty allowed, but must exist). Each entry has `original_finding_id`, `fp_category`, `fp_reason`, `convention_source` (if applicable).
- [ ] **`false_negatives[]` array present** with ≥5 entries. Each entry has `area`, `reason`, `suggestedSquad`, `severity_if_found`, `suggestedVerificationCommand`.
- [ ] **Cross-squad gaps** explicitly listed (pairs of adjacent squads where neither covered shared territory).

### Provenance & traceability
- [ ] **`provenance.source_reports`** lists all consumed squad reports.
- [ ] **`provenance.validation_errors`** lists rejected findings with squad + missing fields.
- [ ] **`provenance.task_ids`** preserved for downstream `background_output` retrieval.
- [ ] **File paths relative**: All paths relative to project root, forward slashes.

---

## Section 9: Anti-Patterns (v3.0 — includes FP/FN failure modes)

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | **Count-only collapse of non-critical findings** | **critical** | v2.0 failure mode: only critical findings detailed, high/medium/low reduced to `"count": N` | 144+ findings become invisible; reviewers can't act on counts | HARD RULE: every finding of every severity carries full v3.0 detail in `findings[]` |
| 2 | Hardcoded output paths | high | Output path hardcoded to specific repo dir | Not portable, breaks in CI/CD | Use configurable path from env var or config file |
| 3 | Missing provenance metadata | medium | Consolidated report has no `source_reports` | Cannot trace findings back to source squads | Always include `provenance` object with full source list |
| 4 | Inconsistent severity counts | high | `total_findings` doesn't match sum of severity breakdown | Undermines report credibility | Validate counts before emitting report |
| 5 | Silently dropping squad outputs | critical | Squad report fails validation but error is swallowed | Missing findings create false security | Log validation errors, include in `squads_missing` with reason |
| 6 | Duplicate findings not merged | medium | Same issue reported by multiple squads appears multiple times | Inflates finding count, confuses reviewers | Apply deduplication with `corroborating_squads` field |
| 7 | **Skipping validation layer** | **critical** | Orchestrator aggregates without independently verifying any findings | FPs ship as if real; reviewer trust erodes | MANDATORY: validate ≥20% sample + 100% of critical + 100% of suspect queue |
| 8 | **Auto-trusting squad self-assessed FP risk** | high | Orchestrator copies squad's `fp_risk_self_assessment` without re-checking | Circular reasoning — squad's self-assessment is the thing being audited | Independent verification per Section 5; bias toward skepticism on critical |
| 9 | **FN enumeration omitted** | **critical** | Consolidated report has no `false_negatives[]` array | Invisible coverage holes; reviewers think audit was exhaustive when it wasn't | MANDATORY: ≥5 likely FNs with `suggestedVerificationCommand` |
| 10 | **Phantom findings accepted** | high | Orchestrator didn't read the cited file/line to confirm the pattern exists | Hallucinated findings waste reviewer time | Always run `verification_command` for sampled findings; reject phantom findings |
| 11 | **Convention overrides ignored** | medium | Repo explicitly permits a pattern (in AGENTS.md, ADR, conventions) but finding ships anyway | Wastes reviewer cycles; erodes trust in audit | Read convention files during validation; dismiss as `fp-convention-override` |
| 12 | **Severity inflation/deflation** | medium | Real finding but wrong severity bucket | Mis-allocates reviewer attention | Apply severity triggers from Section 8 of `_shared/audit-template.md` |

---

## Section 10: Code Example — Report Aggregation (v3.0 reference implementation)

```python
"""
Reference implementation of v3.0 audit-consolidated aggregation.
Adapt to your project's language/tools as needed.
"""
import json
import subprocess
from pathlib import Path
from collections import defaultdict
from datetime import datetime, timezone


REQUIRED_V3_FIELDS = {
    "id", "severity", "file", "line", "title", "description", "root_cause",
    "impact", "recommendation", "fix_examples", "fix_guide", "effort_estimate",
    "technology", "category", "detection_method", "verification_command",
    "references", "confidence", "fp_risk_self_assessment",
}


def aggregate_reports(output_dir: Path, project_name: str, project_root: Path) -> dict:
    """Collect, validate, deduplicate, independently verify, and merge squad reports."""
    all_findings = []
    squads_run = []
    squads_with_errors = []
    validation_errors = []

    # Step 1: Collect + schema-validate
    for report_file in sorted(output_dir.glob("audit-*.json")):
        try:
            report = json.loads(report_file.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, UnicodeDecodeError) as e:
            squads_with_errors.append({"file": report_file.name, "error": str(e)})
            continue

        for finding in report.get("findings", []):
            missing = REQUIRED_V3_FIELDS - finding.keys()
            if missing:
                validation_errors.append({
                    "squad": report.get("squad", "unknown"),
                    "finding_id": finding.get("id", "unknown"),
                    "missing_fields": sorted(missing),
                    "action": "rejected",
                })
                continue
            finding["_source_squad"] = report["squad"]
            all_findings.append(finding)

        squads_run.append(report["squad"])

    # Step 2: Deduplicate
    deduplicated = _deduplicate_findings(all_findings)

    # Step 3: Sort
    severity_order = {"critical": 0, "high": 1, "medium": 2, "low": 3}
    deduplicated.sort(key=lambda f: (
        severity_order.get(f["severity"], 99),
        f.get("file", ""),
        f.get("line", 0),
    ))

    # Step 4: Independent validation (sample)
    validated = _run_validation_layer(deduplicated, project_root)

    # Step 5: Aggregate FPs + FNs from squads
    false_positives = [f for f in validated if f["validation_status"].startswith("fp")]
    false_negatives = _collect_false_negatives(output_dir, project_root)

    # Step 6: Build severity breakdown
    by_severity = defaultdict(int)
    for finding in validated:
        by_severity[finding["severity"]] += 1

    return {
        "report_version": "3.0",
        "type": "consolidated",
        "project": project_name,
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "squads_run": squads_run,
        "squads_with_errors": squads_with_errors,
        "total_findings": len(validated),
        "severity_breakdown": dict(by_severity),
        "findings": validated,
        "false_positives": false_positives,
        "false_negatives": false_negatives,
        "provenance": {
            "generated_by": "audit-consolidated",
            "schema_version": "3.0",
            "source_reports": [f.name for f in sorted(output_dir.glob("audit-*.json"))],
            "validation_errors": validation_errors,
            "validation_sample_size": sum(1 for f in validated if f.get("validation_status")),
            "validation_sample_percent": (
                sum(1 for f in validated if f.get("validation_status")) / max(len(validated), 1) * 100
            ),
        },
    }


def _deduplicate_findings(findings: list[dict]) -> list[dict]:
    """Merge overlapping findings, track corroborating squads, preserve all detail."""
    groups: dict[str, list[dict]] = defaultdict(list)
    for f in findings:
        key = f"{f.get('file', '')}:{f.get('line', 0)}:{f.get('category', '')}"
        groups[key].append(f)

    severity_rank = {"critical": 0, "high": 1, "medium": 2, "low": 3}
    result = []
    for group in groups.values():
        best = min(group, key=lambda f: severity_rank.get(f["severity"], 99))
        squads = list({f.get("_source_squad", "unknown") for f in group})
        if len(squads) > 1:
            best["corroborating_squads"] = squads
            best["confidence"] = best.get("confidence", {"score": 0.5, "rationale": ""})
            best["confidence"]["corroboration_boost"] = min(
                best["confidence"]["score"] + 0.1 * (len(squads) - 1), 0.99
            )
        best.pop("_source_squad", None)
        result.append(best)
    return result


def _run_validation_layer(findings: list[dict], project_root: Path) -> list[dict]:
    """
    Independently verify a sample of findings.
    - 100% of critical
    - ≥30% of high
    - ≥15% of medium
    - ≥5% of low
    - 100% of suspect queue (confidence < 0.5 OR fp_risk == 'high')
    """
    import random
    random.seed(42)  # reproducible sampling

    by_severity = defaultdict(list)
    suspect = []
    for f in findings:
        if f["severity"] == "critical":
            by_severity["must_validate"].append(f)
        elif f.get("confidence", {}).get("score", 1.0) < 0.5 or \
             f.get("fp_risk_self_assessment", {}).get("risk_level") == "high":
            suspect.append(f)
        else:
            by_severity[f["severity"]].append(f)

    sample_ratios = {"high": 0.30, "medium": 0.15, "low": 0.05}
    to_validate = list(by_severity["must_validate"]) + suspect
    for sev, ratio in sample_ratios.items():
        pool = by_severity[sev]
        n = max(1, int(len(pool) * ratio)) if pool else 0
        to_validate.extend(random.sample(pool, min(n, len(pool))))

    for f in to_validate:
        f["validation_status"], f["validation_evidence"] = _verify_finding(f, project_root)
        if f["validation_status"].startswith("fp"):
            f["fp_reason"] = "Document during validation"

    return findings


def _verify_finding(finding: dict, project_root: Path) -> tuple[str, str]:
    """Read the cited file, run verification_command, return (status, evidence)."""
    cited_path = project_root / finding["file"]
    if not cited_path.exists():
        return "unverified", f"File {finding['file']} does not exist on this branch"

    # Read cited lines
    try:
        lines = cited_path.read_text(encoding="utf-8").splitlines()
        cited_slice = "\n".join(lines[max(0, finding["line"] - 1): finding.get("endLine", finding["line"])])
    except Exception as e:
        return "unverified", f"Could not read {finding['file']}: {e}"

    # Run verification command if provided
    cmd = finding.get("verification_command", "")
    cmd_output = ""
    if cmd:
        try:
            result = subprocess.run(
                cmd, shell=True, cwd=project_root, capture_output=True, text=True, timeout=30
            )
            cmd_output = result.stdout or result.stderr
        except Exception as e:
            cmd_output = f"[command failed: {e}]"

    return "verified", f"Read lines {finding['line']}-{finding.get('endLine', finding['line'])}; verification_command output: {cmd_output[:200]}"


def _collect_false_negatives(output_dir: Path, project_root: Path) -> list[dict]:
    """Aggregate squad-reported FNs + scan for large/churned files with zero findings."""
    fns = []
    for report_file in sorted(output_dir.glob("audit-*.json")):
        report = json.loads(report_file.read_text(encoding="utf-8"))
        for fn in report.get("summary", {}).get("falseNegativesIdentified", []):
            fn["source_squad"] = report.get("squad")
            fns.append(fn)

    # Cross-cutting gap example: WebSocket, SSRF, supply-chain signing
    # (orchestrator should add project-specific entries based on actual gaps)
    return fns
```

---

## Section 11: Output Format

Uses standard v3.0 finding schema (see `_shared/audit-template.md` Section 7.1). Additionally:

### 11.1 Consolidated Report File Naming

```
{output_dir}/consolidated-audit-{YYYYMMDD}.json
{output_dir}/consolidated-audit-{YYYYMMDD}-{HHMMSS}.json  (if multiple runs per day)
{output_dir}/consolidated-audit-{YYYYMMDD}-{HHMMSS}-v3.json  (when re-running with v3.0 skills)
```

### 11.2 Output Path Resolution (Priority Order)

1. `AUDIT_OUTPUT_DIR` environment variable
2. `audit.config.json` → `outputDir` key
3. Default: `<project_root>/.audit/reports/` (or `.sisyphus/reports/` for Sisyphus-managed projects)

---

## Section 12: Integration Notes

### 12.1 Squad Dependency Graph

```
audit-security ─────────────────────┐
audit-architecture ─────────────────┤
audit-code-quality ─────────────────┤
audit-dependencies ─────────────────┤
audit-test-coverage ────────────────┤
audit-fault-tolerance ──────────────┤
audit-observability ────────────────┤
audit-privacy-data ─────────────────┤
audit-financial-model ──────────────┤
audit-regulatory-* ─────────────────┤
audit-consolidated ◄────────────────┘  (consumes ALL squad outputs + runs validation layer)
audit-consistency-coherence            (consumes consolidated + squad outputs, cross-validates)
```

### 12.2 Upstream Dependencies

This squad **consumes** outputs from ALL other audit squads:
- Every squad that produces `audit-*.json` files in output directory
- Validates each report against v3.0 schema before aggregation (see Section 3.2)
- Inherits each squad's `falseNegativesIdentified` for Section 7 FN aggregation

### 12.3 Downstream Consumers

- `audit-consistency-coherence` — consumes consolidated report for cross-domain validation
- Human reviewers — read consolidated reports for sign-off
- CI/CD pipelines — may gate on consolidated severity counts (after FP filtering)

### 12.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-consolidated | audit-consistency-coherence | Consolidated aggregates findings + runs validation layer; consistency validates cross-domain alignment |
| audit-consolidated | audit-lmas-traceability | Consolidated aggregates findings; traceability tracks agent spans |
| audit-consolidated | audit-security (FP review) | Consolidated independently re-verifies security findings; security still owns the original scan |

### 12.5 Recommended Execution Order

1. All domain-specific squads run first (with v3.0 schema + per-squad FN enumeration)
2. `audit-consolidated` aggregates all findings + runs validation layer + enumerates cross-cutting FNs
3. `audit-consolidated` v3.1: computes fingerprints, builds dependency DAG, renders coverage map, runs delta vs `--previous` (if provided)
4. `audit-consolidated` v3.1: applies baseline suppression (if `--baseline` provided), emits in requested `--output-format`(s)
5. `audit-consistency-coherence` validates cross-domain alignment (LAST)

---

## Section 13: Delta Tracking (v3.1 — fingerprint + status across runs)

See `_shared/audit-template.md` Section 13 for the field schema and computation rules. The orchestrator's job at consolidation time:

### 13.1 Fingerprint Computation (deterministic)

For every finding missing a `fingerprint` field, compute one:

```python
import hashlib

def compute_fingerprint(finding: dict) -> str:
    """Stable 16-char ID that survives line-number drift."""
    norm_path = finding["file"].lower().replace("\\", "/").lstrip("./")
    norm_category = finding["category"].lower().strip()
    norm_root_cause = (
        finding.get("root_cause", "")
        .lower()
        .replace(":", "").replace(",", "").replace(".", "")
        .replace("line ", "").replace("lines ", "")  # strip line refs
    )
    # Strip version-specific tokens (e.g., "ethers 2.0" -> "ethers")
    import re
    norm_root_cause = re.sub(r"\b\d+\.\d+\.\d+\b", "", norm_root_cause)
    norm_root_cause = re.sub(r"\s+", " ", norm_root_cause).strip()
    
    raw = f"{norm_path}::{norm_category}::{norm_root_cause}"
    return hashlib.sha256(raw.encode("utf-8")).hexdigest()[:16]
```

### 13.2 Status Computation (requires `--previous` report)

If `audit --previous path/to/old.json` was provided, the orchestrator loads the previous report's findings, indexes them by `fingerprint`, and tags each current finding with `status`:

```python
def compute_status(current: dict, previous_by_fp: dict[str, dict]) -> dict:
    fp = current["fingerprint"]
    if fp not in previous_by_fp:
        current["status"] = "new"
        current["first_seen"] = current.get("first_seen", datetime.utcnow().isoformat())
    else:
        prev = previous_by_fp[fp]
        sev_order = {"low": 0, "medium": 1, "high": 2, "critical": 3}
        if sev_order[current["severity"]] > sev_order[prev["severity"]]:
            current["status"] = "regressed"
        elif sev_order[current["severity"]] < sev_order[prev["severity"]]:
            current["status"] = "improved"
        else:
            current["status"] = "recurring"
        current["first_seen"] = prev.get("first_seen", prev.get("timestamp"))
        current["previous_severity"] = prev["severity"]
    current["last_seen"] = datetime.utcnow().isoformat()
    return current

def compute_fixed(current_findings: list[dict], previous_by_fp: dict[str, dict]) -> list[dict]:
    current_fps = {f["fingerprint"] for f in current_findings}
    return [
        {
            "fingerprint": fp,
            "previous_severity": prev["severity"],
            "previous_id": prev.get("id"),
            "fixed_in_commit": None,  # discoverable via git log -S root_cause
            "fixed_at": None,
        }
        for fp, prev in previous_by_fp.items()
        if fp not in current_fps
    ]
```

### 13.3 Aging Surfacing (findings open >90 days)

```python
def surface_aging(findings: list[dict], threshold_days: int = 90) -> list[dict]:
    now = datetime.utcnow()
    aging = []
    for f in findings:
        first_seen = datetime.fromisoformat(f.get("first_seen", now.isoformat()).replace("Z", ""))
        days_open = (now - first_seen).days
        if days_open >= threshold_days and f["status"] in {"recurring", "regressed"}:
            f["aging_days"] = days_open
            aging.append(f)
    return aging
```

Aging findings get a `recommendation`: "Long-standing {severity} — consider explicit accept-risk decision via `--baseline` rather than repeated flagging."

### 13.4 Delta Output Block

The consolidated report includes a top-level `delta` block (see template Section 13.3 for schema). If `--previous` was not provided, the `delta` block is `null` with a `reason: "no previous report provided"` note — the orchestrator doesn't fail, but trending isn't available.

---

## Section 14: Fix Dependency DAG (v3.1 — computed leverage ranking)

**Goal**: Replace manual `recommended_first_actions_ordered_by_leverage` with computed leverage. Fix C-019 (cargo-deny gate) unblocks C-001 + C-002 — this relationship should be inferred from root-cause clustering, not hand-curated.

### 14.1 Root-Cause Clustering

Group findings by normalized `root_cause` (same normalization as fingerprint, but coarser — match on first 200 chars or first sentence). Findings in the same cluster share a root cause.

```python
def cluster_by_root_cause(findings: list[dict]) -> dict[str, list[dict]]:
    """Group findings sharing a root cause."""
    clusters = defaultdict(list)
    for f in findings:
        rc = f.get("root_cause", "").split(".")[0][:200].lower().strip()
        clusters[rc].append(f)
    return clusters
```

### 14.2 Dependency Inference Heuristics

Within each root-cause cluster, identify the "keystone" finding (fixing it makes others moot):

| Pattern | Rule | Example |
|---------|------|---------|
| **Preventive control missing** | Finding describing "no gate/policy/check" unblocks findings describing specific violations | "no cargo-deny gate" (C-019) unblocks "ethers pulls cc-rs" (C-001) + "rusqlite bundled" (C-002) |
| **Architectural root** | Finding in `architecture` category unblocks findings in dependent layers | "no separation between fmt and CLI" unblocks "code-duplication between pipeline.rs and lib.rs" |
| **Convention gap** | Finding describing "no documented rule" unblocks findings describing specific violations | "no LICENSE files" (C-017) unblocks per-crate license mismatches |
| **Higher in dep tree** | Dependency finding on root cause crate unblocks findings on dependent crates | "ethers smuggles cc-rs" unblocks "buff-web3 fails to compile on Windows" |

```python
def infer_unblocks(findings: list[dict]) -> dict[str, list[str]]:
    """For each finding, infer which other findings it unblocks."""
    clusters = cluster_by_root_cause(findings)
    unblocks = defaultdict(list)
    
    for cluster_root_cause, cluster_findings in clusters.items():
        if len(cluster_findings) < 2:
            continue  # No leverage within a singleton cluster
        
        # Find the keystone (preventive/architectural/convention/dep-root)
        keystone_keywords = ["no gate", "no policy", "no check", "no documented", 
                             "no automated", "missing enforcement", "missing endpoint"]
        keystones = [
            f for f in cluster_findings
            if any(kw in f.get("root_cause", "").lower() for kw in keystone_keywords)
            or f.get("category") in {"architecture", "compliance"}
        ]
        
        for keystone in keystones:
            for f in cluster_findings:
                if f["id"] != keystone["id"]:
                    unblocks[keystone["id"]].append(f["id"])
                    # Inverse:
                    unblocks[f["id"]].append("__BLOCKED_BY__:" + keystone["id"])
    
    return dict(unblocks)
```

### 14.3 Transitive Closure + Leverage Score

For each finding, compute `transitive_unblocks_count` (how many findings downstream are unblocked transitively). Leverage score = `transitive_unblocks_count / max(1, effort_minutes / 60)`:

```python
def compute_leverage(unblocks: dict[str, list[str]], findings: list[dict]) -> list[dict]:
    """Rank findings by leverage (downstream impact per hour of effort)."""
    effort_by_id = {f["id"]: f["effort_estimate"]["minutes"] for f in findings}
    
    # Compute transitive closure (simple BFS)
    def transitively_unblocks(fid: str, seen: set[str] = None) -> set[str]:
        if seen is None:
            seen = set()
        for downstream in unblocks.get(fid, []):
            if downstream.startswith("__BLOCKED_BY__:"):
                continue
            if downstream not in seen:
                seen.add(downstream)
                transitively_unblocks(downstream, seen)
        return seen
    
    leverage = []
    for f in findings:
        downstream = transitively_unblocks(f["id"])
        hours = max(0.1, effort_by_id.get(f["id"], 60) / 60)
        score = len(downstream) / hours
        leverage.append({
            "finding_id": f["id"],
            "transitive_unblocks": sorted(downstream),
            "transitive_unblocks_count": len(downstream),
            "effort_hours": hours,
            "leverage_score": round(score, 2),
        })
    
    leverage.sort(key=lambda x: -x["leverage_score"])
    return leverage
```

### 14.4 Output Schema

The consolidated report gains a `fix_dependency_dag` block:

```json
"fix_dependency_dag": {
  "clusters": [
    {
      "root_cause_signature": "no automated gate enforcing no-c-library rule",
      "finding_ids": ["dep-001", "arch-002", "cc-001"],
      "keystone_finding_id": "cc-001",
      "total_effort_if_keystone_only": 240,
      "total_effort_if_all_separately": 960,
      "savings_by_fixing_keystone": 720
    }
  ],
  "leverage_ranking": [
    {
      "finding_id": "cc-001",
      "transitive_unblocks_count": 5,
      "transitive_unblocks": ["dep-001", "arch-002", "sec-003", "cq-004", "ft-005"],
      "effort_hours": 4.0,
      "leverage_score": 1.25,
      "rank": 1
    }
  ],
  "recommended_fix_order": ["cc-001", "cq-003", "sec-014", "..."]
}
```

`recommended_fix_order` is the topological sort of the DAG (fix keystones first). This REPLACES the manual `recommended_first_actions_ordered_by_leverage`.

---

## Section 15: Coverage Heat Map (v3.1 — measured negative space)

**Goal**: Aggregate `files_examined` from all squads; surface large/churned files with zero coverage. See template Section 12 for the per-squad schema.

### 15.1 Aggregation Procedure

```python
def aggregate_coverage(squad_reports: list[dict], project_root: Path) -> dict:
    """Build coverage heat map across all squads."""
    files_examined = defaultdict(lambda: {"squads": [], "lines_read": 0, "findings": []})
    
    for report in squad_reports:
        squad = report["squad"]
        for entry in report.get("summary", {}).get("files_examined", []):
            path = entry["path"]
            files_examined[path]["squads"].append(squad)
            files_examined[path]["lines_read"] = max(
                files_examined[path]["lines_read"], entry["lines_read"]
            )
            files_examined[path]["findings"].extend(entry.get("findings_emitted", []))
    
    # Compute total source files / lines in repo
    total_files, total_lines = count_source_files(project_root)
    
    # Identify large uncovered files (top-N by LOC)
    uncovered = []
    for path, loc in top_n_files_by_loc(project_root, n=50):
        if path not in files_examined or not files_examined[path]["squads"]:
            uncovered.append({"path": path, "loc": loc, "zero_examinations": True})
    
    # Identify heavily-churned uncovered files (top-N by recent git activity)
    for path, commits in top_n_files_by_git_churn(project_root, days=30, n=50):
        if path not in files_examined or not files_examined[path]["squads"]:
            uncovered.append({
                "path": path, "git_churn_30d": commits, "zero_examinations": True,
                "risk_reason": "Heavily churned, never examined",
            })
    
    return {
        "total_source_files": total_files,
        "total_source_lines": total_lines,
        "files_examined_count": len(files_examined),
        "coverage_percent": round(100 * len(files_examined) / max(1, total_files), 1),
        "line_coverage_percent": round(
            100 * sum(v["lines_read"] for v in files_examined.values()) / max(1, total_lines), 1
        ),
        "by_squad": {
            squad: {
                "files": sum(1 for v in files_examined.values() if squad in v["squads"]),
                "lines": sum(v["lines_read"] for v in files_examined.values() if squad in v["squads"]),
            }
            for squad in {s for v in files_examined.values() for s in v["squads"]}
        },
        "uncovered_high_risk_files": uncovered[:20],  # top 20
    }
```

### 15.2 Output Schema

The consolidated report gains a `coverage_map` block:

```json
"coverage_map": {
  "total_source_files": 487,
  "total_source_lines": 48210,
  "files_examined_count": 92,
  "coverage_percent": 18.9,
  "line_coverage_percent": 22.4,
  "by_squad": {
    "audit-code-quality": { "files": 32, "lines": 3120 },
    "audit-security": { "files": 18, "lines": 1840 }
  },
  "uncovered_high_risk_files": [
    {
      "path": "crates/buff-lang-codegen-rust/src/rust_codegen.rs",
      "loc": 10570,
      "zero_examinations": false,
      "squads_examined": ["audit-code-quality", "audit-refactoring"],
      "findings_emitted_count": 5,
      "verdict": "examined"
    },
    {
      "path": "crates/buff-lang-cli/src/ui_dev/server.rs",
      "loc": 1840,
      "git_churn_30d": 12,
      "zero_examinations": true,
      "risk_reason": "Heavily churned, never examined by any squad",
      "verdict": "blind-spot"
    }
  ],
  "coverage_warnings": [
    "Overall file coverage 18.9% < 30% threshold — most source files were not examined by any squad",
    "12 files with >500 LOC have zero examinations across all squads"
  ]
}
```

### 15.3 Coverage Discipline Rules

1. **`coverage_percent < 30%`** → mandatory warning in the report.
2. **Top-N uncovered files by LOC** MUST appear even if no FN was reported.
3. **Top-N uncovered files by git churn** MUST appear (recent churn is a leading indicator).
4. **`uncovered_high_risk_files[]` feeds back into `false_negatives[]`** — each entry becomes a candidate FN.

---

## Section 16: Output Emitters (v3.1 — multi-format output)

The consolidated report can be emitted in multiple formats. Default is JSON. Other formats are triggered by `--output-format {json,markdown,sarif,github-comment,all}`.

### 16.1 Format Selection

```python
def emit_reports(consolidated: dict, output_dir: Path, formats: list[str]) -> dict[str, Path]:
    """Emit consolidated report in requested formats. Returns paths."""
    paths = {}
    timestamp = consolidated["timestamp"].replace(":", "").replace("-", "")[:15]
    base = output_dir / f"consolidated-audit-{timestamp}"
    
    if "json" in formats or "all" in formats:
        path = base.with_suffix(".json")
        path.write_text(json.dumps(consolidated, indent=2), encoding="utf-8")
        paths["json"] = path
    
    if "markdown" in formats or "all" in formats:
        path = base.with_suffix(".md")
        path.write_text(render_markdown_summary(consolidated), encoding="utf-8")
        paths["markdown"] = path
    
    if "sarif" in formats or "all" in formats:
        path = base.with_suffix(".sarif")
        path.write_text(render_sarif(consolidated), encoding="utf-8")
        paths["sarif"] = path
    
    if "github-comment" in formats or "all" in formats:
        path = base.with_suffix(".github-comment.md")
        path.write_text(render_github_comment(consolidated), encoding="utf-8")
        paths["github-comment"] = path
    
    return paths
```

### 16.2 Markdown Executive Summary (the human format)

The Markdown companion exists because JSON is for machines and humans don't read it. Structure:

```markdown
# Audit Report — {project} ({timestamp})

## Executive Summary (for CTO / Tech Lead)

- **{critical_count} critical findings** ({new_count} new, {regressed_count} regressed) — fix top 3 this week
- **Overall coverage: {coverage_percent}%** ({coverage_warning if <30%})
- **Validation: {validation_sample_percent}% of findings independently verified** ({fp_count} FPs detected)
- **Top leverage:** Fix **{top_keystone_finding}** to unblock {transitive_count} downstream findings (~{savings}h saved)

## Top 5 Highest-Leverage Actions

| Rank | Finding | Severity | Effort | Unblocks | Why |
|------|---------|----------|--------|----------|-----|
| 1 | {id}: {title} | {sev} | {hours}h | {N} | {root_cause} |

## Critical Findings (full detail)

### {id}: {title}
- **File:** `{file}:{line}`
- **Severity:** critical (first seen {first_seen}, {aging_days} days open)
- **Root cause:** {root_cause}
- **Impact:** {impact}
- **Fix:** {fix_guide}
- **Verification:** `{verification_command}`
- **Validation:** {validation_status} — {validation_evidence}

[repeat for each critical]

## High Findings (full detail)
[...same format...]

## Medium Findings (full detail)
[...same format...]

## Low Findings (full detail)
[...same format...]

## False Positives Detected ({count})

| Original Finding | FP Category | Reason |
|------------------|-------------|--------|

## False Negatives / Coverage Gaps ({count})

| Area | Reason | Suggested Squad | Try This |
|------|--------|-----------------|----------|

## Delta vs Previous Audit

- New: {new_count}
- Regressed (severity escalated): {regressed_count}
- Improved (severity reduced): {improved_count}
- Fixed since last audit: {fixed_count} 🎉
- Aging (>90 days open): {aging_count}

## Squad Coverage Matrix

| Squad | Findings | Validated | FPs | FNs Reported | Files Examined |
|-------|----------|-----------|-----|--------------|----------------|

## Provenance
- Report version: {schema_version}
- Squad count: {squad_count}
- Validation sample: {sample_size}/{total} ({percent}%)
- Full JSON: `{json_path}`
```

### 16.3 SARIF Emitters (for GitHub Code Scanning / Azure DevOps)

SARIF (Static Analysis Results Interchange Format) is the OASIS standard for code-scanning results. GitHub uploads SARIF files via `github/codeql-action/upload-sarif@v3`.

```python
def render_sarif(consolidated: dict) -> str:
    """Render SARIF 2.1.0 for GitHub code scanning upload."""
    # Filter out FPs — only verified/partially-verified findings appear
    sarif_findings = []
    for f in consolidated["findings"]:
        if f.get("validation_status") in {"fp-confirmed", "fp-convention-override"}:
            continue
        sarif_findings.append({
            "ruleId": f"{f['category']}-{f['severity']}",
            "level": {"critical": "error", "high": "error", "medium": "warning", "low": "note"}[f["severity"]],
            "message": {
                "text": f"{f['title']}\n\nRoot cause: {f['root_cause']}\nFix: {f['fix_guide']}"
            },
            "locations": [{
                "physicalLocation": {
                    "artifactLocation": {"uri": f["file"]},
                    "region": {"startLine": f["line"], "endLine": f.get("endLine", f["line"])},
                }
            }],
            "partialFingerprints": {"primaryLocationLineHash": f.get("fingerprint", "")},
        })
    
    return json.dumps({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "audit-consolidated",
                    "version": "3.1.0",
                    "informationUri": "https://github.com/buff-lang/buff",
                }
            },
            "results": sarif_findings,
        }],
    }, indent=2)
```

### 16.4 GitHub PR Comment Format (terse, ≤6500 chars)

```markdown
## 🔍 Audit: {critical_count} critical / {high_count} high / {medium_count} medium / {low_count} low

**Coverage:** {coverage_percent}% • **Validation:** {validation_percent}% • **FPs filtered:** {fp_count} • **New since last audit:** {new_count}

### 🚨 Critical (top 3 of {critical_count})
1. **{id}** — {title} ({file}:{line}) — {effort_hours}h fix
2. **{id}** — {title} ({file}:{line}) — {effort_hours}h fix
3. **{id}** — {title} ({file}:{line}) — {effort_hours}h fix

### 💡 Highest-leverage fix
Fix **{keystone_id}** ({keystone_title}) to unblock **{N}** downstream findings (~{savings}h saved).

### 📊 Delta vs previous audit
- 🆕 {new_count} new • ⚠️ {regressed_count} regressed • ✅ {fixed_count} fixed

[Full report]({markdown_path}) • [JSON]({json_path}) • [SARIF]({sarif_path})
```

### 16.5 Emitter Discipline

1. **Markdown is the primary human deliverable** — JSON is for tooling; markdown is for decisions.
2. **SARIF filters out FPs** — only `verified` + `partially-verified` findings appear in SARIF.
3. **GitHub comment is terse** — ≤6500 chars (GitHub limit). Top 3 critical + 1 leverage action + delta.
4. **All emitters must agree on counts** — same `critical_count` in JSON, markdown, SARIF, GitHub comment.

---

## Section 17: Baseline Suppression & CI Gating (v3.1 — `--baseline` + `--fail-on`)

### 17.1 Baseline File Format

A baseline file lists fingerprints of ACCEPTED findings (the team has decided to live with them). Format:

```json
{
  "baseline_version": "1.0",
  "project": "buff",
  "created_at": "2026-07-26T16:00:00Z",
  "last_updated": "2026-08-15T10:00:00Z",
  "accepted_findings": [
    {
      "fingerprint": "a3f4b2c1d9e8f7a6",
      "original_id": "cq-007",
      "severity": "medium",
      "accepted_reason": "Legacy module scheduled for rewrite in Q4 2026 — see ADR-0042",
      "accepted_by": "tech-lead@example.com",
      "accepted_at": "2026-07-26T16:00:00Z",
      "expires_at": "2026-12-31T00:00:00Z",
      "adr_reference": ".sisyphus/decisions/accept-risk-cq-007.md"
    }
  ]
}
```

### 17.2 Suppression Procedure

When `--baseline path/to/baseline.json` is provided:

```python
def apply_baseline(findings: list[dict], baseline: dict, now: datetime) -> tuple[list[dict], list[dict]]:
    """Split findings into (active, suppressed). Suppressed findings still appear in the report
    but with status='suppressed' and don't count toward --fail-on."""
    accepted_by_fp = {f["fingerprint"]: f for f in baseline["accepted_findings"]}
    active, suppressed = [], []
    for f in findings:
        accepted = accepted_by_fp.get(f.get("fingerprint"))
        if accepted and (accepted.get("expires_at") is None or 
                        datetime.fromisoformat(accepted["expires_at"]) > now):
            f["suppressed"] = True
            f["suppression_reason"] = accepted["accepted_reason"]
            f["suppression_expires_at"] = accepted.get("expires_at")
            suppressed.append(f)
        else:
            active.append(f)
    return active, suppressed
```

### 17.3 CI Gating (`--fail-on`)

```python
def evaluate_fail_on(active_findings: list[dict], threshold: str) -> int:
    """Return exit code: 0 = pass, 1 = fail."""
    sev_rank = {"low": 0, "medium": 1, "high": 2, "critical": 3}
    threshold_rank = sev_rank[threshold]
    failures = [f for f in active_findings if sev_rank[f["severity"]] >= threshold_rank]
    if failures:
        print(f"Audit FAILED: {len(failures)} findings at or above {threshold} severity")
        for f in failures[:10]:
            print(f"  - {f['id']}: {f['title']} ({f['file']}:{f['line']})")
        return 1
    return 0
```

### 17.4 CI Discipline Rules

1. **Suppressed findings still appear in the report** — they're tagged `suppressed: true` with the reason + expiry. Visibility, not erasure.
2. **Baseline entries MUST have an expiry** — accepted-risk decisions aren't permanent. Default 90 days; max 1 year. Lapsed entries auto-reactivate.
3. **Baseline entries MUST cite an ADR** — accept-risk decisions need documentation.
4. **`--fail-on critical` is the recommended CI gate** — surfaces real risks without drowning in low-severity noise.
5. **FPs never count toward `--fail-on`** — they're filtered before evaluation.
6. **New findings (status='new') at severity ≥ threshold** trigger a non-zero exit even if the same number existed last run — the point is to prevent NEW criticals from sneaking in.

---

## Section 18: Resilience Protocol (v3.2 MANDATORY — Disk-Level Persistence)

**Goal**: Make audit robust to orchestrator death at ANY phase. Squad findings + validation + consolidation state all persist on disk. Failed run = 5-10 min resume, not 45-min restart.

On-disk state shape defined here. Resume logic in §19. Squad-side persistence in `_shared/audit-template.md` §14. Chunked writes in `_shared/audit-template.md` §15.

### 18.1 On-Disk State Layout

For each audit run with id `{run-id}` (ISO timestamp `YYYYMMDD-HHMM`):

```
.audit/
└── in-progress/
    └── {run-id}/                          # one directory per run
        ├── checkpoint.json                # phase tracker (THIS section)
        ├── squads/                        # one file per squad (template Section 14)
        │   ├── audit-code-quality.json
        │   ├── audit-security.json
        │   ├── audit-architecture.json
        │   └── ... (13 files for full audit)
        ├── validation.json                # validation results (Step 7.5 output)
        ├── consolidated.partial.json.tmp  # incrementally-built final JSON (template Section 15)
        ├── consolidated.partial.json      # latest complete-as-of snapshot (renamed from .tmp)
        └── run.log                        # human-readable progress log (optional)
```

After successful run completion, the orchestrator moves the run directory to `.audit/completed/{run-id}/` (configurable, default keep 7 days then auto-purge).

### 18.2 Checkpoint File Schema (MANDATORY — atomic rewrite after each phase)

`.audit/in-progress/{run-id}/checkpoint.json`:

```json
{
  "run_id": "2026-07-26-2200",
  "schema_version": "3.2",
  "started_at": "2026-07-26T22:00:00Z",
  "last_update": "2026-07-26T22:14:32Z",
  "phase": "validation",
  "phase_started_at": "2026-07-26T22:12:00Z",
  "previous_phase": "squad-execution",
  "config": {
    "argument": "all",
    "previous_report": ".sisyphus/reports/buff-audit-2026-07-26-1514.json",
    "baseline": null,
    "fail_on": null,
    "output_formats": ["json", "markdown", "sarif", "github-comment"]
  },
  "squad_groups": {
    "1": { "category": "Code", "status": "complete", "squads": ["code-quality", "refactoring", "test", "test-coverage", "dependencies"] },
    "2": { "category": "Architecture", "status": "complete", "squads": ["architecture", "security", "fault-tolerance", "observability"] },
    "3": { "category": "Business", "status": "skipped-na", "squads": [] },
    "4": { "category": "Compliance", "status": "skipped-na", "squads": [] },
    "5": { "category": "Regulatory", "status": "skipped-na", "squads": [] },
    "6": { "category": "Meta", "status": "complete", "squads": ["consistency-coherence", "lmas-traceability", "cicd"] }
  },
  "completed_squads": ["code-quality", "refactoring", "test", "test-coverage", "dependencies", "architecture", "security", "fault-tolerance", "observability", "consistency-coherence", "lmas-traceability", "cicd"],
  "missing_squads": ["product-roadmap", "license"],
  "squad_files_written": [
    ".audit/in-progress/2026-07-26-2200/squads/audit-code-quality.json",
    "... (13 paths)"
  ],
  "squad_files_missing": [],
  "findings_collected_count": 77,
  "findings_collected_by_severity": { "critical": 16, "high": 27, "medium": 22, "low": 12 },
  "validation": {
    "started_at": "2026-07-26T22:12:00Z",
    "sample_size_planned": 20,
    "sample_size_actual": 19,
    "sample_percent": 24.7,
    "validated_finding_ids": ["cc-001", "cc-002", "cc-003", "..."],
    "results": { "verified": 17, "fp-confirmed": 1, "fp-convention-override": 1, "partially-verified": 1, "unverified": 0 }
  },
  "consolidation": {
    "started_at": null,
    "fingerprints_computed": false,
    "delta_computed": false,
    "dag_computed": false,
    "coverage_map_computed": false,
    "false_positives_count": 0,
    "false_negatives_count": 0
  },
  "emit": {
    "started_at": null,
    "formats_emitted": [],
    "final_paths": {}
  },
  "failures": [
    { "phase": "squad-execution", "squad": "test-coverage", "attempt": 1, "reason": "cert error", "retried": true, "recovered": true }
  ],
  "resume_count": 0
}
```

### 18.3 Phase Update Rules (orchestrator MUST update checkpoint atomically after each transition)

The orchestrator writes the checkpoint file:
1. **At run start** (phase=`squad-execution`, started_at=now, all groups `pending`).
2. **After each squad completes** (update `completed_squads`, `squad_files_written`, `findings_collected_*`).
3. **After each squad group completes** (update `squad_groups[N].status`).
4. **At validation start** (phase=`validation`, validation.started_at=now).
5. **After each finding is validated** (append to `validation.validated_finding_ids`, update `validation.results`).
6. **At consolidation start** (phase=`consolidation`, consolidation.started_at=now).
7. **After each consolidation sub-step** (fingerprints, delta, DAG, coverage_map — flip the corresponding boolean).
8. **At emit start** (phase=`emit`, emit.started_at=now).
9. **After each format is emitted** (append to `formats_emitted`, set `final_paths[format]`).
10. **At run completion** (phase=`complete`, move run dir to `.audit/completed/`).

**Atomic rewrite**: write to `checkpoint.json.tmp`, rename to `checkpoint.json`. Never leave checkpoint in inconsistent state.

### 18.4 Incremental Consolidated JSON Emit (template Section 15 applied)

The orchestrator uses `.audit/in-progress/{run-id}/consolidated.partial.json.tmp` as the in-progress file:

```
After all squads complete (phase=squad-execution → validation):
  - Write .tmp with header + empty findings[]

After each squad group's findings are validated (during validation phase):
  - Append that group's validated findings to .tmp findings[]

After validation completes (phase=consolidation):
  - Compute fingerprints, append fields to each finding
  - Compute delta block, append
  - Compute DAG block, append
  - Compute coverage_map block, append
  - Compute false_positives, false_negatives, append

After consolidation completes (phase=emit):
  - Verify .tmp is valid JSON with all expected blocks
  - Atomic rename .tmp → .audit/in-progress/{run-id}/consolidated.partial.json
  - For each requested output format:
    - Read .partial.json
    - Render format-specific output (markdown, sarif, github-comment)
    - Write each as {format}.tmp → atomic rename → {format}
  - Copy/rename final files to .sisyphus/reports/buff-audit-{run-id}-v3.{ext}
```

### 18.5 Anti-Stall Discipline (Cross-Reference template Section 15)

1. **No single write call exceeds 50 KB.** Break consolidated JSON into chunks of 10-20 findings per `edit`.
2. **Always `.tmp` first, atomic rename at end.** Final filename only when complete + validated.
3. **Verify after each chunk** — quick Read + JSON parse.
4. **`compress` between chunks OK** if context tight. NEVER compromise findings[] completeness.
5. **Orchestrator killed mid-emit**: `.tmp` survives. Resume reads `.tmp`, validates to truncation, continues.

### 18.6 Recovery Scenario Walkthrough

Original run: phase=`validation`, 13/13 squads persisted to disk. Orchestrator dies after validating 12/19 sampled findings.

**Without v3.2**: Total loss. Re-run from scratch = 45 min.

**With v3.2**:
1. User (or auto-wrapper) invokes `audit resume 2026-07-26-2200`.
2. Resume reads checkpoint → phase=`validation`, 12/19 validated.
3. Resume reads `squads/*.json` → 13 squad files exist, valid.
4. Resume launches new orchestrator scoped to: "read checkpoint, validate remaining 7 findings, proceed to consolidation + emit".
5. Wall-clock: ~10 min instead of 45.

12 already-validated findings not re-done. 13 squad outputs not re-generated. Only missing work done.

---

## Section 19: Resume Protocol (v3.2 — `audit resume` subcommand)

**Goal**: Failed/incomplete audit → fast continuation. Resume reads on-disk state from §18, launches ONLY incomplete work.

### 19.1 Resume Invocation

```bash
audit resume                              # resume most recent in-progress run
audit resume 2026-07-26-2200              # resume specific run by ID
audit resume --dry-run                    # show what would be re-run, then exit
audit resume --force-fresh                # ignore in-progress state, start over
```

### 19.2 Resume Detection (Step 0 of `/audit`)

When `audit` (any subcommand) starts, the orchestrator MUST:

1. Scan `.audit/in-progress/*/checkpoint.json` for existing runs.
2. If any exist:
   - Sort by `started_at` descending.
   - For the most recent, summarize: `{run_id}, phase={phase}, N/M squads complete, K/J findings validated`.
   - Prompt the user: "Found in-progress audit run: {summary}. Resume, start fresh, or list all in-progress runs?"
3. If `audit resume` was explicitly invoked, skip the prompt and proceed to Section 19.3.

### 19.3 Resume Phase Logic

Read checkpoint. Branch on `phase`:

#### Case A: `phase=squad-execution` (orchestrator died during squad dispatch/collection)

1. List `squad_files_written` vs expected squads for the configured scope.
2. Identify `missing_squads` — those without a file on disk.
3. Re-launch ONLY `missing_squads` (parallel, with the v3.2 squad invocation that writes to disk per template Section 14).
4. After they complete, verify all expected squad files exist + are valid JSON.
5. Proceed to validation phase.

#### Case B: `phase=validation` (squads done, validation was interrupted)

1. Verify all squad files exist on disk (re-launch any missing per Case A).
2. Read `validation.validated_finding_ids` from checkpoint.
3. Read all findings from squad files.
4. Validate findings NOT in `validated_finding_ids` until the sample quota is met (≥20% overall, 100% of criticals, 100% of suspect queue).
5. Merge new validation results into existing `validation` block.
6. Proceed to consolidation phase.

#### Case C: `phase=consolidation` (validation done, consolidation interrupted)

1. Verify all squad files exist.
2. Read validation results from checkpoint (or re-run validation if checkpoint's `validated_finding_ids` is empty).
3. Read findings from squad files, apply validation results.
4. Re-compute any unfinished consolidation sub-steps (fingerprints, delta, DAG, coverage_map — checkpoint tracks which are done).
5. Write/update `consolidated.partial.json` with all blocks.
6. Proceed to emit phase.

#### Case D: `phase=emit` (consolidation done, emission interrupted)

1. Verify `consolidated.partial.json` exists and is valid.
2. Read `emit.formats_emitted` from checkpoint.
3. Emit ONLY the missing formats (e.g., if JSON + Markdown done but SARIF + GitHub comment missing, emit only those two).
4. Verify all requested formats are present + agree on counts.
5. Move run dir to `.audit/completed/{run-id}/`.
6. Set phase=`complete`.

#### Case E: `phase=complete` (nothing to do)

Report "Run {run_id} already complete. Final outputs at: {paths}."

#### Case F: Missing/invalid checkpoint

Report "Checkpoint for {run_id} is missing or invalid. Squad files may still be usable — invoke `audit rebuild {run_id}` to consolidate from squad files only."

### 19.4 Auto-Resume Wrapper (commands/audit.md v3.2 Step 12)

The `/audit` command wraps every orchestrator launch with auto-retry:

```python
def run_audit_with_resilience(args):
    run_id = generate_run_id()
    write_initial_checkpoint(run_id, args)
    
    max_retries = 2
    for attempt in range(max_retries + 1):
        try:
            launch_orchestrator_task(args, run_id)
            return  # success
        except TaskTimeout, TaskCancelled:
            if attempt < max_retries:
                log(f"Orchestrator died (attempt {attempt+1}). Auto-resuming in 5s...")
                sleep(5)
                # Resume reads checkpoint to determine what's left
                args.resume_run_id = run_id
                # Re-invoke with resume mode
                continue
            else:
                log(f"Orchestrator died {max_retries+1} times. Manual recovery:")
                log(f"  audit resume {run_id}")
                return 1
```

The user never sees the retry unless both attempts fail. The retry is silent and fast.

### 19.5 Resume Discipline Rules

1. **Never re-run completed work.** Checkpoint exists to skip completed phases.
2. **Validate squad files before trusting.** Exists but invalid JSON = missing → re-launch squad.
3. **Idempotent squad writes.** Re-launched squad overwrites own file (no `.v2.json`).
4. **Resume count tracked.** Checkpoint `resume_count` increments per resume. >3 → warning: "Frequent resumes suggest systemic issue (timeout too low, context exhaustion, tool failure)."
5. **Final completion moves run dir.** `.audit/in-progress/{run-id}/` → `.audit/completed/{run-id}/`. Preserved for inspection.
6. **Old in-progress runs are stale.** `last_update` >7 days → prompt user: resume, abandon, or force-complete.

### 19.6 Cleanup

- `.audit/in-progress/` NOT gitignored by default (visible for debugging).
- `.audit/completed/` gitignored after 7 days (configurable).
- `.gitignore` entry `.audit/in-progress/` recommended for committed repos.
- `audit cleanup` subcommand (future) removes runs older than threshold.