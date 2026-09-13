---
name: audit-dependencies
description: >-
  Dependency auditing: scans package manifests for known vulnerabilities (CVEs),
  evaluates license compatibility in the dependency tree, identifies transitive
  dependency risks, audits vendored code, and generates software bill of materials
  (SBOM) across multi-language codebases. Produces structured findings with
  severity ratings and concrete remediation steps.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "2.0.0"
---

# Dependency Audit & SBOM

## USE FOR

- NuGet package vulnerability scanning via `dotnet list package --vulnerable` in .NET
- Transitive dependency analysis — unexpected/risky transitive deps entering project
- Central package management (Directory.Packages.props) version drift monitoring in .NET monorepos
- Major version lag — packages behind latest stable (e.g., v5 vs v12)
- Deprecated package detection — superseded alternatives (e.g., Polly.Extensions.Http → Microsoft.Extensions.Http.Resilience)
- Vendored/embedded code license audit — third-party code in project source tree
- Python dependency audit via `pip-audit`, `pip-licenses`, `safety` scanning
- Python lockfile integrity — `requirements.txt` hashes, `pyproject.toml` lock files
- JavaScript/TypeScript dependency audit via `npm audit` / `yarn audit`
- Lock file integrity verification for `package-lock.json` / `yarn.lock` / `pnpm-lock.yaml`
- Go module vulnerability scanning (`govulncheck`) and `go.sum` integrity
- Java Maven/Gradle dependency scanning with OWASP Dependency-Check
- Rust crate vulnerability scanning via `cargo audit` and `Cargo.lock` integrity
- SBOM generation guidance — all dependencies with versions, licenses, known CVEs
- Test SDK freshness — testing packages, coverage tools, infra tooling lagging behind

## DO NOT USE FOR

- Code-level quality analysis (use `audit-code-quality`)
- Code-level security vulnerability scanning (use `audit-security`)
- License compliance of project's own code (use `audit-license`)
- Refactoring recommendations for deprecated API usage (use `audit-refactoring`)
- CI/CD pipeline configuration audit (use `audit-cicd`)

---

## Target Discovery

Agent MUST auto-detect project structure. Do NOT hardcode repository names.

### Language Detection

| Signal | File Pattern | Detected Language |
|--------|-------------|-------------------|
| C# source files | `*.cs`, `*.csproj`, `*.sln`, `*.slnx` | .NET/C# |
| Python source files | `*.py`, `*.pyi`, `pyproject.toml`, `requirements.txt`, `setup.py`, `Pipfile`, `Pipfile.lock` | Python |
| JavaScript source files | `*.js`, `*.mjs`, `*.cjs` | JavaScript |
| TypeScript source files | `*.ts`, `*.tsx`, `tsconfig.json` | TypeScript (treat as JavaScript superset) |
| Go source files | `*.go`, `go.mod`, `go.sum` | Go |
| Java source files | `*.java`, `pom.xml`, `build.gradle`, `build.gradle.kts` | Java |
| Rust source files | `*.rs`, `Cargo.toml`, `Cargo.lock` | Rust |

### Dependency Management Detection

| Signal | File Pattern | Package Manager |
|--------|-------------|-----------------|
| NuGet | `*.csproj` with `PackageReference`, `Directory.Packages.props` | NuGet / Central Package Management |
| pip | `requirements.txt`, `Pipfile`, `Pipfile.lock` | pip / Pipenv |
| Poetry | `pyproject.toml` with `[tool.poetry]`, `poetry.lock` | Poetry |
| npm | `package.json` + `package-lock.json` | npm |
| Yarn | `package.json` + `yarn.lock` | Yarn |
| pnpm | `package.json` + `pnpm-lock.yaml` | pnpm |
| Go modules | `go.mod` + `go.sum` | Go modules |
| Maven | `pom.xml` | Maven |
| Gradle | `build.gradle`, `build.gradle.kts` | Gradle |
| Cargo | `Cargo.toml` + `Cargo.lock` | Cargo |

### Vendored Code Detection

