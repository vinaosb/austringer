---
name: audit-lmas-traceability
description: >-
  LMAS traceability auditing: agent behavior monitoring, cost auditing per agent,
  inter-squad task tracking, evidence verification, span validation, provenance
  metadata on reports, and audit trail persistence.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "2.0.0"
---

# LMAS Traceability Audit

## USE FOR

- Agent audit trail persistence — verify admin/agent actions logged to durable storage, not just ephemeral structured logs
- Evidence file completeness — verify audit evidence files exist, are non-empty, and contain valid structured results
- Span validation — verify audit spans cover all required areas for complete traceability
- Cost auditing per agent — track token consumption per squad per run, verify cost attribution
- Inter-squad task tracking — verify task handoff completeness, no orphan findings between squads
- Provenance metadata on reports — verify audit reports include agent ID, timestamp, input hash, model version
- Reproducibility check — verify same input produces consistent output across runs
- Audit log durability — verify logs are persisted to database or durable storage, not only to console/structured output
- Finding deduplication — verify cross-squad findings are properly deduplicated with provenance chain

### DO NOT USE FOR

- Observability infrastructure setup (use `audit-observability`)
- Test quality assessment (use `audit-test-coverage`)
- Code security scanning (use `audit-security`)
- Performance profiling (use `audit-code-quality`)

---

## Target Discovery

The agent MUST auto-detect the project structure. Do NOT hardcode repository names.

### 3.1 Language Detection

| Signal | File Pattern | Detected Language |
|--------|-------------|-------------------|
| C# source files | `*.cs`, `*.csproj`, `*.sln`, `*.slnx` | .NET/C# |
| Python source files | `*.py`, `*.pyi`, `pyproject.toml`, `requirements.txt`, `setup.py`, `Pipfile` | Python |
| JavaScript source files | `*.js`, `*.mjs`, `*.cjs` | JavaScript |
| TypeScript source files | `*.ts`, `*.tsx`, `tsconfig.json` | TypeScript (treat as JavaScript superset) |
| Go source files | `*.go`, `go.mod`, `go.sum` | Go |
| Java source files | `*.java`, `pom.xml`, `build.gradle`, `build.gradle.kts` | Java |
| Rust source files | `*.rs`, `Cargo.toml`, `Cargo.lock` | Rust |
| Ruby source files | `*.rb`, `*.gemspec`, `Gemfile` | Ruby |

### 3.2 Framework Detection

| Signal | File Pattern | Detected Framework |
|--------|-------------|--------------------|
| ASP.NET Core | `Program.cs` + `*.csproj` containing `Microsoft.AspNetCore` | ASP.NET Core |
| Blazor | `*.razor`, `*.razor.cs` | Blazor Server/WebAssembly |
| EF Core | `*.csproj` containing `Microsoft.EntityFrameworkCore` | Entity Framework Core |
| Django | `manage.py`, `settings.py` containing `DJANGO_SETTINGS_MODULE` | Django |
| FastAPI | `*.py` importing `fastapi` or `pyproject.toml` listing `fastapi` | FastAPI |
| Flask | `*.py` importing `flask` | Flask |
| Express | `package.json` listing `express` | Express.js |
| NestJS | `package.json` listing `@nestjs/core` | NestJS |
| Next.js | `package.json` listing `next` | Next.js |
| Spring Boot | `pom.xml`/`build.gradle` containing `spring-boot` | Spring Boot |
| Actix | `Cargo.toml` containing `actix` | Actix Web |
| Gin | `go.mod` containing `gin-gonic` | Gin |

### 3.3 Test Framework Detection

| Signal | Pattern | Detected Framework |
|--------|---------|--------------------|
| xUnit/NUnit/MSTest | `*.csproj` containing `xunit`/`nunit`/`MSTest` | .NET test frameworks |
| pytest | `pytest.ini`, `pyproject.toml` with `[tool.pytest]`, `conftest.py` | pytest |
| unittest | `*test*.py` with `unittest.TestCase` | unittest |
| Jest | `jest.config.*`, `package.json` with `jest` | Jest |
| Vitest | `vitest.config.*`, `package.json` with `vitest` | Vitest |
| Go testing | `*_test.go` files | Go testing |
| JUnit | `*Test.java` with `@Test` annotations | JUnit |
| Cargo test | `Cargo.toml` with `[dev-dependencies]` | Rust test |

### 3.4 CI/CD Detection

| Signal | File Pattern | Platform |
|--------|-------------|----------|
| GitLab CI | `.gitlab-ci.yml` | GitLab |
| GitHub Actions | `.github/workflows/*.yml` | GitHub |
| Jenkins | `Jenkinsfile` | Jenkins |
| CircleCI | `.circleci/config.yml` | CircleCI |
| Azure Pipelines | `azure-pipelines.yml`, `.azure-pipelines.yml` | Azure DevOps |

