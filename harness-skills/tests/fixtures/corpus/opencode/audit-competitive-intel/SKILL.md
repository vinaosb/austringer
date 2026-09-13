---
name: audit-competitive-intel
description: >-
  Competitive intelligence auditing: market positioning analysis, competitive
  differentials review, TAM/SAM/SOM claims validation, trend alignment check,
  and pricing competitiveness verification across business documentation.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "2.0.0"
---

# Competitive Intelligence Audit

## USE FOR

- Market positioning — verify stated position vs actual landscape
- Competitive differentials — validate USPs, moat claims, differentiation
- TAM/SAM/SOM validation — cross-check market size claims against data
- Trend alignment — verify alignment with macro trends (tech, regulatory, consumer)
- Competitive gaps — missing analysis, outdated references
- Pricing competitiveness — claims vs market reality
- Target audience validation — stated market matches product offering
- Moat verification — validate defensibility (tech, network effects, switching costs)
- Market timing — verify timing claims vs market maturity

### DO NOT USE FOR

- Code quality (use `audit-code-quality`)
- Financial model accuracy (use `audit-financial-model`)
- Security vulnerabilities (use `audit-security`)
- Test coverage (use `audit-test-coverage`)
- Architecture (use `audit-architecture`)

---

## Target Discovery

Agent MUST auto-detect project structure. Do NOT hardcode repository names.

### 3.1 Language Detection

| Signal | File Pattern | Detected Language |
|--------|-------------|-------------------|
| C# source files | `*.cs`, `*.csproj`, `*.sln`, `*.slnx` | .NET/C# |
| Python source files | `*.py`, `*.pyi`, `pyproject.toml`, `requirements.txt`, `setup.py`, `Pipfile` | Python |
| JavaScript source files | `*.js`, `*.mjs`, `*.cjs` | JavaScript |
| TypeScript source files | `*.ts`, `*.tsx`, `tsconfig.json` | TypeScript (JS superset) |
| Go source files | `*.go`, `go.mod`, `go.sum` | Go |
| Java source files | `*.java`, `pom.xml`, `build.gradle`, `build.gradle.kts` | Java |
| Rust source files | `*.rs`, `Cargo.toml`, `Cargo.lock` | Rust |
| Ruby source files | `*.rb`, `*.gemspec`, `Gemfile` | Ruby |

### 3.2 Framework Detection

| Signal | File Pattern | Detected Framework |
|--------|-------------|--------------------|
| ASP.NET Core | `Program.cs` + `*.csproj` with `Microsoft.AspNetCore` | ASP.NET Core |
| Blazor | `*.razor`, `*.razor.cs` | Blazor Server/WebAssembly |
| EF Core | `*.csproj` with `Microsoft.EntityFrameworkCore` | Entity Framework Core |
| Django | `manage.py`, `settings.py` with `DJANGO_SETTINGS_MODULE` | Django |
| FastAPI | `*.py` importing `fastapi` | FastAPI |
| Flask | `*.py` importing `flask` | Flask |
| Express | `package.json` listing `express` | Express.js |
| NestJS | `package.json` listing `@nestjs/core` | NestJS |
| Next.js | `package.json` listing `next` | Next.js |
| Spring Boot | `pom.xml`/`build.gradle` with `spring-boot` | Spring Boot |
| Actix | `Cargo.toml` with `actix` | Actix Web |
| Gin | `go.mod` with `gin-gonic` | Gin |

### 3.3 Test Framework Detection

| Signal | Pattern | Detected Framework |
|--------|---------|--------------------|
| xUnit/NUnit/MSTest | `*.csproj` with `xunit`/`nunit`/`MSTest` | .NET test frameworks |
| pytest | `pytest.ini`, `pyproject.toml` with `[tool.pytest]`, `conftest.py` | pytest |
| unittest | `*test*.py` with `unittest.TestCase` | unittest |
| Jest | `jest.config.*`, `package.json` with `jest` | Jest |
| Vitest | `vitest.config.*`, `package.json` with `vitest` | Vitest |
| Go testing | `*_test.go` files | Go testing |
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
| Database migrations | `migrations/`, `Migrations/`, `alembic/`, `db/migrate/` | Schema |
| OTel | `otel-collector-config.yml`, `opentelemetry` in deps | Observability |
| Grafana | `grafana/`, `grafana.ini`, `provisioning/` | Dashboards |
| Prometheus | `prometheus.yml`, `prometheus/` | Metrics |

### 3.6 Discovery Procedure

1. **Scan root** for file patterns.
2. **Identify language** by file count.
3. **Identify frameworks** by config analysis.
4. **Identify test framework** by test dir/config.
5. **Identify CI/CD** by workflow files.
6. **Identify infrastructure** by Docker/ORM/observability.
7. **Scan business docs** — `docs/`, `business-plan/`, `README.md`, investor materials.
8. **Select checklist** from Section 4.
9. **Multiple languages** → run all applicable.

---

## Technology-Specific Checklists

### 4.1 Cross-Language Competitive Intelligence Checklist

