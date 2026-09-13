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

- Market positioning analysis — verify stated market position against actual competitive landscape
- Competitive differentials review — validate unique selling propositions, moat claims, differentiation
- Business plan market claims validation — cross-check TAM/SAM/SOM claims against available data
- Trend alignment check — verify business aligns with macro trends (technology, regulatory, consumer)
- Competitive gap identification — missing competitive analysis, outdated competitor references
- Pricing competitiveness — pricing claims vs market reality
- Target audience validation — stated target market matches product offering
- Moat claims verification — validate defensibility claims (technology, network effects, switching costs)
- Market timing analysis — verify business timing claims align with actual market maturity

### DO NOT USE FOR

- Code quality assessment (use `audit-code-quality`)
- Financial model accuracy (use `audit-financial-model`)
- Security vulnerabilities (use `audit-security`)
- Test coverage analysis (use `audit-test-coverage`)
- Architecture review (use `audit-architecture`)

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
7. **Scan for business plan documentation** — `docs/`, `business-plan/`, `README.md`, investor materials
8. **Select the matching checklist** from Section 4 based on detected technologies and documentation.
9. **If multiple languages detected** (monorepo, polyglot), run all applicable checklists.

---

## Technology-Specific Checklists

### 4.1 Cross-Language Competitive Intelligence Checklist

- [ ] **Market positioning analysis**: Verify stated market position (leader, challenger, niche) is supported by evidence
- [ ] **Competitive differentials review**: Validate USP/moat claims against publicly available competitive data
- [ ] **TAM/SAM/SOM validation**: Cross-check market size claims against industry reports, census data, or public sources
- [ ] **Trend alignment check**: Verify business aligns with current macro trends in technology, regulation, and consumer behavior
- [ ] **Competitive gap identification**: Flag missing competitive analysis sections, outdated competitor references (>2 years old)
- [ ] **Pricing competitiveness**: Compare stated pricing against market benchmarks and competitor pricing pages
- [ ] **Target audience validation**: Verify target market definition matches product capabilities and go-to-market strategy
- [ ] **Moat claims verification**: Validate defensibility claims (technology patents, network effects, data moats, switching costs)
- [ ] **Market timing analysis**: Verify business timing claims align with market maturity indicators
- [ ] **Competitor completeness**: Verify top 3-5 competitors are identified with feature-by-feature comparison
- [ ] **Data source citations**: Market data claims include credible sources with dates
- [ ] **SWOT/Porter analysis**: Strategic analysis frameworks present and internally consistent

### 4.2 .NET/C# Specific Checklist

- [ ] **Technology stack competitiveness**: Verify .NET/Blazor/PostgreSQL stack claims align with modern SaaS expectations
- [ ] **Feature parity assessment**: Verify feature roadmap claims match actual codebase capabilities
- [ ] **Open-source competitor awareness**: Acknowledge open-source alternatives that could disrupt the model

### 4.3 Python Specific Checklist

- [ ] **ML/AI claims validation**: If claiming AI/ML capabilities, verify actual model implementation exists in codebase
- [ ] **Data pipeline claims**: Verify data processing capabilities match business plan descriptions

### 4.4 JavaScript/TypeScript Specific Checklist

- [ ] **Frontend competitiveness**: Verify frontend UX claims match actual component library quality
- [ ] **Real-time feature claims**: Verify WebSocket/SignalR claims have corresponding implementation

### 4.5 Go Specific Checklist

- [ ] **Performance claims**: Verify performance benchmark claims have corresponding test evidence
- [ ] **Scalability claims**: Verify scalability architecture matches business plan growth projections

### 4.6 Java Specific Checklist

- [ ] **Enterprise readiness claims**: Verify enterprise feature claims (SSO, RBAC, audit) have implementation
- [ ] **Integration claims**: Verify third-party integration claims have corresponding adapter code

---

## Anti-Patterns

### 5.1 Competitive Intelligence-Specific Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Unsupported TAM/SAM/SOM claims | high | Market size numbers without citations or methodology | Investors lose trust; claims unverifiable; potentially misleading | Include source, date, methodology for every market size claim |
| 2 | Missing competitive analysis entirely | high | No competitor identification or comparison | Blind to market threats; pricing may be off; positioning claims unsupported | Include competitor matrix with feature comparison |
| 3 | Outdated competitor references | medium | Competitor analysis references data >2 years old | Market has shifted; competitor may have pivoted; analysis invalid | Refresh competitive data quarterly; cite dates |
| 4 | Unrealistic moat claims | medium | Claims "unassailable technology lead" with no patent or IP | Investors skeptical; competitors may catch up quickly | Ground moat claims in verifiable IP, data, or network effects |
| 5 | Target audience mismatch | medium | Stated market doesn't match product capabilities | Go-to-market strategy fails; wasted marketing spend | Validate target audience against actual feature set and pricing |
| 6 | Cherry-picked pricing comparison | high | Only compares against most expensive competitor | Misleading pricing position; investors detect bias | Compare against median and range of competitor pricing |
| 7 | No trend analysis | low | Business plan ignores macro trends (regulatory, tech, consumer) | Appears out of touch; blind to tail risks | Include PESTEL or equivalent trend analysis |

