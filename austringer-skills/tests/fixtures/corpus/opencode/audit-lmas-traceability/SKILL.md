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

- Agent audit trail persistence — admin/agent actions logged to durable storage, not ephemeral structured logs
- Evidence file completeness — evidence files exist, non-empty, contain valid structured results
- Span validation — audit spans cover all required areas for complete traceability
- Cost auditing per agent — token consumption per squad per run, cost attribution
- Inter-squad task tracking — task handoff completeness, no orphan findings
- Provenance metadata on reports — agent ID, timestamp, input hash, model version
- Reproducibility check — same input produces consistent output across runs
- Audit log durability — persisted to database/durable storage, not console/structured output only
- Finding deduplication — cross-squad findings deduplicated with provenance chain

### DO NOT USE FOR

- Observability infrastructure setup (use `audit-observability`)
- Test quality assessment (use `audit-test-coverage`)
- Code security scanning (use `audit-security`)
- Performance profiling (use `audit-code-quality`)

---

## Target Discovery

Agent MUST auto-detect project structure. Do NOT hardcode repository names.

### 3.1 Language Detection

| Signal | File Pattern | Detected Language |
|--------|-------------|-------------------|
| C# source files | `*.cs`, `*.csproj`, `*.sln`, `*.slnx` | .NET/C# |
| Python source files | `*.py`, `*.pyi`, `pyproject.toml`, `requirements.txt`, `setup.py`, `Pipfile` | Python |
| JavaScript source files | `*.js`, `*.mjs`, `*.cjs` | JavaScript |
| TypeScript source files | `*.ts`, `*.tsx`, `tsconfig.json` | TypeScript (treat as JS superset) |
| Go source files | `*.go`, `go.mod`, `go.sum` | Go |
| Java source files | `*.java`, `pom.xml`, `build.gradle`, `build.gradle.kts` | Java |
| Rust source files | `*.rs`, `Cargo.toml`, `Cargo.lock` | Rust |
| Ruby source files | `*.rb`, `*.gemspec`, `Gemfile` | Ruby |

### 3.2 Framework Detection

| Signal | File Pattern | Framework |
|--------|-------------|-----------|
| ASP.NET Core | `Program.cs` + `*.csproj` with `Microsoft.AspNetCore` | ASP.NET Core |
| Blazor | `*.razor`, `*.razor.cs` | Blazor Server/WebAssembly |
| EF Core | `*.csproj` with `Microsoft.EntityFrameworkCore` | Entity Framework Core |
| Django | `manage.py`, `settings.py` with `DJANGO_SETTINGS_MODULE` | Django |
| FastAPI | `*.py` importing `fastapi` / `pyproject.toml` listing `fastapi` | FastAPI |
| Flask | `*.py` importing `flask` | Flask |
| Express | `package.json` listing `express` | Express.js |
| NestJS | `package.json` listing `@nestjs/core` | NestJS |
| Next.js | `package.json` listing `next` | Next.js |
| Spring Boot | `pom.xml`/`build.gradle` with `spring-boot` | Spring Boot |
| Actix | `Cargo.toml` with `actix` | Actix Web |
| Gin | `go.mod` with `gin-gonic` | Gin |

### 3.3 Test Framework Detection

| Signal | Pattern | Framework |
|--------|---------|-----------|
| xUnit/NUnit/MSTest | `*.csproj` with `xunit`/`nunit`/`MSTest` | .NET |
| pytest | `pytest.ini`, `pyproject.toml` `[tool.pytest]`, `conftest.py` | pytest |
| unittest | `*test*.py` with `unittest.TestCase` | unittest |
| Jest | `jest.config.*`, `package.json` with `jest` | Jest |
| Vitest | `vitest.config.*`, `package.json` with `vitest` | Vitest |
| Go testing | `*_test.go` files | Go testing |
| JUnit | `*Test.java` with `@Test` | JUnit |
| Cargo test | `Cargo.toml` with `[dev-dependencies]` | Rust test |

### 3.4 CI/CD Detection

| Signal | Pattern | Platform |
|--------|---------|----------|
| GitLab CI | `.gitlab-ci.yml` | GitLab |
| GitHub Actions | `.github/workflows/*.yml` | GitHub |
| Jenkins | `Jenkinsfile` | Jenkins |
| CircleCI | `.circleci/config.yml` | CircleCI |
| Azure Pipelines | `azure-pipelines.yml`, `.azure-pipelines.yml` | Azure DevOps |

### 3.5 Infrastructure Detection