### 3.5 Infrastructure Detection

| Signal | File Pattern | Component |
|--------|-------------|-----------|
| Docker | `Dockerfile`, `docker-compose.yml`, `docker-compose*.yml` | Containerization |
| Kubernetes | `*.yaml` with `apiVersion` + `kind` | Orchestration |
| Terraform | `*.tf` | Infrastructure as Code |
| Database migrations | `migrations/`, `Migrations/`, `alembic/`, `db/migrate/` | Schema management |
| OTel | `otel-collector-config.yml`, `opentelemetry` in dependencies | Observability |
| Grafana | `grafana/`, `grafana.ini`, `provisioning/` | Dashboards |
| Prometheus | `prometheus.yml`, `prometheus/` | Metrics |

### 3.6 Discovery Procedure

1. **Scan root directory** for all file patterns above.
2. **Identify primary language** by file count (most source files wins).
3. **Identify frameworks** by config file analysis.
4. **Identify test framework** by test directory/config presence.
5. **Identify CI/CD** by workflow file presence.
6. **Identify infrastructure** by Docker/ORM/observability configs.
7. **Select the matching checklist** from Section 4 based on detected technologies.
8. **If multiple languages detected** (monorepo, polyglot), run all applicable checklists.

---

## Technology-Specific Checklists

### 4.1 .NET/C# Traceability Checklist

- [ ] **Agent audit trail persistence**: Admin actions (user create/delete, role change, config update) logged to durable database table, not only to Serilog/console
- [ ] **Evidence file completeness**: Each audit squad output has a corresponding evidence file with valid JSON findings
- [ ] **Span validation**: OTel spans cover all audit operations with correct operation names and attributes
- [ ] **Cost auditing per agent**: Token consumption tracked per squad per run with model and timestamp
- [ ] **Inter-squad task tracking**: Squad findings reference upstream/downstream squad outputs by ID
- [ ] **Provenance metadata on reports**: Reports include `agentId`, `timestamp`, `inputHash`, `modelVersion`, `squadName`
- [ ] **Finding deduplication**: Cross-squad findings deduplicated with `provenanceChain` array showing which squads confirmed the finding
- [ ] **Audit log immutability**: Audit entries stored in append-only table with no UPDATE/DELETE permissions
- [ ] **Structured logging vs audit persistence**: Distinction between operational logs (Serilog) and audit trail (database) — critical actions must be in database

### 4.2 Python Traceability Checklist

- [ ] **Audit trail durability**: Critical operations persisted to database, not only to `structlog`/`logging`
- [ ] **Evidence file validation**: Evidence JSON files parse without errors and contain required fields
- [ ] **Cost tracking**: Token usage tracked per model call with squad attribution
- [ ] **Provenance metadata**: Audit reports include model version and input fingerprint

### 4.3 JavaScript/TypeScript Traceability Checklist

- [ ] **Audit logging persistence**: Admin actions persisted to database, not only console/Pino
- [ ] **Evidence file structure**: Evidence files follow consistent schema across all squads
- [ ] **Cost tracking**: API call costs attributed to originating squad

### 4.4 Go Traceability Checklist

- [ ] **Audit middleware**: HTTP middleware logs critical operations to database with structured context
- [ ] **Evidence file verification**: Automated verification of evidence file integrity

### 4.5 Java Traceability Checklist

- [ ] **Audit entity persistence**: Spring Data JPA audit entities with immutable timestamps
- [ ] **Evidence validation**: Schema validation on evidence files before processing

### 4.6 Rust Traceability Checklist

- [ ] **Audit trail serialization**: Structured audit events serialized to durable storage
- [ ] **Evidence file integrity**: Hash verification of evidence files

---

## Anti-Patterns

### 5.1 LMAS Traceability-Specific Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Admin actions logged to structured logs only | high | Critical operations (user create, role change) written to Serilog but not to database | Logs can be purged, rotated, or lost — no durable audit trail for compliance | Persist audit events to database table with append-only constraints |
| 2 | Audit report lacking provenance metadata | high | Report missing agent ID, timestamp, input hash, or model version | Cannot verify who produced the report, when, from what input, using which model | Include required provenance fields in every audit report header |
| 3 | Evidence files empty or missing | high | Squad output directory exists but evidence files are empty or absent | Cannot verify audit was actually performed, findings cannot be reproduced | Validate evidence files exist, are non-empty, and parse as valid JSON |
| 4 | No cost attribution per squad | medium | Token consumption tracked globally but not per squad | Cannot identify most expensive squads or optimize cost allocation | Track `squadName`, `modelId`, `inputTokens`, `outputTokens`, `timestamp` per call |
| 5 | Orphan findings across squads | medium | Finding references upstream squad that doesn't exist or has no output | Inter-squad dependency broken, finding cannot be validated or escalated | Validate upstream references exist and have matching output files |
| 6 | Audit logs mutable | high | Audit table allows UPDATE/DELETE operations | Audit trail can be tampered with post-factum | Use append-only tables with restricted permissions and triggers preventing modification |