- [ ] **Market positioning**: Verify position (leader, challenger, niche) supported by evidence
- [ ] **Competitive differentials**: Validate USP/moat claims against public data
- [ ] **TAM/SAM/SOM validation**: Cross-check against industry reports, census, public sources
- [ ] **Trend alignment**: Verify alignment with macro trends (tech, regulation, consumer)
- [ ] **Competitive gaps**: Flag missing analysis, outdated refs (>2 years)
- [ ] **Pricing competitiveness**: Compare against benchmarks and competitor pricing pages
- [ ] **Target audience**: Verify market definition matches product + GTM strategy
- [ ] **Moat verification**: Validate defensibility (patents, network effects, data moats, switching costs)
- [ ] **Market timing**: Verify timing claims vs market maturity
- [ ] **Competitor completeness**: Top 3-5 competitors with feature-by-feature comparison
- [ ] **Data source citations**: Market claims include credible sources with dates
- [ ] **SWOT/Porter analysis**: Strategic frameworks present and consistent

### 4.2 .NET/C# Specific Checklist

- [ ] **Stack competitiveness**: .NET/Blazor/PostgreSQL claims align with SaaS expectations
- [ ] **Feature parity**: Roadmap claims match actual codebase capabilities
- [ ] **Open-source competitor awareness**: Acknowledge OSS alternatives

### 4.3 Python Specific Checklist

- [ ] **ML/AI claims**: If claiming AI/ML, verify model implementation exists
- [ ] **Data pipeline claims**: Verify processing capabilities match plan descriptions

### 4.4 JavaScript/TypeScript Specific Checklist

- [ ] **Frontend competitiveness**: UX claims match component library quality
- [ ] **Real-time claims**: WebSocket/SignalR claims have implementation

### 4.5 Go Specific Checklist

- [ ] **Performance claims**: Benchmark claims have test evidence
- [ ] **Scalability claims**: Architecture matches growth projections

### 4.6 Java Specific Checklist

- [ ] **Enterprise readiness**: SSO, RBAC, audit claims have implementation
- [ ] **Integration claims**: Third-party integration claims have adapter code

---

## Anti-Patterns

### 5.1 Competitive Intelligence-Specific Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Unsupported TAM/SAM/SOM | high | Market size without citations/methodology | Unverifiable; investor trust loss | Source, date, methodology for every claim |
| 2 | Missing competitive analysis | high | No competitor identification/comparison | Blind to threats; pricing off; unsupported positioning | Competitor matrix with feature comparison |
| 3 | Outdated competitor refs | medium | Data >2 years old | Market shifted; analysis invalid | Refresh quarterly; cite dates |
| 4 | Unrealistic moat claims | medium | "Unassailable lead" with no patent/IP | Investors skeptical; competitors catch up | Verifiable IP, data, or network effects |
| 5 | Target audience mismatch | medium | Market doesn't match product | GTM fails; wasted spend | Validate against feature set and pricing |
| 6 | Cherry-picked pricing | high | Only vs most expensive competitor | Misleading; investors detect bias | Median and range of competitor pricing |
| 7 | No trend analysis | low | Ignores macro trends | Out of touch; blind to tail risks | PESTEL or equivalent |

### 5.2 Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded competitor data | medium | Names/pricing in app code | Stale data; code changes to update | Config or external data source |

---

## Code Examples (Bad/Good Pairs)

### 6.1 .NET/C# Examples

#### Example 1: Market Size Claims With and Without Citations

```markdown
<!-- ❌ BAD: Unsupported market size claim — unverifiable -->
## Market Opportunity
The family office software market is worth $15 billion and growing at 25% annually.

<!-- ✅ GOOD: Cited market size claim with methodology -->
## Market Opportunity
The global wealth management platform market was valued at $2.8B in 2024
(Grand View Research, "Wealth Management Platform Market Report," 2024)
and is projected to reach $6.1B by 2030 (CAGR 13.8%).
Our SAM focuses on family offices with $50M-$500M AUM, estimated at
$340M based on Campden Wealth 2024 survey data (n=1,200 family offices).
```

### 6.2 Python Examples

#### Example 1: Competitive Analysis Structure

```yaml
# ❌ BAD: Competitor listed without comparison
competitors:
  - Salesforce
  - Wealthbox
  - Junxure

# ✅ GOOD: Structured competitive comparison with evidence
competitors:
  - name: Salesforce Financial Services Cloud
    segment: Enterprise wealth management
    pricing: "$150-300/user/month (2024 pricing page, accessed 2024-12)"
    strengths: ["CRM ecosystem", "API marketplace", "brand trust"]
    weaknesses: ["Complex deployment", "Not family-office specific", "High TCO"]
    our_differential: "Purpose-built for family offices, 10x faster onboarding"
    last_verified: "2024-12-01"
```

### 6.3 JavaScript/TypeScript Examples

#### Example 1: Feature Parity Validation

```typescript
// ❌ BAD: Feature claim with no code backing
// Business plan: "Real-time collaboration with co-editing"
// Codebase: No WebSocket/SignalR, no CRDT library

// ✅ GOOD: Feature claim matches implementation
// Business plan: "Real-time notifications via WebSocket"
// Codebase:
// src/Infrastructure/Messaging/ChatHub.cs — SignalR hub
// src/Web/Pages/Shared/NotificationComponent.razor — client component
```