| Signal | File Pattern | Implication |
|--------|-------------|-------------|
| Git submodules | `.gitmodules` | External code tracked as submodule — check version/license |
| Vendored directory | `vendor/`, `third_party/`, `lib/` (not in .gitignore) | Third-party code in source tree — manual audit |
| vendored packages | `openbao/`, `vaultsharp/`, `.tmp/` | Vendored upstream — license compatibility, update cadence |
| Embedded libraries | `*.cs` files with third-party copyright headers | Copied source — check license compliance |

### CI/CD Detection

| Signal | File Pattern | Platform |
|--------|-------------|----------|
| GitLab CI | `.gitlab-ci.yml` | GitLab |
| GitHub Actions | `.github/workflows/*.yml` | GitHub |
| Jenkins | `Jenkinsfile` | Jenkins |
| CircleCI | `.circleci/config.yml` | CircleCI |
| Azure Pipelines | `azure-pipelines.yml`, `.azure-pipelines.yml` | Azure DevOps |

### Infrastructure Detection

| Signal | File Pattern | Component |
|--------|-------------|-----------|
| Docker | `Dockerfile`, `docker-compose.yml`, `docker-compose*.yml` | Containerization |
| Kubernetes | `*.yaml` with `apiVersion` + `kind` | Orchestration |
| Terraform | `*.tf` | Infrastructure as Code |
| Database migrations | `migrations/`, `Migrations/`, `alembic/`, `db/migrate/` | Schema management |

### Discovery Procedure

1. **Scan root directory** for all file patterns above.
2. **Identify package managers** by config file presence.
3. **Identify vendored code** by checking `vendor/`, `third_party/`, `lib/`, `.gitmodules`.
4. **Check for central version management** in .NET (`Directory.Packages.props`) or Go (`go.mod replace directives`).
5. **Run vulnerability scans** — `dotnet list package --vulnerable`, `npm audit`, `pip-audit`, `cargo audit`.
6. **Analyze version drift** — compare current versions with latest stable.
7. **Select matching checklist** from Section 4 based on detected package managers.
8. **If multiple languages detected**, run all applicable checklists.

---

## Technology-Specific Checklists

### .NET/C# Checklist

- [ ] **`dotnet list package --vulnerable`**: Run and check output. Any package with known CVE must have documented mitigation or be upgraded.
- [ ] **Transitive dependency analysis**: Check `dotnet list package --include-transitive` for:
  - Known CVEs in transitive deps
  - Deprecated transitive packages (e.g., old `Polly.Extensions.Http`)
  - Duplicate packages at different versions
- [ ] **Central package management version drift**: In `Directory.Packages.props`:
  - Major version lag: Flag packages >1 major version behind latest stable
  - Common lag candidates: WolverineFx (v5.30 vs v6.3), FluentValidation (v11 vs v12), Refit (v8 vs v10)
  - Version alignment: Same package at different versions across projects
- [ ] **Deprecated packages**: Check for superseded alternatives:
  - `Polly.Extensions.Http` → `Microsoft.Extensions.Http.Resilience` (.NET 8+)
  - `xUnit` v2 → v3 (.NET 8+)
  - `Microsoft.AspNetCore.All` → `Microsoft.AspNetCore.App`
  - Legacy `Newtonsoft.Json` → `System.Text.Json` (new code)
  - `AutoMapper` → Mapster (if project standard)
- [ ] **Package freshness**: Key infra packages within 1 major version of latest:
  - EF Core / Npgsql, WolverineFx, FluentValidation, Mapster, Serilog/sinks, OpenTelemetry, xUnit/coverlet, Playwright
- [ ] **Test SDK and coverage tooling**: Check xUnit/NUnit/MSTest, coverlet, Playwright, FluentAssertions versions
- [ ] **Vendored code license audit**: Check `vendor/`, `third_party/`, `openbao/`, `vaultsharp/`, `.tmp/`:
  - License compatibility with project license
  - Version pinned to known secure release
  - Modifications documented
- [ ] **IsTestProject metadata**: Verify `IsTestProject=true` only on actual test projects, never source libraries
- [ ] **NuGet.config / package source integrity**: Package sources point to trusted feeds. No unauthenticated sources for production packages.