| Signal | Pattern | Component |
|--------|---------|-----------|
| Docker | `Dockerfile`, `docker-compose.yml`, `docker-compose*.yml` | Containerization |
| Kubernetes | `*.yaml` with `apiVersion` + `kind` | Orchestration |
| Terraform | `*.tf` | IaC |
| DB migrations | `migrations/`, `Migrations/`, `alembic/`, `db/migrate/` | Schema mgmt |
| OTel | `otel-collector-config.yml`, `opentelemetry` in deps | Observability |
| Grafana | `grafana/`, `grafana.ini`, `provisioning/` | Dashboards |
| Prometheus | `prometheus.yml`, `prometheus/` | Metrics |

### 3.6 Discovery Procedure

1. **Scan root** for all file patterns.
2. **Identify primary language** by file count.
3. **Identify frameworks** by config files.
4. **Identify test framework** by dir/config.
5. **Identify CI/CD** by workflow files.
6. **Identify infrastructure** by Docker/ORM/observability configs.
7. **Select checklist** from Section 4 based on technologies.
8. **If multiple languages** (monorepo), run all applicable checklists.

---

## Technology-Specific Checklists

### 4.1 .NET/C# Traceability Checklist

- [ ] **Agent audit trail**: Admin actions (user create/delete, role change, config update) logged to DB table, not Serilog/console only
- [ ] **Evidence file completeness**: Each squad output has evidence file with valid JSON findings
- [ ] **Span validation**: OTel spans cover all audit ops with correct operation names/attributes
- [ ] **Cost auditing per agent**: Token consumption per squad per run with model/timestamp
- [ ] **Inter-squad task tracking**: Squad findings reference upstream/downstream outputs by ID
- [ ] **Provenance metadata**: Reports include `agentId`, `timestamp`, `inputHash`, `modelVersion`, `squadName`
- [ ] **Finding deduplication**: Cross-squad findings deduplicated with `provenanceChain` array
- [ ] **Audit log immutability**: Append-only table, no UPDATE/DELETE permissions
- [ ] **Structured logging vs audit persistence**: Critical actions in DB, not just Serilog

### 4.2 Python Traceability Checklist

- [ ] **Audit trail durability**: Critical ops persisted to DB, not only structlog/logging
- [ ] **Evidence file validation**: JSON files parse without errors, contain required fields
- [ ] **Cost tracking**: Token usage tracked per model call with squad attribution
- [ ] **Provenance metadata**: Audit reports include model version and input fingerprint

### 4.3 JavaScript/TypeScript Traceability Checklist

- [ ] **Audit logging persistence**: Admin actions persisted to DB, not only console/Pino
- [ ] **Evidence file structure**: Consistent schema across all squads
- [ ] **Cost tracking**: API call costs attributed to originating squad

### 4.4 Go Traceability Checklist

- [ ] **Audit middleware**: HTTP middleware logs critical ops to DB
- [ ] **Evidence file verification**: Automated integrity verification

### 4.5 Java Traceability Checklist

- [ ] **Audit entity persistence**: Spring Data JPA audit entities with immutable timestamps
- [ ] **Evidence validation**: Schema validation before processing

### 4.6 Rust Traceability Checklist

- [ ] **Audit trail serialization**: Structured events to durable storage
- [ ] **Evidence file integrity**: Hash verification of evidence files

---

## Anti-Patterns

### 5.1 LMAS Traceability-Specific Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Admin actions logged to structured logs only | high | Critical ops to Serilog, not DB | Logs purged/rotated/lost — no durable trail | Persist to DB with append-only |
| 2 | Audit report lacks provenance metadata | high | Missing agent ID, timestamp, input hash, model version | Cannot verify report origin | Include required provenance fields |
| 3 | Evidence files empty or missing | high | Squad output dir exists but evidence empty/absent | Cannot verify audit performed | Validate exist, non-empty, valid JSON |
| 4 | No cost attribution per squad | medium | Global token consumption, not per squad | Cannot ID most expensive squads | Track squadName, modelId, tokens, timestamp per call |
| 5 | Orphan findings across squads | medium | Finding references non-existent upstream | Broken inter-squad dependency | Validate upstream references exist |
| 6 | Audit logs mutable | high | Audit table allows UPDATE/DELETE | Tampering possible | Append-only table with restricted permissions |

### 5.2 Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded secrets in audit config | critical | DB credentials, API keys in audit config files | Credentials exposed, audit system compromised | Use env vars or secret manager |
| 2 | Missing timestamp on audit entries | high | No creation timestamp | Cannot establish timeline | Auto-generate timestamp on insert, non-nullable |

---

## Code Examples (Bad/Good Pairs)

### 6.1 .NET/C# Examples

#### Example 1: Audit Trail — Structured Log vs Durable Persistence