### 5.2 Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded competitor data in source | medium | Competitor names/pricing hardcoded in application code | Data becomes stale; requires code changes to update | Store competitive data in configuration or external data source |

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
// ❌ BAD: Feature claim in business plan with no code backing
// Business plan states: "Real-time collaboration with co-editing"
// Codebase has: No WebSocket/SignalR implementation, no CRDT library

// ✅ GOOD: Feature claim matches implementation
// Business plan states: "Real-time notifications via WebSocket"
// Codebase has:
// src/Infrastructure/Messaging/ChatHub.cs — SignalR hub
// src/Web/Pages/Shared/NotificationComponent.razor — client component
```

---

## Output Format

All audit findings MUST be emitted as structured JSON objects. One finding per object.

### 7.1 Finding Schema

```json
{
  "severity": "critical|high|medium|low",
  "file": "relative/path/to/business-plan.md",
  "line": 42,
  "endLine": 45,
  "description": "What was found and why it matters. Include the competitive intelligence issue.",
  "recommendation": "Specific fix with data source reference. Suggest credible sources.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "compliance|architecture|maintainability"
}
```

### 7.2 Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Section 8 for escalation rules.
- **file**: Relative path from project root. Use forward slashes. Never absolute paths.
- **line**: 1-based line number where the finding starts.
- **endLine**: Optional. Line where the finding ends (for multi-line issues).
- **description**: Factual, specific. Include the claim being questioned. No hedging.
- **recommendation**: Actionable. Include suggested data source or methodology.
- **technology**: Primary technology of the finding. Use `CrossLanguage` for documentation-level findings.
- **category**: Primary category. Use the most specific category.

### 7.3 Summary Report

After all findings, emit a summary:

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
| **critical** | Materially misleading competitive claims in investor-facing materials | Fabricated market size, fake competitor comparisons, falsified pricing |
| **high** | Unsupported market size claims, missing competitive analysis, cherry-picked pricing | TAM/SAM without sources, no competitor matrix, selective pricing comparison |
| **medium** | Outdated competitor references, weak moat arguments, audience mismatch | >2 year old competitive data, unrealistic moat claims, target audience doesn't match product |
| **low** | Formatting improvements, additional data sources suggested, minor citation issues |

### 8.2 Severity Escalation Triggers

Apply these modifiers to the base severity. Multiple triggers stack (up to critical max).

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding is in investor-facing materials (pitch deck, equity documents) | +1 level | All technologies |
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
- Feature claim with no implementation → medium (roadmap gap)
- Pricing claim vs competitor pricing page discrepancy → high (misleading)

#### Python
- AI/ML claim with no model code → high (unsupported capability claim)

#### JavaScript/TypeScript
- Real-time claim without WebSocket implementation → medium (feature gap)

#### Go
- Performance claim without benchmark tests → medium (unsupported claim)

#### Java
- Enterprise claim without SSO/RBAC implementation → medium (feature gap)

#### Rust
- Safety/security claim without audit evidence → medium (unsupported claim)

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

- **audit-product-roadmap**: Feature completeness findings inform competitive gap analysis
- **audit-financial-model**: Pricing data informs pricing competitiveness review

### 9.3 Downstream Consumers

- **audit-consolidated**: Aggregates all squad findings into unified report
- **audit-consistency-coherence**: Validates cross-domain alignment between squad findings

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-competitive-intel | audit-product-roadmap | Competitive intel focuses on market positioning; roadmap focuses on delivery vs plan |
| audit-competitive-intel | audit-financial-model | Competitive intel focuses on market claims; financial focuses on pricing logic accuracy |
| audit-competitive-intel | audit-acquisition-metrics | Competitive intel focuses on market analysis; acquisition focuses on CAC/LTV metrics |

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
12. **audit-competitive-intel** — Competitive intelligence (this squad)
13. **audit-consolidated** — Aggregate all findings
14. **audit-consistency-coherence** — Cross-domain validation

### 9.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read the previous squad's findings from the shared output location.
2. Deduplicate: skip findings already reported by a previous squad.
3. Cross-reference: escalate findings confirmed by multiple squads.
4. Output in the standardized Section 7 format for aggregation.
