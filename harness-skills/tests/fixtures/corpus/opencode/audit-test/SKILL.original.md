---
name: audit-test
description: >-
  Infrastructure pipeline validation: agent and skill smoke tests, end-to-end pipeline
  verification, output format compliance, and error handling validation for the
  audit system itself. NOT a code testing squad — validates the audit infrastructure.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "2.0.0"
---

# Audit Test Squad — Infrastructure Validation

Validates the audit agent+skill infrastructure itself. Runs smoke tests to verify agents
invoke correctly, skills load properly, outputs conform to schemas, and the pipeline
handles errors gracefully. NOT for testing application code.

---

## Section 1: USE FOR / DO NOT USE FOR

### USE FOR

- Agent invocation verification (can the agent be instantiated and execute?)
- Skill loading validation (does the skill file parse and register correctly?)
- Output format compliance (do squad outputs match the v2.0 schema?)
- Pipeline end-to-end smoke tests (does a full audit run complete without errors?)
- Error handling validation (does the pipeline handle malformed inputs gracefully?)
- Regression testing after infrastructure changes (new skills, config changes)

### DO NOT USE FOR

- Production audit runs (use specialized squads)
- Security analysis of application code (use `audit-security`)
- Architecture review of application code (use `audit-architecture`)
- Testing application business logic (use `audit-test-coverage`)

---

## Section 2: Target Discovery

The agent MUST auto-detect the audit infrastructure layout. Do NOT hardcode paths.

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

### 2.2 Audit Infrastructure Detection

| Signal | File Pattern | Component |
|--------|-------------|-----------|
| Skill definitions | `**/SKILL.md` files | Skill registry |
| Agent configs | `**/*agent*.json`, `**/*agent*.yaml` | Agent configuration |
| Audit config | `audit.config.json`, `.auditrc` | Audit settings |
| Output directory | `AUDIT_OUTPUT_DIR` env var or `.audit/reports/` | Report storage |
| Orchestrator scripts | `audit-*.sh`, `audit-*.py`, `run-audit.*` | Pipeline entry points |

### 2.3 Discovery Procedure

1. **Scan root directory** for all language and infrastructure patterns above.
2. **Locate skill definitions**: Scan for `SKILL.md` files in configured skill directories.
3. **Resolve output directory**: Check `AUDIT_OUTPUT_DIR` env var, then config files, then default.
4. **Validate infrastructure completeness**: All expected components present?

---

## Section 3: Technology-Specific Checklists

### 3.1 Agent Invocation Verification

- [ ] **Agent instantiation**: Agent can be created without errors
- [ ] **Skill discovery**: All registered skills are discoverable by the agent
- [ ] **Skill parsing**: Each SKILL.md file parses without YAML/markdown errors
- [ ] **Tool availability**: Required MCP tools are reachable and responsive
- [ ] **Context injection**: Agent receives correct project context (languages, frameworks)
- [ ] **Timeout handling**: Agent respects configured timeouts

### 3.2 Skill Loading Validation

- [ ] **Frontmatter valid**: YAML frontmatter parses, has required fields (`name`, `description`, `version`)
- [ ] **Sections present**: All 9 template sections exist in the skill file
- [ ] **No hardcoded repos**: Skill file contains no hardcoded repository names or paths
- [ ] **Cross-references valid**: Squad names in integration notes match actual squads
- [ ] **Version consistency**: All skills using template v2.0 have `version: "2.0.0"`

### 3.3 Output Format Compliance

- [ ] **JSON valid**: Squad output is valid JSON
- [ ] **Required fields present**: `report_version`, `squad`, `project`, `timestamp`, `findings`, `summary`
- [ ] **Severity values valid**: Only `critical`, `high`, `medium`, `low`
- [ ] **File paths relative**: No absolute paths in `file` fields
- [ ] **Summary counts accurate**: `totalFindings` matches actual findings array length

### 3.4 Pipeline End-to-End Tests