### 5.2 Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded secrets in audit config | critical | Database credentials, API keys in audit configuration files | Credentials exposed, audit system itself compromised | Use environment variables or secret manager for all credentials |
| 2 | Missing timestamp on audit entries | high | Audit entries lack creation timestamp | Cannot establish timeline of events | Auto-generate timestamp on insert, make column non-nullable |

---

## Code Examples (Bad/Good Pairs)

### 6.1 .NET/C# Examples

#### Example 1: Audit Trail — Structured Log vs Durable Persistence

```csharp
// ❌ BAD: Admin action logged only to Serilog — lost on log rotation
public async Task AssignRoleAsync(Guid userId, string role)
{
    _logger.LogInformation("Assigned role {Role} to user {UserId}", role, userId);
    await _userRepository.UpdateRoleAsync(userId, role);
}

// ✅ GOOD: Admin action persisted to audit table + structured log
public async Task AssignRoleAsync(Guid userId, string role, string actorId)
{
    await _auditRepository.AddAsync(new AuditEntry
    {
        Action = "AssignRole",
        TargetType = "User",
        TargetId = userId,
        ActorId = actorId,
        Details = $"Assigned role '{role}'",
        Timestamp = DateTime.UtcNow
    });
    _logger.LogInformation("Assigned role {Role} to user {UserId}", role, userId);
    await _userRepository.UpdateRoleAsync(userId, role);
}
```

#### Example 2: Audit Report Provenance Metadata

```csharp
// ❌ BAD: Report missing provenance — cannot verify origin
var report = new AuditReport
{
    SquadName = "audit-security",
    Findings = findings
};

// ✅ GOOD: Full provenance chain on report
var report = new AuditReport
{
    SquadName = "audit-security",
    AgentId = Environment.GetEnvironmentVariable("AGENT_ID"),
    Timestamp = DateTime.UtcNow,
    InputHash = ComputeHash(inputFiles),
    ModelVersion = Environment.GetEnvironmentVariable("MODEL_VERSION"),
    Findings = findings
};
```

### 6.2 Python Examples

#### Example 1: Evidence File Validation

```python
# ❌ BAD: No validation — evidence file could be empty or malformed
def load_evidence(squad_name: str) -> dict:
    path = Path(f".sisyphus/evidence/{squad_name}.json")
    return json.loads(path.read_text())

# ✅ GOOD: Validate evidence file exists, is non-empty, and parses correctly
def load_evidence(squad_name: str) -> dict:
    path = Path(f".sisyphus/evidence/{squad_name}.json")
    if not path.exists():
        raise EvidenceMissingError(f"No evidence file for squad: {squad_name}")
    content = path.read_text()
    if not content.strip():
        raise EvidenceEmptyError(f"Evidence file empty for squad: {squad_name}")
    data = json.loads(content)
    required_fields = {"squad", "timestamp", "findings"}
    missing = required_fields - set(data.keys())
    if missing:
        raise EvidenceSchemaError(f"Missing fields: {missing}")
    return data
```

---

## Output Format

All audit findings MUST be emitted as structured JSON objects. One finding per object.

### 7.1 Finding Schema

```json
{
  "severity": "critical|high|medium|low",
  "file": "relative/path/to/file",
  "line": 42,
  "endLine": 45,
  "description": "What was found and why it matters. Include the traceability gap or evidence issue.",
  "recommendation": "Specific fix with code example. Reference audit best practice.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "compliance|security|maintainability|observability"
}
```

### 7.2 Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Section 8 for escalation rules.
- **file**: Relative path from project root. Use forward slashes. Never absolute paths.
- **line**: 1-based line number where the finding starts.
- **endLine**: Optional. Line where the finding ends (for multi-line issues).
- **description**: Factual, specific. Include the traceability issue. No hedging ("might be a problem").
- **recommendation**: Actionable. Include a code snippet showing the fix when possible.
- **technology**: Primary technology of the finding. Use `CrossLanguage` only for patterns that span multiple files in different languages.
- **category**: Primary category. Use the most specific category.

### 7.3 Summary Report

After all findings, emit a summary:

```json
{
  "summary": {
    "squad": "audit-lmas-traceability",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#"],
    "totalFindings": 8,
    "bySeverity": {
      "critical": 0,
      "high": 3,
      "medium": 4,
      "low": 1
    },
    "byCategory": {
      "compliance": 5,
      "maintainability": 2,
      "observability": 1
    },
    "filesScanned": 25,
    "filesWithFindings": 6
  }
}
```

