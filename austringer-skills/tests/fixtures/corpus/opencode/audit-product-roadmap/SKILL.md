---
name: audit-product-roadmap
description: >-
  Product roadmap synchronization auditing: delivery pace vs milestones, technical
  debt impact on timeline, feature completeness vs plan, sprint velocity assessment,
  resource adequacy, and dependency risk analysis.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "2.0.0"
---

# Product Roadmap Sync Audit

## USE FOR

- Delivery pace vs milestones — compare planned timeline against actual code
- Technical debt impact on timeline — flag debt slowing roadmap delivery
- Feature completeness vs plan — gaps between business plan and code reality
- Sprint velocity assessment — delivery cadence vs roadmap commitments
- Resource adequacy — team size/skills match roadmap ambition
- Dependency risk — external deps that could delay roadmap items
- MVP scope validation — MVP achievable with current resources
- Commit velocity analysis — commit frequency/size vs milestones
- Documentation currency — business plan reflects current code state

### DO NOT USE FOR

- Code quality assessment (use `audit-code-quality`)
- Financial projections accuracy (use `audit-financial-model`)
- Security vulnerabilities (use `audit-security`)
- Test coverage analysis (use `audit-test-coverage`)
- Architecture review (use `audit-architecture`)
- Competitive analysis (use `audit-competitive-intel`)

---

## Target Discovery

Agent MUST auto-detect project structure. Do NOT hardcode repo names.

### 3.1 Language Detection

| Signal | File Pattern | Detected Language |
|--------|-------------|-------------------|
| C# source files | `*.cs`, `*.csproj`, `*.sln`, `*.slnx` | .NET/C# |
| Python source files | `*.py`, `*.pyi`, `pyproject.toml`, `requirements.txt`, `setup.py`, `Pipfile` | Python |
| JavaScript source files | `*.js`, `*.mjs`, `*.cjs` | JavaScript |
| TypeScript source files | `*.ts`, `*.tsx`, `tsconfig.json` | TypeScript |
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
| Django | `manage.py`, `settings.py` with `DJANGO_SETTINGS_MODULE` | Django |
| FastAPI | `*.py` importing `fastapi` or `pyproject.toml` listing `fastapi` | FastAPI |
| Flask | `*.py` importing `flask` | Flask |
| Express | `package.json` listing `express` | Express.js |
| NestJS | `package.json` listing `@nestjs/core` | NestJS |
| Next.js | `package.json` listing `next` | Next.js |
| Spring Boot | `pom.xml`/`build.gradle` with `spring-boot` | Spring Boot |
| Actix | `Cargo.toml` containing `actix` | Actix Web |
| Gin | `go.mod` containing `gin-gonic` | Gin |

### 3.3 Test Framework Detection

| Signal | Pattern | Detected Framework |
|--------|---------|--------------------|
| xUnit/NUnit/MSTest | `*.csproj` with `xunit`/`nunit`/`MSTest` | .NET test |
| pytest | `pytest.ini`, `pyproject.toml` `[tool.pytest]`, `conftest.py` | pytest |
| unittest | `*test*.py` with `unittest.TestCase` | unittest |
| Jest | `jest.config.*`, `package.json` with `jest` | Jest |
| Vitest | `vitest.config.*`, `package.json` with `vitest` | Vitest |
| Go testing | `*_test.go` | Go testing |
| JUnit | `*Test.java` with `@Test` | JUnit |
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
| Terraform | `*.tf` | IaC |
| DB migrations | `migrations/`, `Migrations/`, `alembic/`, `db/migrate/` | Schema management |
| OTel | `otel-collector-config.yml`, `opentelemetry` deps | Observability |
| Grafana | `grafana/`, `grafana.ini`, `provisioning/` | Dashboards |
| Prometheus | `prometheus.yml`, `prometheus/` | Metrics |

### 3.6 Discovery Procedure

1. **Scan root** for file patterns above.
2. **Identify primary language** by file count.
3. **Identify frameworks** by config analysis.
4. **Identify test framework** by test dir/config.
5. **Identify CI/CD** by workflow files.
6. **Identify infrastructure** by Docker/ORM/OTel configs.
7. **Scan for business plan docs** — `docs/`, `business-plan/`, `README.md`, milestone docs
8. **Analyze git history** — commit frequency, last commit date, branch structure
9. **Select checklist** from Section 4 based on detected tech and docs.
10. **Multiple languages** (monorepo/polyglot) → run all applicable checklists.

---

## Technology-Specific Checklists

### 4.1 Cross-Language Product Roadmap Checklist

- [ ] **Delivery pace vs milestones**: Compare planned dates vs git history and feature completion
- [ ] **Tech debt impact**: TODOs, FIXMEs, deprecated patterns slowing delivery
- [ ] **Feature completeness vs plan**: Cross-reference business plan vs actual capabilities
- [ ] **Sprint velocity**: Commit frequency/scope vs roadmap commitments
- [ ] **Resource adequacy**: Effort remaining vs stated team capacity
- [ ] **Dependency risk**: External deps not yet integrated
- [ ] **MVP scope validation**: MVP features implemented and testable
- [ ] **Documentation currency**: Business plan reflects code state, not outdated aspirations
- [ ] **Module completion tracking**: Each planned module/domain has source code
- [ ] **Test coverage vs plan**: Test pyramid matches planned quality level