- [ ] **Full run completes**: Minimal audit run (1 file, 1 squad) completes without errors
- [ ] **Output written**: Report file created in configured output directory
- [ ] **Consolidation works**: `audit-consolidated` can merge multiple squad outputs
- [ ] **Error recovery**: Pipeline continues after single squad failure
- [ ] **Cleanup**: Temporary files cleaned up after run

### 3.5 Error Handling in Audit Pipeline

- [ ] **Malformed input**: Pipeline handles invalid JSON gracefully (logs error, continues)
- [ ] **Missing skill**: Pipeline reports missing skill clearly (doesn't silently skip)
- [ ] **Agent crash**: Pipeline catches agent exceptions and logs with context
- [ ] **Timeout exceeded**: Pipeline enforces timeouts and reports timed-out squads

---

## Section 4: Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Silent error swallowing | critical | Pipeline catches exceptions and continues without logging | Findings silently lost, false sense of security | Log every error with context, include in `squads_with_errors` |
| 2 | Skills not loading | high | Skill file exists but isn't discovered by the agent | Entire squad audit missing from report | Validate skill discovery at pipeline startup, fail fast |
| 3 | Hardcoded test expectations | medium | Smoke tests check for specific repo names or file paths | Tests break when run against different projects | Use dynamic project detection, parametrize fixtures |
| 4 | No output validation | high | Pipeline produces output but never checks schema compliance | Malformed reports break downstream consumers | Validate every report against v2.0 schema before accepting |
| 5 | Stale skill cache | medium | Agent uses cached skill definition after file is updated | New checks not applied in audit runs | Invalidate skill cache on file change, or always re-read |
| 6 | Single-point-of-failure pipeline | medium | One squad crash stops entire audit pipeline | Partial results lost, no report generated | Isolate squad execution, collect partial results |

---

## Section 5: Code Example — Pipeline Smoke Test

```python
import json
from pathlib import Path
from datetime import datetime, timezone


def run_smoke_test(skill_dir: Path, output_dir: Path) -> list[dict]:
    """Run a minimal end-to-end pipeline smoke test."""
    results = []

    # Test 1: Skill file parsing
    for skill_file in skill_dir.rglob("SKILL.md"):
        result = _validate_skill_file(skill_file)
        results.append(result)

    # Test 2: Output format compliance
    output_dir.mkdir(parents=True, exist_ok=True)
    sample = output_dir / "audit-test-smoke.json"
    sample.write_text(json.dumps({
        "report_version": "2.0",
        "squad": "audit-test",
        "project": "smoke-test",
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "findings": results,
        "summary": {
            "totalFindings": len(results),
            "bySeverity": _count_severities(results),
            "filesScanned": 0,
            "filesWithFindings": 0,
        },
    }, indent=2))

    parsed = json.loads(sample.read_text())
    required = {"report_version", "squad", "project", "timestamp", "findings", "summary"}
    missing = required - parsed.keys()
    results.append({
        "severity": "high",
        "file": "infrastructure:smoke-test",
        "description": f"Schema compliance: {'PASS' if not missing else f'Missing: {missing}'}",
        "recommendation": "Fix missing required fields" if missing else "No action needed",
        "technology": "CrossLanguage",
        "category": "maintainability",
    })

    return results


def _validate_skill_file(path: Path) -> dict:
    """Validate a SKILL.md file has correct frontmatter and sections."""
    content = path.read_text(encoding="utf-8")
    errors = []

    if not content.startswith("---"):
        errors.append("Missing YAML frontmatter")
    else:
        end = content.find("---", 3)
        if end == -1:
            errors.append("Unclosed YAML frontmatter")
        else:
            frontmatter = content[3:end].strip()
            for field in ["name:", "description:", "version:"]:
                if field not in frontmatter:
                    errors.append(f"Missing frontmatter field: {field.rstrip(':')}")

    return {
        "severity": "medium",
        "file": str(path.relative_to(path.parents[2])),
        "description": f"Skill validation: {'PASS' if not errors else '; '.join(errors)}",
        "recommendation": "Fix errors above" if errors else "No action needed",
        "technology": "CrossLanguage",
        "category": "maintainability",
    }


def _count_severities(findings: list[dict]) -> dict:
    counts = {"critical": 0, "high": 0, "medium": 0, "low": 0}
    for f in findings:
        sev = f.get("severity", "low")
        if sev in counts:
            counts[sev] += 1
    return counts
```

---

## Section 6: Output Format

Uses the standard finding schema from the shared template. Smoke test results structured as:

```json
{
  "severity": "high",
  "file": "skills/audit-security/SKILL.md",
  "line": 1,
  "description": "Skill file missing required frontmatter field: version",
  "recommendation": "Add 'version: \"2.0.0\"' to YAML frontmatter",
  "technology": "CrossLanguage",
  "category": "maintainability"
}
```

### 6.1 Smoke Test Summary

```json
{
  "summary": {
    "squad": "audit-test",
    "timestamp": "2026-06-04T10:00:00Z",
    "project": "infrastructure-validation",
    "technologies": ["CrossLanguage"],
    "totalFindings": 3,
    "bySeverity": { "critical": 0, "high": 1, "medium": 2, "low": 0 },
    "tests_run": {
      "skill_parsing": { "total": 25, "passed": 24, "failed": 1 },
      "output_compliance": { "total": 1, "passed": 1, "failed": 0 }
    }
  }
}
```

---

## Section 7: Severity Guidelines

Uses the standard severity definitions from the shared template with infrastructure-specific adjustments:

| Finding | Severity | Reason |
|---------|----------|--------|
| Pipeline silently swallows errors | critical | Findings lost without trace |
| Skill not discovered by agent | high | Entire squad audit missing from report |
| Output schema violation | high | Breaks downstream consumers |
| Hardcoded repo name in skill | medium | Not portable across projects |
| Missing frontmatter field | medium | Skill may not load correctly |
| Stale skill cache | low | Eventually consistent, not data loss |

---

## Section 8: Integration Notes

### 8.1 Squad Dependency Graph

```
audit-test (standalone — validates infrastructure, not code)
    ├── Tests: skill loading (reads all SKILL.md files)
    ├── Tests: agent invocation (exercises agent runtime)
    ├── Tests: output compliance (validates report schemas)
    └── Tests: pipeline end-to-end (exercises full audit flow)
```

### 8.2 Upstream Dependencies

This squad has NO upstream dependencies. It reads:
- Skill definition files (`SKILL.md`)
- Agent configuration files
- Audit output directory (for compliance checks)

### 8.3 Downstream Consumers

- CI/CD pipelines — smoke test results gate deployment of infrastructure changes
- Infrastructure maintainers — use results to debug skill/agent issues
- `audit-consolidated` — smoke test reports can be aggregated alongside normal audit reports

### 8.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-test | audit-test-coverage | audit-test validates infrastructure; test-coverage validates application test quality |
| audit-test | audit-consolidated | audit-test validates report schemas; consolidated produces reports |
| audit-test | audit-lmas-traceability | audit-test validates pipeline execution; traceability tracks agent spans |

---

## Section 9: Execution Protocol

### 9.1 Recommended Execution Order

1. **Before** any other squad — pre-flight check to verify infrastructure is working
2. **After** infrastructure changes — to verify nothing is broken
3. **In CI/CD** — as a gate for infrastructure deployments

### 9.2 Execution Checklist

- [ ] Verify all skill files parse correctly
- [ ] Verify agent can be invoked
- [ ] Validate output report schema compliance
- [ ] Check error handling with malformed inputs
- [ ] Verify output directory is writable
- [ ] Report all infrastructure findings using standard schema

### 9.3 Output Integration

When running alongside other squads, this squad SHOULD:
1. Run before all other squads as a pre-flight check.
2. Produce smoke test findings in the standard Section 7 output format.
3. Emit findings to the shared output directory for `audit-consolidated` aggregation.
4. Skip deduplication — infrastructure findings are unique to this squad.