---

## Severity Guidelines

### 8.1 Base Severity Definitions

| Level | Definition | Example |
|-------|-----------|---------|
| **critical** | Audit trail completely absent for critical operations | No audit logging for user management, role assignment, or data deletion |
| **high** | Audit trail incomplete — structured logs only, no durable persistence | Admin actions logged to Serilog but not database; reports missing provenance |
| **medium** | Missing cost attribution, orphan findings, incomplete evidence validation | No per-squad cost tracking, evidence files not validated before processing |
| **low** | Minor traceability improvements, documentation gaps | Missing audit log enricher, incomplete provenance fields |

### 8.2 Severity Escalation Triggers

Apply these modifiers to the base severity. Multiple triggers stack (up to critical max).

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding is in authentication/authorization code | +1 level | All technologies |
| Finding is in data encryption/key management | +1 level | All technologies |
| Finding affects multiple tenants or users | +1 level | Multi-tenant systems |
| Finding is in a public-facing endpoint | +1 level | Web applications |
| Finding involves financial/billing data | +1 level | Financial systems |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding is in non-production/test code only | -1 level (min low) | All technologies |
| Finding has existing mitigating control documented | -1 level (min low) | All technologies |
| Finding is in deprecated code scheduled for removal | -1 level (min low) | All technologies |
| Finding is in documentation/comments | -1 level (min low) | All technologies |

### 8.4 Technology-Specific Severity Adjustments

#### .NET/C#
- Missing audit repository for admin actions → high (no durable trail)
- Serilog-only audit logging → high (lost on rotation)
- Missing EF audit interceptor → medium (no automatic audit capture)

#### Python
- `print()` for audit events → high (no structured output, no persistence)
- Missing `structlog` with audit sink → medium (unstructured audit)

#### JavaScript/TypeScript
- `console.log` for audit events → high (no persistence)
- Missing database audit table → high (no durable trail)

#### Go
- Missing middleware audit logging → medium (gaps in coverage)
- Audit entries without context → low (incomplete traceability)

#### Java
- Missing Spring Security audit events → high (no auth audit)
- JPA entities without audit columns → medium (no timestamp tracking)

#### Rust
- `println!` for audit events → high (no structured output)
- Missing audit serialization → medium (no persistence)

---

## Integration Notes

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
audit-cicd ─────────────────────────┤
audit-license ──────────────────────┤
audit-refactoring ──────────────────┤
audit-product-roadmap ──────────────┤
audit-acquisition-metrics ──────────┤
audit-ai-cost ──────────────────────┤
audit-regulatory-* ─────────────────┤
audit-lmas-traceability ────────────┤
audit-schema-metrics ───────────────┤
audit-consolidated ◄────────────────┘  (consumes all other squad outputs)
audit-consistency-coherence            (consumes all other squad outputs)
```

### 9.2 Upstream Dependencies

- **All other audit squads**: This squad consumes outputs from every other squad to verify evidence completeness and inter-squad traceability

### 9.3 Downstream Consumers

- **audit-consolidated**: Aggregates all squad findings into unified report
- **audit-consistency-coherence**: Validates cross-domain alignment between squad findings
- **audit-ai-cost**: Uses cost tracking data from this squad for agent cost analysis

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-lmas-traceability | audit-observability | Traceability focuses on audit trail durability; observability focuses on runtime telemetry |
| audit-lmas-traceability | audit-ai-cost | Traceability focuses on agent behavior tracking; ai-cost focuses on token cost optimization |
| audit-lmas-traceability | audit-consolidated | Traceability verifies individual squad outputs; consolidated aggregates all findings |

### 9.5 Recommended Execution Order

For a full audit, run squads in this sequence:

1. **audit-dependencies** — Establish baseline of third-party risk
2. **audit-security** — Code-level vulnerability scan
3. **audit-architecture** — Structural analysis
4. **audit-code-quality** — Implementation quality
5. **audit-test-coverage** — Testing assessment
6. **audit-fault-tolerance** — Resilience review
7. **audit-observability** — Telemetry validation
8. **audit-privacy-data** — PII/data handling
9. **audit-cicd** — Pipeline security
10. **audit-license** — License compliance
11. **Domain-specific squads** (financial-model, tax-fiscal, regulatory-*, etc.)
12. **audit-lmas-traceability** — Traceability verification (this squad)
13. **audit-consolidated** — Aggregate all findings
14. **audit-consistency-coherence** — Cross-domain validation

### 9.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read the previous squad's findings from the shared output location.
2. Deduplicate: skip findings already reported by a previous squad.
3. Cross-reference: escalate findings confirmed by multiple squads.
4. Output in the standardized Section 7 format for aggregation.