### Python Checklist

- [ ] **`pip-audit` / `safety` scan**: Run and check output. Known CVE must have documented mitigation.
- [ ] **`requirements.txt` freshness**: Flag packages >1 major version behind:
  - Django / FastAPI / Flask, Pydantic / SQLAlchemy / Alembic, httpx / requests, pytest / coverage
- [ ] **Transitive dependency CVEs**: Check `pip-audit` transitive vulnerability reporting.
- [ ] **Lock file integrity**: Poetry (`poetry.lock`) or Pipenv (`Pipfile.lock`):
  - Lock file committed
  - No `*` floating constraints in `pyproject.toml`
  - Upper bounds on major versions (e.g., `fastapi>=0.100,<1.0`)
- [ ] **pyproject.toml dependency lifecycle**: Check deprecated packages, Python version compatibility, optional vs core deps
- [ ] **License compatibility**: Use `pip-licenses` to list dependency licenses. Flag GPL/AGPL in tree.
- [ ] **Test dependency isolation**: Test deps in `[tool.poetry.group.dev.dependencies]` or `[project.optional-dependencies]`

### JavaScript/TypeScript Checklist

- [ ] **`npm audit` / `yarn audit` / `pnpm audit`**: Run and check. Flag high/critical. Check audit fix suggestions.
- [ ] **Lock file integrity**:
  - `package-lock.json` / `yarn.lock` / `pnpm-lock.yaml` committed
  - No conflicting lock files
  - Lock file matches `package.json` specs
- [ ] **Deprecated packages**: Check superseded packages:
  - `moment.js` → `date-fns` / `luxon` / `Intl`
  - `request` → `got` / `ky` / `fetch`
  - `bluebird` → native Promise
  - `lodash` → native methods
  - `gulp` / `grunt` → esbuild, vite, turbopack
- [ ] **Package freshness**: Within 1 major version of latest:
  - React / Next.js / Vue / Angular, TypeScript, ESLint / Prettier, Tailwind CSS, Vite / esbuild / webpack
- [ ] **Transitive dependency analysis**: Check `npm ls --all` for duplicates, CVEs, abandoned packages
- [ ] **ESM/CJS consistency**: Consistent module system. Mixed causes build issues.
- [ ] **TypeScript type packages**: `@types/*` versions match corresponding library.

### Go Checklist

- [ ] **`govulncheck` scan**: Run and check output. Flag known Go module vulnerabilities.
- [ ] **`go.sum` integrity**: Committed and matches `go.mod` specs.
- [ ] **`replace` directives**: Local path `replace` directives break builds elsewhere. Must have documented justification.
- [ ] **Module version freshness**: Within 1 major:
  - Go version, Gin / Echo / Chi, GORM / sqlx, Testify / GoMock
- [ ] **Transitive dependency audit**: Flag unmaintained/abandoned modules.
- [ ] **Vendored directory**: If `vendor/` exists, verify it matches `go.mod`, kept current with `go mod vendor`.

### Java (Maven/Gradle) Checklist

- [ ] **OWASP Dependency-Check**: Review report if configured. Recommend if missing.
- [ ] **Dependabot / Renovate**: Automated dependency updates configured? Flag if manual only.
- [ ] **`pom.xml` / `build.gradle` version drift**:
  - Spring Boot/Spring Framework — within 1 major of latest GA
  - Jackson, Hibernate, Logback — current patches
  - JUnit 5 vs JUnit 4 — recommend migration
- [ ] **Maven BOM / Gradle platform**: Version alignment via BOM/platform plugin, not manual pinning.
- [ ] **Transitive dependency conflicts**: Flag "nearest wins" conflicts.
- [ ] **License audit**: Use `mvn license:add-third-party` or Gradle License plugin.

### Rust Checklist

- [ ] **`cargo audit` scan**: Run and check. Flag known crate vulnerabilities.
- [ ] **`Cargo.lock` integrity**: Lock file committed for binary crates. Library crates may omit.
- [ ] **Dependency freshness**: Within 1 major of latest:
  - Tokio, Axum, Actix-web, Serde, Clap, Tracing, SQLx / Diesel
