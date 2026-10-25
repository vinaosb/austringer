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

- Delivery pace vs milestones — compare planned timeline against actual code state
- Technical debt impact on timeline — flag debt that will slow roadmap delivery
- Feature completeness vs plan — identify gaps between business plan promises and code reality
- Sprint velocity assessment — evaluate if delivery cadence matches roadmap commitments
- Resource adequacy check — team size/skills match roadmap ambition
- Dependency risk — external dependencies that could delay roadmap items
- MVP scope validation — verify MVP definition is achievable with current resources
- Commit velocity analysis — evaluate commit frequency and size against roadmap milestones
- Documentation currency — verify business plan documents reflect current code state

### DO NOT USE FOR

- Code quality assessment (use `audit-code-quality`)
- Financial projections accuracy (use `audit-financial-model`)
- Security vulnerabilities (use `audit-security`)
- Test coverage analysis (use `audit-test-coverage`)
- Architecture review (use `audit-architecture`)
- Competitive analysis (use `audit-competitive-intel`)

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
7. **Scan for business plan documentation** — `docs/`, `business-plan/`, `README.md`, milestone documents
8. **Analyze git history** — commit frequency, last commit date, branch structure
9. **Select the matching checklist** from Section 4 based on detected technologies and documentation.
10. **If multiple languages detected** (monorepo, polyglot), run all applicable checklists.

---

## Technology-Specific Checklists

### 4.1 Cross-Language Product Roadmap Checklist

- [ ] **Delivery pace vs milestones**: Compare planned delivery dates against git commit history and feature completion
- [ ] **Technical debt impact on timeline**: Identify TODOs, FIXMEs, deprecated patterns that will slow delivery
- [ ] **Feature completeness vs plan**: Cross-reference business plan feature list against actual codebase capabilities
- [ ] **Sprint velocity assessment**: Evaluate commit frequency and scope against roadmap commitments
- [ ] **Resource adequacy**: Estimate effort remaining vs stated team capacity
- [ ] **Dependency risk**: Flag external dependencies (APIs, services, libraries) that are not yet integrated
- [ ] **MVP scope validation**: Verify MVP features are implemented and testable
- [ ] **Documentation currency**: Verify business plan reflects current code state, not outdated aspirations
- [ ] **Module completion tracking**: Verify each planned module/domain has corresponding source code
- [ ] **Test coverage vs plan**: Verify test pyramid matches planned quality level

### 4.2 .NET/C# Specific Checklist

- [ ] **Module registration completeness**: Each planned module has `*ModuleRegistration.cs` with DI services
- [ ] **Wolverine handler coverage**: Each planned command/query has a corresponding handler implementation
- [ ] **Blazor page completeness**: Each planned UI feature has corresponding `.razor` pages under `Features/`
- [ ] **EF migration currency**: Database schema migrations are up to date with entity definitions
- [ ] **Health check coverage**: Each external dependency has a registered health check

### 4.3 Python Specific Checklist

- [ ] **API endpoint coverage**: Each planned API endpoint has corresponding route handler
- [ ] **Django/FastAPI app registration**: Each planned module is registered in the application
- [ ] **Migration currency**: Alembic/Django migrations reflect current model definitions

### 4.4 JavaScript/TypeScript Specific Checklist

- [ ] **Route coverage**: Each planned page has a corresponding route definition
- [ ] **Component library completeness**: Shared components exist for planned UI patterns
- [ ] **API integration**: Frontend API calls match backend endpoint definitions

### 4.5 Go Specific Checklist

- [ ] **Handler coverage**: Each planned endpoint has a corresponding handler function
- [ ] **Middleware registration**: Planned middleware (auth, logging, CORS) is registered in the server
- [ ] **Database migration currency**: Migrations match current model definitions

### 4.6 Java Specific Checklist

- [ ] **Spring Bean registration**: Each planned service has corresponding `@Service`/`@Component`
- [ ] **REST controller coverage**: Each planned endpoint has a corresponding controller method
- [ ] **JPA entity coverage**: Each planned domain model has corresponding entity class

---

## Anti-Patterns

### 5.1 Product Roadmap-Specific Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Business plan promises features with no code | critical | Feature listed in business plan with zero implementation | Stakeholders expect functionality that doesn't exist | Update business plan to reflect actual implementation state |
| 2 | Major timeline gaps | high | Planned delivery dates passed with no corresponding code delivery | Project appears stalled; stakeholders lose confidence | Rebaseline timeline against actual velocity and remaining work |
| 3 | Unresourced roadmap items | high | Features in roadmap with no assigned owner or team capacity | Items never get picked up; roadmap becomes wish list | Assign owners, estimate effort, remove items exceeding capacity |
| 4 | Unrealistic velocity assumptions | high | Sprint velocity assumes no tech debt, no bugs, no scope creep | Actual delivery falls far short of plan | Base velocity on historical data including tech debt and bug fixes |
| 5 | Documentation doesn't match code state | medium | Business plan describes features as "launched" but code shows skeleton implementations | Misleading to stakeholders and investors | Sync documentation with code reality; use "planned"/"in-progress"/"shipped" labels |
| 6 | Missing tests for shipped features | medium | Features marked complete but have zero or minimal test coverage | Quality gate bypass; regressions likely | Establish minimum coverage threshold per feature before marking complete |
| 7 | Stale external dependencies | medium | Business plan relies on integrations not yet implemented | Timeline at risk when integration work is finally discovered | Track integration work as explicit roadmap items |