---

## Output Format

All findings as structured JSON. One per object.

### 7.1 Finding Schema

```json
{
  "severity": "critical|high|medium|low",
  "file": "relative/path/to/business-plan.md",
  "line": 42,
  "endLine": 45,
  "description": "What was found. Include the competitive intelligence issue.",
  "recommendation": "Fix with data source reference. Suggest credible sources.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "compliance|architecture|maintainability"
}
```

### 7.2 Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Section 8.
- **file**: Relative path. Forward slashes. Never absolute.
- **line**: 1-based start line.
- **endLine**: Optional end line.
- **description**: Factual, specific. Include the claim. No hedging.
- **recommendation**: Actionable. Suggest data source or methodology.
- **technology**: Primary technology. `CrossLanguage` for doc-level findings.
- **category**: Most specific category.

### 7.3 Summary Report

After all findings:

```json
{
  "summary": {
    "squad": "audit-competitive-intel",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": ["Markdown", ".NET/C#"],
    "totalFindings": 10,
    "bySeverity": {
      "critical": 0,
      "high": 3,
      "medium": 5,
      "low": 2
    },
    "byCategory": {
      "compliance": 6,
      "architecture": 2,
      "maintainability": 2
    },
    "filesScanned": 15,
    "filesWithFindings": 8
  }
}
```

---

## Severity Guidelines

### 8.1 Base Severity Definitions

| Level | Definition | Example |
|-------|-----------|---------|
| **critical** | Materially misleading claims in investor materials | Fabricated market size, fake comparisons, falsified pricing |
| **high** | Unsupported claims, missing analysis, cherry-picked pricing | TAM/SAM without sources, no competitor matrix, selective comparison |
| **medium** | Outdated refs, weak moat arguments, audience mismatch | >2 year old data, unrealistic moat, audience doesn't match product |
| **low** | Formatting, additional sources, minor citation issues |

### 8.2 Severity Escalation Triggers

Modifiers stack (up to critical).

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Investor-facing materials (pitch deck, equity docs) | +1 level | All technologies |
| Financial/billing data | +1 level | Financial systems |
| Multiple tenants affected | +1 level | Multi-tenant |
| Public-facing endpoint | +1 level | Web apps |
| Auth/authorization code | +1 level | All technologies |
| Data encryption/key management | +1 level | All technologies |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Non-production/test only | -1 level (min low) | All technologies |
| Mitigating control documented | -1 level (min low) | All technologies |
| Deprecated code for removal | -1 level (min low) | All technologies |
| Documentation/comments only | -1 level (min low) | All technologies |

### 8.4 Technology-Specific Severity Adjustments

#### .NET/C#
- Feature claim without implementation → medium (roadmap gap)
- Pricing vs competitor discrepancy → high (misleading)

#### Python
- AI/ML claim without model code → high (unsupported capability)

#### JavaScript/TypeScript
- Real-time claim without WebSocket → medium (feature gap)

#### Go
- Performance claim without benchmarks → medium (unsupported)

#### Java
- Enterprise claim without SSO/RBAC → medium (feature gap)

#### Rust
- Safety/security claim without audit evidence → medium (unsupported)

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
audit-consolidated ◄────────────────┘  (consumes all)
audit-consistency-coherence            (consumes all)
```

### 9.2 Upstream Dependencies

- **audit-product-roadmap**: Feature completeness informs competitive gap analysis
- **audit-financial-model**: Pricing data informs competitiveness review

### 9.3 Downstream Consumers

- **audit-consolidated**: Aggregates all findings
- **audit-consistency-coherence**: Validates cross-domain alignment

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-competitive-intel | audit-product-roadmap | CI: market positioning; roadmap: delivery vs plan |
| audit-competitive-intel | audit-financial-model | CI: market claims; financial: pricing logic accuracy |
| audit-competitive-intel | audit-acquisition-metrics | CI: market analysis; acquisition: CAC/LTV metrics |

### 9.5 Recommended Execution Order

For full audit:

1. **audit-dependencies** — Third-party risk baseline
2. **audit-security** — Vulnerability scan
3. **audit-architecture** — Structural analysis
4. **audit-code-quality** — Implementation quality
5. **audit-test-coverage** — Testing assessment
6. **audit-fault-tolerance** — Resilience review
7. **audit-observability** — Telemetry validation
8. **audit-privacy-data** — PII/data handling
9. **audit-cicd** — Pipeline security
10. **audit-license** — License compliance
11. **Domain-specific squads** (financial-model, tax-fiscal, regulatory-*, etc.)
12. **audit-competitive-intel** — Competitive intelligence (this squad)
13. **audit-consolidated** — Aggregate findings
14. **audit-consistency-coherence** — Cross-domain validation

### 9.6 Output Integration

When running sequentially:
1. Read previous findings from shared output.
2. Deduplicate: skip already-reported findings.
3. Cross-reference: escalate confirmed by multiple squads.
4. Output in Section 7 format.