- [ ] **`cargo deny` check**: License conflicts, duplicate versions, known CVEs.
- [ ] **Unmaintained crates**: Flag crates with no updates >2 years or yanked.

---

## Anti-Patterns

### Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Known CVE ignored without mitigation | critical | Package with CVE (CVSS >= 9.0) used without upgrade or documented mitigation | Direct exploit risk, compliance failure | Upgrade to patched version or document compensating control |
| 2 | Floating versions without upper bound | high | Dep as `>=1.0` without upper bound | Next major version may break build | Pin with upper bound: `>=1.0,<2.0` or exact version |
| 3 | Unpinned transitive dep with CVE | high | Transitive dep at vulnerable version, no control | Indirect vulnerability, no direct fix | Add explicit direct dependency at safe version or use override |
| 4 | Outdated >1 major version | medium | Package v5 when latest v12 | Missing security patches, features | Upgrade; plan incremental migration |
| 5 | Uncommitted lock file | medium | Lock file not in VCS | Non-reproducible builds | Commit lock files |

### .NET/C# Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | src project marked `IsTestProject=true` | high | Library project has test metadata | Pulls xunit, Moq, coverlet into library; breaks packing | Only set on actual test projects |
| 2 | Deprecated Polly.Extensions.Http | high | Using legacy Polly package | Old Polly version, missing new resilience patterns | Migrate to `Microsoft.Extensions.Http.Resilience` (.NET 8+) |
| 3 | Central package version drift | medium | Same package at different versions across projects | Inconsistent behavior | Use `Directory.Packages.props` with `ManagePackageVersionsCentrally="true"` |
| 4 | Major version lag on core package | medium | Infra package behind 2+ major versions | Missing features, potential CVEs | Plan incremental upgrade |
| 5 | Vendored code without license check | medium | Third-party code without license verification | License violation risk | Check license header, document in LICENSE/NOTICE |
| 6 | xUnit v2 in modern project | low | .NET 8+ project on xUnit v2 | Missing v3 features | Upgrade to xUnit v3 |

### Python Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Floating deps in production | high | No version pin in requirements.txt | Unpredictable deployments | Pin exact versions or use lock file |
| 2 | No lock file | medium | requirements.txt without hashes/lock | Non-reproducible builds, supply chain risk | Use Poetry or pip-tools with hash pinning |
| 3 | GPL/AGPL dep in proprietary project | high | GPL license in commercial project | License violation, legal risk | Replace with MIT/Apache-2.0 alternative |
| 4 | Stale test tooling | low | pytest >1 major behind | Missing features, potential CVEs | Upgrade to latest stable |

### JavaScript/TypeScript Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Abandoned dependency | high | Package no updates >2 years | Unpatched CVEs, no support | Replace with maintained alternative |
| 2 | Mixed lock files | medium | Both package-lock.json and yarn.lock | Build confusion, CI uses wrong lock | Pick one manager, delete other lock |
| 3 | lodash for simple operations | low | Importing whole lodash for .map()/.filter() | Unnecessary bundle size | Use native methods |
| 4 | Missing lock file integrity check | medium | Lock updated without reviewing changes | Malicious injection goes unnoticed | Review lock diffs, use `npm ci`/`yarn --frozen-lockfile` in CI |

---

## Code Examples (Bad/Good Pairs)

### .NET/C# Example 1: Central Package Version Bump

```xml
<!-- BAD: Directory.Packages.props with stale packages, version drift -->
<Project>
  <PropertyGroup>
    <ManagePackageVersionsCentrally>true</ManagePackageVersionsCentrally>
  </PropertyGroup>
  <ItemGroup>
    <PackageVersion Include="WolverineFx" Version="5.30.0" />
    <PackageVersion Include="FluentValidation" Version="11.9.0" />
    <PackageVersion Include="Refit" Version="8.0.0" />
    <PackageVersion Include="xunit" Version="2.9.3" />
  </ItemGroup>
</Project>

<!-- GOOD: Directory.Packages.props with current packages -->
<Project>
  <PropertyGroup>
    <ManagePackageVersionsCentrally>true</ManagePackageVersionsCentrally>
  </PropertyGroup>
  <ItemGroup>
    <PackageVersion Include="WolverineFx" Version="6.3.0" />
    <PackageVersion Include="FluentValidation" Version="12.0.0" />
    <PackageVersion Include="Refit" Version="10.0.0" />
    <PackageVersion Include="xunit.v3" Version="1.0.0" />
    <PackageVersion Include="xunit.runner.visualstudio" Version="3.0.0" />
  </ItemGroup>
</Project>
```