### 4.2 .NET/C# Specific Checklist

- [ ] **Module registration**: Each planned module has `*ModuleRegistration.cs`
- [ ] **Wolverine handler coverage**: Each planned command/query has handler
- [ ] **Blazor page completeness**: Features have `.razor` pages under `Features/`
- [ ] **EF migration currency**: Migrations up to date with entity definitions
- [ ] **Health check coverage**: Each external dep has registered health check

### 4.3 Python Specific Checklist

- [ ] **API endpoint coverage**: Each planned endpoint has route handler
- [ ] **Django/FastAPI app registration**: Modules registered in app
- [ ] **Migration currency**: Alembic/Django migrations reflect current models

### 4.4 JavaScript/TypeScript Specific Checklist

- [ ] **Route coverage**: Each planned page has route definition
- [ ] **Component library completeness**: Shared components for planned UI
- [ ] **API integration**: Frontend calls match backend endpoint definitions

### 4.5 Go Specific Checklist

- [ ] **Handler coverage**: Each planned endpoint has handler function
- [ ] **Middleware registration**: Auth, logging, CORS registered
- [ ] **DB migration currency**: Migrations match current models

### 4.6 Java Specific Checklist

- [ ] **Spring Bean registration**: Each service has `@Service`/`@Component`
- [ ] **REST controller coverage**: Each endpoint has controller method
- [ ] **JPA entity coverage**: Each domain model has entity class

---

## Anti-Patterns

### 5.1 Product Roadmap-Specific Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Features promised with no code | critical | Feature in business plan with zero implementation | Stakeholders expect nonexistent functionality | Update business plan to reflect actual state |
| 2 | Major timeline gaps | high | Planned dates passed with no code | Project appears stalled; confidence lost | Rebaseline timeline vs actual velocity |
| 3 | Unresourced roadmap items | high | Features with no owner or capacity | Never get picked up; wish list | Assign owners, estimate effort, remove excess |
| 4 | Unrealistic velocity | high | Sprint assumes no tech debt, bugs, scope creep | Actual delivery falls short | Base velocity on historical data |
| 5 | Docs don't match code | medium | Features described "launched" but skeleton only | Misleading to stakeholders/investors | Sync docs with reality; label planned/in-progress/shipped |
| 6 | Missing tests for shipped | medium | Features marked complete with 0 tests | Quality gate bypass, regressions likely | Minimum coverage threshold per feature |
| 7 | Stale external deps | medium | Plan relies on integrations not yet implemented | Timeline at risk | Track integration work as explicit items |

### 5.2 Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded feature flags | medium | Feature flags hardcoded `true` not config-driven | Cannot toggle features | Use config system or feature flag service |

---

## Code Examples (Bad/Good Pairs)

### 6.1 .NET/C# Examples

#### Example 1: Module Registration — Planned vs Actual

```csharp
// ❌ BAD: Business plan lists "Reporting" module but no registration exists
// Program.cs includes: AddPeopleModule(), AddFinancialModule(), AddDocumentsModule()
// Missing: AddReportingModule()

// ✅ GOOD: All planned modules registered
services.AddPeopleModule();
services.AddFinancialModule();
services.AddDocumentsModule();
services.AddReportingModule();  // Matches business plan
```

#### Example 2: Feature Completeness — Handler Implementation

```csharp
// ❌ BAD: Business plan promises "automated report generation" — handler is empty
public class GenerateReportHandler
{
    public Task Handle(GenerateReport command)
    {
        // TODO: Implement report generation
        return Task.CompletedTask;
    }
}

// ✅ GOOD: Handler fully implemented with actual logic
public class GenerateReportHandler
{
    public async Task<ReportResult> Handle(GenerateReport command)
    {
        var data = await _reportRepository.QueryAsync(command.Parameters);
        var pdf = await _reportGenerator.GenerateAsync(data, command.Format);
        await _storage.SaveAsync(pdf, command.OutputPath);
        return new ReportResult(pdf.Id, pdf.Pages);
    }
}
```

### 6.2 Python Examples

#### Example 1: API Endpoint Coverage

```python
# ❌ BAD: Business plan promises "analytics dashboard API" — no endpoint
# urls.py only has: path('api/users/'), path('api/reports/')

# ✅ GOOD: All planned endpoints registered
urlpatterns = [
    path("api/users/", user_views.list),
    path("api/reports/", report_views.list),
    path("api/analytics/", analytics_views.dashboard),  # Matches business plan
]
```

### 6.3 JavaScript/TypeScript Examples

#### Example 1: Route Coverage vs Business Plan

```typescript
// ❌ BAD: Business plan promises "Settings" page — no route defined
const routes = [
  { path: "/", component: Dashboard },
  { path: "/users", component: UserList },
  { path: "/reports", component: Reports },
  // Missing: /settings
];

// ✅ GOOD: All planned routes defined
const routes = [
  { path: "/", component: Dashboard },
  { path: "/users", component: UserList },
  { path: "/reports", component: Reports },
  { path: "/settings", component: Settings },  // Matches business plan
];
```