### 5.2 Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded feature flags | medium | Feature flags hardcoded to `true` instead of configuration-driven | Cannot toggle features; deployment flags stuck | Use configuration system or feature flag service |

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

All audit findings MUST be emitted as structured JSON objects. One finding per object.

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

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Section 8 for escalation rules.
- **file**: Relative path from project root. Use forward slashes. Never absolute paths.
- **line**: 1-based line number where the finding starts.
- **endLine**: Optional. Line where the finding ends (for multi-line issues).
- **description**: Factual, specific. Include the roadmap gap or timeline issue. No hedging.
- **recommendation**: Actionable. Include evidence from codebase analysis.
- **technology**: Primary technology of the finding. Use `CrossLanguage` for documentation-level findings.
- **category**: Primary category. Use the most specific category.

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
| **critical** | Business plan promises features that don't exist in code at all | "AI-powered analytics" in pitch deck with zero ML code |
| **high** | Major timeline gaps, unresourced roadmap items, unrealistic velocity | Delivery date passed with no code; 3-month feature with no team assigned |
| **medium** | Minor feature gaps, moderate technical debt impact, documentation drift | TODO comments in production code; business plan says "launched" for skeleton features |
| **low** | Documentation improvements, minor adjustments, cosmetic issues |

### 8.2 Severity Escalation Triggers

Apply these modifiers to the base severity. Multiple triggers stack (up to critical max).

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding is in investor-facing materials | +1 level | All technologies |
| Finding involves financial/billing data | +1 level | Financial systems |
| Finding affects multiple tenants or users | +1 level | Multi-tenant systems |
| Finding is in a public-facing endpoint | +1 level | Web applications |
| Finding is in authentication/authorization code | +1 level | All technologies |
| Finding is in data encryption/key management | +1 level | All technologies |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding is in non-production/test code only | -1 level (min low) | All technologies |
| Finding has existing mitigating control documented | -1 level (min low) | All technologies |
| Finding is in deprecated code scheduled for removal | -1 level (min low) | All technologies |
| Finding is in documentation/comments | -1 level (min low) | All technologies |

### 8.4 Technology-Specific Severity Adjustments

#### .NET/C#
- Empty handler with TODO → medium (skeleton implementation)
- Missing module registration → high (feature unreachable)
- No EF migration for new entity → medium (schema out of sync)

#### Python
- Missing API endpoint for planned feature → medium (feature gap)
- Empty view function with `pass` → medium (skeleton)
- Missing migration for new model → medium (schema drift)

#### JavaScript/TypeScript
- Missing route for planned page → medium (navigation gap)
- Component with placeholder text → low (UI placeholder)
- Missing API integration in frontend → medium (feature non-functional)

#### Go
- Missing handler for planned endpoint → medium (feature gap)
- Middleware not registered → medium (security/cors gap)

#### Java
- Missing Spring controller → medium (endpoint gap)
- Service with empty method body → medium (skeleton)

#### Rust
- Missing route handler → medium (feature gap)
- Stub implementation returning default → low (skeleton)

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

- **audit-code-quality**: Provides technical debt metrics that impact roadmap timeline
- **audit-test-coverage**: Provides coverage data for feature completeness assessment
- **audit-architecture**: Provides module/structure analysis for completeness tracking

### 9.3 Downstream Consumers

- **audit-competitive-intel**: Uses roadmap findings to assess competitive positioning
- **audit-consolidated**: Aggregates all squad findings into unified report
- **audit-consistency-coherence**: Validates cross-domain alignment between squad findings

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-product-roadmap | audit-competitive-intel | Roadmap focuses on delivery vs plan; competitive focuses on market positioning |
| audit-product-roadmap | audit-financial-model | Roadmap focuses on feature timeline; financial focuses on pricing/revenue projections |
| audit-product-roadmap | audit-code-quality | Roadmap focuses on feature completeness; code-quality focuses on implementation quality |

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
12. **audit-product-roadmap** — Roadmap sync (this squad)
13. **audit-consolidated** — Aggregate all findings
14. **audit-consistency-coherence** — Cross-domain validation

### 9.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read the previous squad's findings from the shared output location.
2. Deduplicate: skip findings already reported by a previous squad.
3. Cross-reference: escalate findings confirmed by multiple squads.
4. Output in the standardized Section 7 format for aggregation.