### .NET/C# Example 2: Proper IsTestProject Scoping

```xml
<!-- BAD: Source project marked as test project -->
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
    <IsTestProject>true</IsTestProject>
  </PropertyGroup>
  <ItemGroup>
    <PackageReference Include="xunit" Version="2.9.3" />
    <PackageReference Include="Moq" Version="4.20.72" />
    <PackageReference Include="Microsoft.NET.Test.Sdk" Version="17.12.0" />
  </ItemGroup>
</Project>

<!-- GOOD: Source project clean, test deps in test project -->
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
  </PropertyGroup>
  <ItemGroup>
    <PackageReference Include="FluentValidation" />
  </ItemGroup>
</Project>

<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
    <IsTestProject>true</IsTestProject>
    <TestingPlatformDotnetTestSupport>true</TestingPlatformDotnetTestSupport>
  </PropertyGroup>
  <ItemGroup>
    <PackageReference Include="xunit.v3" Version="1.0.0" />
    <PackageReference Include="Moq" Version="4.20.72" />
    <PackageReference Include="Microsoft.NET.Test.Sdk" Version="17.12.0" />
    <PackageReference Include="coverlet.collector" Version="6.0.4" />
  </ItemGroup>
</Project>
```

### .NET/C# Example 3: Deprecated Polly HTTP Migration

```csharp
// BAD: Legacy Polly.Extensions.Http — old Polly + transitive deps
services.AddHttpClient("ExternalApi")
    .AddTransientHttpErrorPolicy(policy =>
        policy.WaitAndRetryAsync(3, retryAttempt => TimeSpan.FromSeconds(Math.Pow(2, retryAttempt))));

// GOOD: Microsoft.Extensions.Http.Resilience — modern, less transitive deps
services.AddHttpClient("ExternalApi")
    .AddStandardResilienceHandler();
```

### Python Example 1: Floating Dependencies

```python
# BAD: Floating versions — unpredictable deployments
fastapi>=0.100.0
sqlalchemy>=2.0.0
httpx>=0.25.0

# GOOD: Pinned versions
fastapi==0.115.6
sqlalchemy==2.0.36
httpx==0.28.1
```

### JavaScript/TypeScript Example 1: npm Audit Findings

```bash
# BAD: Critical/high findings ignored
$ npm audit
# critical  lodash 4.17.20
# high     axios 0.21.1

# GOOD: Findings addressed
$ npm audit fix
$ npm install lodash@4.17.21 axios@1.7.0
$ npm audit  # 0 vulnerabilities
```

---

## Output Format

All audit findings MUST be emitted as structured JSON objects. One finding per object.

### Finding Schema

```json
{
  "severity": "critical|high|medium|low",
  "file": "relative/path/to/package-manifest",
  "line": 1,
  "endLine": 5,
  "description": "Dependency issue found. Include package name, current version, known CVE.",
  "recommendation": "Specific fix: upgrade target version, migration path, or documented mitigation.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "security|maintainability|compliance|performance"
}
```

### Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Severity Guidelines.
- **file**: Relative path from project root. Forward slashes. Never absolute paths.
- **line**: 1-based line number where issue starts.
- **endLine**: Optional. Line where issue ends.
- **description**: Factual, specific. Include package name, version, CVEs, license.
- **recommendation**: Actionable. Include target version and migration notes.
- **technology**: Primary technology of finding.
- **category**: `security` for CVE, `maintainability` for version drift, `compliance` for license.

### Summary Report

After all findings, emit summary:

```json
{
  "summary": {
    "squad": "audit-dependencies",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#", "TypeScript"],
    "totalFindings": 18,
    "bySeverity": {
      "critical": 2,
      "high": 5,
      "medium": 8,
      "low": 3
    },
    "byCategory": {
      "security": 8,
      "maintainability": 7,
      "compliance": 3
    },
    "filesScanned": 12,
    "filesWithFindings": 8
  }
}
```

---

## Severity Guidelines

### Base Severity Definitions

| Level | Definition | Dependency Audit Example |
|-------|-----------|------------------------|
| **critical** | Known CVE CVSS >= 9.0, or GPL in proprietary code | Package with RCE CVE, AGPL in commercial product |
| **high** | Known CVE CVSS 7-9, deprecated core package, src marked as test | Active CVE with exploit, Polly.Extensions.Http in prod, IsTestProject on library |
| **medium** | Package >1 major behind, uncommitted lock file, minor license concerns | WolverineFx v5 vs v6, no lock file, vendored code without header |
| **low** | Minor version behind, non-critical update, test tooling staleness | Coverlet minor behind, deprecated dev-only package |

### Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding has known CVE with published exploit | +1 level | All technologies |
| Finding is in production/run-time dep (not dev/test) | +1 level | All technologies |
| Finding affects auth/authorization library | +1 level | All technologies |
| Finding is in encryption/cryptography package | +1 level | All technologies |
| Package is unmaintained >2 years | +1 level | All technologies |

### Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding is in test-only dependency | -1 level (min low) | All technologies |
| Finding has documented compensating control | -1 level (min low) | All technologies |
| CVE is disputed or no known exploit | -1 level (min low) | All technologies |
| Package scheduled for removal in next sprint | -1 level (min low) | All technologies |

### Technology-Specific Severity Adjustments

#### .NET/C#
- Known CVE in production NuGet → critical
- Central package version drift >1 major → medium
- src project `IsTestProject=true` → high
- Deprecated `Polly.Extensions.Http` → high

#### Python
- Floating version without upper bound → high
- GPL dep in proprietary project → critical
- No lock file in production → medium

#### JavaScript/TypeScript
- Abandoned package with known CVE → critical
- Mixed lock files → medium
- Deprecated package with replacement (e.g., `moment.js`) → medium

---

## Integration Notes

### Squad Dependency Graph

```
audit-dependencies ◄─────────────────┐  (foundation for security and license audits)
audit-security ◄─────────────────────┤  (consumes CVE data)
audit-license ◄──────────────────────┤  (consumes license data)
audit-refactoring ◄──────────────────┤  (consumes deprecated package data)
audit-code-quality ──────────────────┤
audit-architecture ──────────────────┤
audit-test-coverage ─────────────────┤
audit-consolidated ◄─────────────────┘  (consumes all)
audit-consistency-coherence            (consumes all)
```

### Upstream Dependencies

- *(none)* — Foundation squad, does not consume other squad findings.

### Downstream Consumers

- **audit-security**: Consumes CVE findings to correlate with code-level vulns.
- **audit-license**: Consumes license findings for compliance report.
- **audit-refactoring**: Consumes deprecated packages for migration paths.
- **audit-consolidated**: Aggregates all findings.
- **audit-consistency-coherence**: Validates cross-domain alignment.

### Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-dependencies | audit-security | Dependencies: CVE in third-party packages; Security: code-level vulns |
| audit-dependencies | audit-license | Dependencies: package licenses; License: project's own code license |
| audit-dependencies | audit-refactoring | Dependencies: deprecated packages; Refactoring: API-level migration |

### Recommended Execution Order

Run first in full audit:

1. **audit-dependencies** ← This squad (foundation — establish third-party risk baseline)
2. **audit-security** — Code-level vuln scan (uses CVE data)
3. **audit-architecture** — Structural analysis
4. **audit-code-quality** — Implementation quality
5. **audit-refactoring** — Modernization (uses deprecated package data)
6. **audit-license** — License compliance (uses license data)
7. Then all remaining squads

### Output Integration

When squads run sequentially:
1. This squad runs first — no upstream findings to read.
2. Output early findings in standardized Section 7 format.
3. Subsequent squads consume this squad's findings.
4. Each finding self-contained with context for downstream consumers.