```csharp
// BAD: Admin action logged only to Serilog — lost on log rotation
public async Task AssignRoleAsync(Guid userId, string role)
{
    _logger.LogInformation("Assigned role {Role} to user {UserId}", role, userId);
    await _userRepository.UpdateRoleAsync(userId, role);
}

// GOOD: Admin action persisted to audit table + structured log
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
// BAD: Missing provenance — cannot verify origin
var report = new AuditReport
{
    SquadName = "audit-security",
    Findings = findings
};

// GOOD: Full provenance chain on report
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
# BAD: No validation — file could be empty or malformed
def load_evidence(squad_name: str) -> dict:
    path = Path(f".sisyphus/evidence/{squad_name}.json")
    return json.loads(path.read_text())

# GOOD: Validate exists, non-empty, parses correctly
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
  "description": "Traceability gap or evidence issue found.",
  "recommendation": "Specific fix with code example.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "compliance|security|maintainability|observability"
}
```

### 7.2 Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Section 8.
- **file**: Relative from project root. Forward slashes. Never absolute.
- **line**: 1-based line number.
- **endLine**: Optional. End of multi-line issue.
- **description**: Factual, specific. No hedging.
- **recommendation**: Actionable. Include code snippet when possible.
- **technology**: Primary technology. `CrossLanguage` for multi-file patterns.
- **category**: Most specific.

### 7.3 Summary Report

Emit summary after all findings:

```json
{
  "summary": {
    "squad": "audit-lmas-traceability",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#"],
    "totalFindings": 8,
    "bySeverity": { "critical": 0, "high": 3, "medium": 4, "low": 1 },
    "byCategory": { "compliance": 5, "maintainability": 2, "observability": 1 },
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
| **critical** | Audit trail completely absent for critical ops | No audit logging for user management, role assignment, data deletion |
| **high** | Audit trail incomplete — structured logs only, no durable persistence | Admin in Serilog not DB; reports missing provenance |
| **medium** | Missing cost attribution, orphan findings, incomplete evidence validation | No per-squad cost, evidence files not validated |
| **low** | Minor traceability improvements, doc gaps | Missing enricher, incomplete provenance fields |

### 8.2 Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding in auth/authorization code | +1 level | All |
| Finding in data encryption/key mgmt | +1 level | All |
| Finding affects multiple tenants/users | +1 level | Multi-tenant |
| Finding in public-facing endpoint | +1 level | Web apps |
| Finding involves financial/billing data | +1 level | Financial |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Non-production/test code only | -1 level (min low) | All |
| Existing mitigating control documented | -1 level (min low) | All |
| Deprecated code scheduled for removal | -1 level (min low) | All |
| Documentation/comments | -1 level (min low) | All |

### 8.4 Technology-Specific Severity Adjustments

#### .NET/C#
- Missing audit repository for admin actions → high
- Serilog-only audit logging → high (lost on rotation)
- Missing EF audit interceptor → medium

#### Python
- `print()` for audit events → high
- Missing structlog with audit sink → medium

#### JavaScript/TypeScript
- `console.log` for audit events → high
- Missing DB audit table → high

#### Go
- Missing middleware audit logging → medium
- Audit entries without context → low

#### Java
- Missing Spring Security audit events → high
- JPA entities without audit columns → medium

#### Rust
- `println!` for audit events → high
- Missing audit serialization → medium

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
audit-consolidated ◄────────────────┘
audit-consistency-coherence
```

### 9.2 Upstream Dependencies

- **All other audit squads**: Consumes outputs from every squad for evidence completeness and inter-squad traceability.

### 9.3 Downstream Consumers

- **audit-consolidated**: Aggregates all findings.
- **audit-consistency-coherence**: Validates cross-domain alignment.
- **audit-ai-cost**: Uses cost tracking data.

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-lmas-traceability | audit-observability | Traceability: audit trail durability; Observability: runtime telemetry |
| audit-lmas-traceability | audit-ai-cost | Traceability: agent behavior tracking; Ai-cost: token cost optimization |
| audit-lmas-traceability | audit-consolidated | Traceability: individual squad outputs; Consolidated: all findings aggregate |

### 9.5 Recommended Execution Order

1. **audit-dependencies** — Third-party risk
2. **audit-security** — Vuln scan
3. **audit-architecture** — Structural analysis
4. **audit-code-quality** — Implementation quality
5. **audit-test-coverage** — Testing assessment
6. **audit-fault-tolerance** — Resilience review
7. **audit-observability** — Telemetry validation
8. **audit-privacy-data** — PII/data handling
9. **audit-cicd** — Pipeline security
10. **audit-license** — License compliance
11. Domain-specific squads
12. **audit-lmas-traceability** — Traceability verification (this squad)
13. **audit-consolidated** — Aggregate all
14. **audit-consistency-coherence** — Cross-domain validation

### 9.6 Output Integration

When squads run sequentially:
1. Read previous squad's findings from shared output.
2. Deduplicate: skip already reported.
3. Cross-reference: escalate confirmed by multiple.
4. Output in standardized Section 7 format.