---

## Output Format

All audit findings MUST be emitted as structured JSON objects.

### 7.1 Finding Schema

```json
{
  "severity": "critical|high|medium|low",
  "file": "relative/path/to/file",
  "line": 42,
  "endLine": 45,
  "description": "What was found and why it matters. Include the roadmap sync issue.",
  "recommendation": "Specific adjustment suggestion with evidence.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "architecture|maintainability|testing|compliance"
}
```

### 7.2 Field Rules

- **severity**: One of `critical`, `high`, `medium`, `low`. See Section 8.
- **file**: Relative from project root. Forward slashes. Never absolute.
- **line**: 1-based line where finding starts.
- **endLine**: Optional. Where finding ends (multi-line).
- **description**: Factual, specific. Include roadmap gap/timeline issue. No hedging.
- **recommendation**: Actionable. Include evidence from codebase.
- **technology**: Primary tech. `CrossLanguage` for doc-level findings.
- **category**: Most specific.

### 7.3 Summary Report

After all findings, emit a summary:

```json
{
  "summary": {
    "squad": "audit-product-roadmap",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#", "Markdown"],
    "totalFindings": 12,
    "bySeverity": {
      "critical": 1,
      "high": 4,
      "medium": 5,
      "low": 2
    },
    "byCategory": {
      "architecture": 6,
      "maintainability": 3,
      "testing": 2,
      "compliance": 1
    },
    "filesScanned": 40,
    "filesWithFindings": 10
  }
}
```

---

## Severity Guidelines

### 8.1 Base Severity Definitions

| Level | Definition | Example |
|-------|-----------|---------|
| **critical** | Plan promises features not in code | "AI-powered analytics" in pitch deck with zero ML code |
| **high** | Major timeline gaps, unresourced items, unrealistic velocity | Past delivery with no code; unassigned 3-month feature |
| **medium** | Minor gaps, moderate tech debt, doc drift | TODOs in prod code; "launched" for skeleton features |
| **low** | Doc improvements, minor adjustments | |

### 8.2 Severity Escalation Triggers

Apply modifiers. Multiple stack (up to critical max).

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Investor-facing materials | +1 level | All |
| Financial/billing data | +1 level | Financial systems |
| Multiple tenants/users | +1 level | Multi-tenant |
| Public-facing endpoint | +1 level | Web apps |
| Auth/authorization code | +1 level | All |
| Data encryption/key mgmt | +1 level | All |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Non-production/test only | -1 level (min low) | All |
| Mitigating control documented | -1 level (min low) | All |
| Deprecated code scheduled for removal | -1 level (min low) | All |
| Documentation/comments | -1 level (min low) | All |

### 8.4 Technology-Specific Severity Adjustments

#### .NET/C#
- Empty handler with TODO → medium
- Missing module registration → high
- No EF migration for new entity → medium

#### Python
- Missing API endpoint for planned feature → medium
- Empty view with `pass` → medium
- Missing migration for new model → medium

#### JavaScript/TypeScript
- Missing route for planned page → medium
- Component with placeholder text → low
- Missing API integration → medium

#### Go
- Missing handler for planned endpoint → medium
- Middleware not registered → medium

#### Java
- Missing Spring controller → medium
- Service with empty method → medium

#### Rust
- Missing route handler → medium
- Stub impl returning default → low

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

- **audit-code-quality**: Tech debt metrics impacting roadmap timeline
- **audit-test-coverage**: Coverage data for feature completeness
- **audit-architecture**: Module/structure for completeness tracking

### 9.3 Downstream Consumers

- **audit-competitive-intel**: Uses roadmap findings for competitive positioning
- **audit-consolidated**: Aggregates all findings
- **audit-consistency-coherence**: Cross-domain alignment validation

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-product-roadmap | audit-competitive-intel | Roadmap: delivery vs plan; Competitive: market positioning |
| audit-product-roadmap | audit-financial-model | Roadmap: feature timeline; Financial: pricing/revenue |
| audit-product-roadmap | audit-code-quality | Roadmap: completeness; Code-quality: implementation quality |

### 9.5 Recommended Execution Order

1. **audit-dependencies** — Third-party risk baseline
2. **audit-security** — Vulnerability scan
3. **audit-architecture** — Structural analysis
4. **audit-code-quality** — Implementation quality
5. **audit-test-coverage** — Testing assessment
6. **audit-fault-tolerance** — Resilience
7. **audit-observability** — Telemetry
8. **audit-privacy-data** — PII/data handling
9. **audit-cicd** — Pipeline security
10. **audit-license** — License compliance
11. **Domain-specific squads** (financial-model, tax-fiscal, regulatory-*, etc.)
12. **audit-product-roadmap** — Roadmap sync (this squad)
13. **audit-consolidated** — Aggregate all
14. **audit-consistency-coherence** — Cross-domain validation

### 9.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read previous squad's findings from shared output.
2. Deduplicate: skip already-reported findings.
3. Cross-reference: escalate findings confirmed by multiple squads.
4. Output in Section 7 format.