---
name: audit-consolidated
description: >-
  Consolidated audit report aggregation: standard output format for all squads,
  deduplication, severity resolution, and configurable report generation.
  Language-agnostic output layer that consumes findings from all other audit squads.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "2.0.0"
---

# Consolidated Audit Report Squad

Aggregation layer. Consumes structured findings from all other audit squads, deduplicates,
resolves severity conflicts, and produces unified reports.

---

## Section 1: USE FOR / DO NOT USE FOR

### USE FOR

- Aggregating findings from multiple audit squads into a single report
- Deduplicating overlapping findings across squads
- Generating summary statistics and severity breakdowns
- Validating that all squad outputs conform to the expected schema
- Cross-referencing findings to identify systemic hotspots
- Report quality validation: provenance, naming, consistency checks

### DO NOT USE FOR

- Performing code-level audits (use domain-specific squads)
- Security scanning (use `audit-security`)
- Architecture review (use `audit-architecture`)
- Financial model validation (use `audit-financial-model`)

---

## Section 2: Target Discovery

The agent MUST auto-detect the project structure. Do NOT hardcode repository names or paths.

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
| `AUDIT_OUTPUT_DIR` environment variable | 1 (highest) | `os.environ["AUDIT_OUTPUT_DIR"]` |
| `audit.config.json` in project root | 2 | Parse JSON, read `outputDir` key |
| `.auditrc` or `.auditrc.json` in project root | 3 | Parse as JSON or key=value |
| Default: `<project_root>/.audit/reports/` | 4 (fallback) | Construct from detected project root |

### 2.3 Discovery Procedure

1. **Detect project root**: Look for `.git/`, solution files, `go.mod`, `package.json`, or `Cargo.toml`.
2. **Identify primary language** by file count.
3. **Resolve output directory**: Check config sources in priority order.
4. **Collect squad outputs**: Scan output directory for files matching `audit-*.json`.
5. **Validate schema**: Each file must conform to the per-squad report schema.
6. **Proceed with aggregation**.

---

## Section 3: Standard Output Format

### 3.1 Per-Squad Report Schema

Every squad produces findings in this format:

```json
{
  "report_version": "2.0",
  "squad": "audit-code-quality",
  "project": "detected-project-name",
  "timestamp": "2026-06-04T14:00:00Z",
  "technologies": [".NET/C#", "Python"],
  "findings": [
    {
      "severity": "high",
      "file": "src/Modules/Billing/Calculator.cs",
      "line": 45,
      "endLine": 78,
      "description": "Function exceeds cyclomatic complexity threshold of 10",
      "recommendation": "Extract helper function for the rate calculation logic",
      "technology": ".NET",
      "category": "maintainability"
    }
  ],
  "summary": {
    "totalFindings": 15,
    "bySeverity": { "critical": 0, "high": 2, "medium": 8, "low": 5 },
    "byCategory": { "security": 3, "maintainability": 7, "performance": 5 },
    "filesScanned": 42,
    "filesWithFindings": 12
  }
}
```

### 3.2 Schema Validation Rules

- `report_version`: Must be `"2.0"`. Reject older formats with a warning.
- `squad`: Must start with `audit-`. Used for deduplication grouping.
- `findings[]`: Each finding MUST have: severity, file, line, description, recommendation, technology, category.
- `summary.bySeverity`: Keys must be exactly `critical`, `high`, `medium`, `low`.
- Reject reports with missing required fields. Log the validation error.

---

## Section 4: Aggregation Rules

### 4.1 Step 1 — Collect Individual Reports

Gather all squad reports from the configured output directory.
Filter by: project name match, valid schema, recent timestamp (within audit window).

### 4.2 Step 2 — Merge Findings

Combine all findings arrays into a single list. Sort by:
1. Severity (critical → high → medium → low)
2. File path (alphabetical)
3. Line number (ascending)

### 4.3 Step 3 — Deduplicate

If multiple squads flag the same file + line range with overlapping findings:
- Keep the **highest-severity** version
- Add a `corroborating_squads` field listing all squads that found it
- Merge descriptions if they provide different perspectives

**Deduplication heuristic**: Two findings overlap if:
- Same `file` AND
- Line ranges overlap AND
- Same `category` OR same root cause in description

### 4.4 Step 4 — Generate Consolidated Report

```json
{
  "report_version": "2.0",
  "type": "consolidated",
  "project": "detected-project-name",
  "timestamp": "2026-06-04T15:00:00Z",
  "technologies": [".NET/C#", "Python"],
  "squads_run": ["audit-code-quality", "audit-security"],
  "squads_missing": ["audit-dependencies"],
  "total_findings": 26,
  "severity_breakdown": { "critical": 1, "high": 5, "medium": 12, "low": 8 },
  "top_files": [
    {
      "file": "src/Modules/Billing/Calculator.cs",
      "finding_count": 4,
      "squads": ["audit-code-quality", "audit-security"],
      "highest_severity": "high"
    }
  ],
  "findings": ["...merged and deduplicated..."],
  "provenance": {
    "generated_by": "audit-consolidated",
    "schema_version": "2.0",
    "source_reports": ["audit-code-quality-20260604.json", "audit-security-20260604.json"]
  }
}
```

---

## Section 5: Report Quality Checklist

Before finalizing the consolidated report:

- [ ] **Output format consistency**: All squad reports conform to v2.0 schema
- [ ] **Aggregation rules applied**: Deduplication, severity merging, corroborating_squads populated
- [ ] **Summary statistics accuracy**: `total_findings` matches sum of severity breakdown
- [ ] **Cross-reference validation**: `top_files` entries match actual findings
- [ ] **Report naming conventions**: Files named `consolidated-audit-{date}.json`
- [ ] **Provenance metadata**: `source_reports` lists all consumed squad reports
- [ ] **Missing squads flagged**: `squads_missing` lists expected squads without output
- [ ] **File paths relative**: All paths relative to project root, forward slashes

---

## Section 6: Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded output paths | high | Output path hardcoded to specific repo directory | Not portable, breaks in CI/CD | Use configurable path from env var or config file |
| 2 | Missing provenance metadata | medium | Consolidated report has no `source_reports` | Cannot trace findings back to source squads | Always include `provenance` object with full source list |
| 3 | Inconsistent severity counts | high | `total_findings` doesn't match sum of severity breakdown | Undermines report credibility | Validate counts before emitting report |
| 4 | Silently dropping squad outputs | critical | Squad report fails validation but error is swallowed | Missing findings create false sense of security | Log validation errors, include in `squads_missing` with reason |
| 5 | Duplicate findings not merged | medium | Same issue reported by multiple squads appears multiple times | Inflates finding count, confuses reviewers | Apply deduplication with `corroborating_squads` field |

---

## Section 7: Code Example — Report Aggregation

```python
import json
from pathlib import Path
from collections import defaultdict
from datetime import datetime, timezone


def aggregate_reports(output_dir: Path, project_name: str) -> dict:
    """Collect, validate, deduplicate, and merge squad reports."""
    all_findings = []
    squads_run = []
    squads_with_errors = []

    for report_file in sorted(output_dir.glob("audit-*.json")):
        try:
            report = json.loads(report_file.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, UnicodeDecodeError) as e:
            squads_with_errors.append({"file": report_file.name, "error": str(e)})
            continue

        required_keys = {"report_version", "squad", "findings", "summary"}
        if not required_keys.issubset(report.keys()):
            squads_with_errors.append({
                "file": report_file.name,
                "error": f"Missing keys: {required_keys - report.keys()}",
            })
            continue

        squads_run.append(report["squad"])
        all_findings.extend(report.get("findings", []))

    # Deduplicate and sort
    deduplicated = _deduplicate_findings(all_findings)
    severity_order = {"critical": 0, "high": 1, "medium": 2, "low": 3}
    deduplicated.sort(key=lambda f: (
        severity_order.get(f["severity"], 99), f.get("file", ""),
    ))

    by_severity = defaultdict(int)
    for finding in deduplicated:
        by_severity[finding["severity"]] += 1

    return {
        "report_version": "2.0",
        "type": "consolidated",
        "project": project_name,
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "squads_run": squads_run,
        "squads_with_errors": squads_with_errors,
        "total_findings": len(deduplicated),
        "severity_breakdown": dict(by_severity),
        "findings": deduplicated,
        "provenance": {
            "generated_by": "audit-consolidated",
            "schema_version": "2.0",
            "source_reports": [f.name for f in sorted(output_dir.glob("audit-*.json"))],
        },
    }


def _deduplicate_findings(findings: list[dict]) -> list[dict]:
    """Merge overlapping findings and track corroborating squads."""
    groups: dict[str, list[dict]] = defaultdict(list)
    for f in findings:
        key = f"{f.get('file', '')}:{f.get('line', 0)}:{f.get('category', '')}"
        groups[key].append(f)

    result = []
    severity_rank = {"critical": 0, "high": 1, "medium": 2, "low": 3}
    for group in groups.values():
        best = min(group, key=lambda f: severity_rank.get(f["severity"], 99))
        squads = list({f.get("_source_squad", "unknown") for f in group})
        if len(squads) > 1:
            best["corroborating_squads"] = squads
        result.append(best)

    return result
```

---

## Section 8: Output Format

Uses the standard finding schema. Additionally:

### 8.1 Consolidated Report File Naming

```
{output_dir}/consolidated-audit-{YYYYMMDD}.json
{output_dir}/consolidated-audit-{YYYYMMDD}-{HHMMSS}.json  (if multiple runs per day)
```

### 8.2 Output Path Resolution (Priority Order)

1. `AUDIT_OUTPUT_DIR` environment variable
2. `audit.config.json` → `outputDir` key
3. Default: `<project_root>/.audit/reports/`

---

## Section 9: Integration Notes

### 9.1 Squad Dependency Graph

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
audit-consolidated ◄────────────────┘  (consumes ALL squad outputs)
audit-consistency-coherence            (consumes consolidated + squad outputs)
```

### 9.2 Upstream Dependencies

This squad **consumes** outputs from ALL other audit squads:
- Every squad that produces `audit-*.json` files in the output directory
- Validates each report against the v2.0 schema before aggregation

### 9.3 Downstream Consumers

- `audit-consistency-coherence` — consumes consolidated report for cross-domain validation
- Human reviewers — read consolidated reports for sign-off
- CI/CD pipelines — may gate on consolidated severity counts

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-consolidated | audit-consistency-coherence | Consolidated aggregates findings; consistency validates cross-domain alignment |
| audit-consolidated | audit-lmas-traceability | Consolidated aggregates findings; traceability tracks agent spans |

### 9.5 Recommended Execution Order

1. All domain-specific squads run first
2. `audit-consolidated` aggregates all findings
3. `audit-consistency-coherence` validates cross-domain alignment (LAST)
