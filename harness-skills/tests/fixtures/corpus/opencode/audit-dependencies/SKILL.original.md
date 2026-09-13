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

- NuGet package vulnerability scanning via `dotnet list package --vulnerable` in .NET projects
- Transitive dependency analysis — unexpected or risky transitive dependencies entering the project
- Central package management (Directory.Packages.props) version drift monitoring in .NET monorepos
- Major version lag identification — packages significantly behind latest stable (e.g., package at v5 vs v12)
- Deprecated package detection — packages superseded by newer alternatives (e.g., Polly.Extensions.Http → Microsoft.Extensions.Http.Resilience)
- Vendored/embedded code license audit — third-party code copied into the project source tree
- Python dependency audit via `pip-audit`, `pip-licenses`, and `safety` scanning
- Python lockfile integrity — `requirements.txt` hashes, `pyproject.toml` lock files
- JavaScript/TypeScript dependency audit via `npm audit` / `yarn audit`
- Lock file integrity verification for `package-lock.json` / `yarn.lock` / `pnpm-lock.yaml`
- Go module vulnerability scanning (`govulncheck`) and `go.sum` integrity
- Java Maven/Gradle dependency scanning with OWASP Dependency-Check
- Rust crate vulnerability scanning via `cargo audit` and `Cargo.lock` integrity
- SBOM generation guidance — listing all dependencies with versions, licenses, and known CVEs
- Test SDK freshness — testing packages, coverage tools, and infrastructure tooling lagging behind

## DO NOT USE FOR

- Code-level quality analysis (use `audit-code-quality`)
- Code-level security vulnerability scanning (use `audit-security`)
- License compliance of the project's own code (use `audit-license`)
- Refactoring recommendations for deprecated API usage (use `audit-refactoring`)
- CI/CD pipeline configuration audit (use `audit-cicd`)

---

## Target Discovery

The agent MUST auto-detect the project structure. Do NOT hardcode repository names.

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
| NuGet | `*.csproj` containing `PackageReference`, `Directory.Packages.props` | NuGet / Central Package Management |
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
| Vendored directory | `vendor/`, `third_party/`, `lib/` (not in .gitignore) | Third-party code copied into source tree — manual audit needed |
| vendored packages | `openbao/`, `vaultsharp/`, `.tmp/` | Vendored upstream — license compatibility, update cadence |
| Embedded libraries | `*.cs` files with third-party copyright headers | Copied source — check for license compliance |

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
2. **Identify package managers** by config file presence (NuGet, pip, npm, Go modules, Maven, Gradle, Cargo).
3. **Identify vendored code** by checking `vendor/`, `third_party/`, `lib/` and `.gitmodules`.
4. **Check for central version management** in .NET (`Directory.Packages.props`) or Go (`go.mod replace directives`).
5. **Run vulnerability scans** — `dotnet list package --vulnerable`, `npm audit`, `pip-audit`, or `cargo audit`.
6. **Analyze version drift** by comparing current package versions with latest stable.
7. **Select matching checklist** from Section 4 based on detected package managers.
8. **If multiple languages detected**, run all applicable checklists.

---

## Technology-Specific Checklists

### .NET/C# Checklist

- [ ] **`dotnet list package --vulnerable`**: Run and check output. Any package with known CVE must have a documented mitigation or be upgraded.
- [ ] **Transitive dependency analysis**: Check `dotnet list package --include-transitive` for unexpected transitive packages. Review each for:
  - Known CVEs in transitive dependencies
  - Deprecated packages pulled in transitively (e.g., old `Polly.Extensions.Http`)
  - Duplicate packages resolved to different versions
- [ ] **Central package management version drift**: In `Directory.Packages.props`:
  - Major version lag: Check each package against latest stable. Flag packages >1 major version behind.
  - Common lag candidates: WolverineFx (e.g., v5.30 vs latest v6.3), FluentValidation (v11 vs v12), Refit (v8 vs v10)
  - Version alignment: Same package referenced with different versions across projects
- [ ] **Deprecated packages**: Check for packages superseded by newer alternatives:
  - `Polly.Extensions.Http` → `Microsoft.Extensions.Http.Resilience` (.NET 8+)
  - `xUnit` v2 → v3 (if project is on .NET 8+)
  - `Microsoft.AspNetCore.All` → `Microsoft.AspNetCore.App`
  - Legacy `Newtonsoft.Json` → `System.Text.Json` (for new code)
  - `AutoMapper` → Mapster (if project standard is Mapster)
- [ ] **Package freshness**: Key infra packages should be within 1 major version of latest:
  - EF Core / Npgsql
  - WolverineFx
  - FluentValidation
  - Mapster
  - Serilog / sinks
  - OpenTelemetry packages
  - xUnit / coverlet (test tooling)
  - Playwright (E2E)
- [ ] **Test SDK and coverage tooling**: Check versions of:
  - xUnit / NUnit / MSTest SDKs — are they current?
  - coverlet / Microsoft.CodeCoverage — latest stable
  - Playwright / Microsoft.Playwright — latest stable
  - FluentAssertions (if used) — check for major version lag
- [ ] **Vendored code license audit**: Check `vendor/`, `third_party/`, `openbao/`, `vaultsharp/`, `.tmp/`:
  - License compatibility with project license (MIT, Apache-2.0 compatible)
  - Version pinned to known secure release
  - Any modifications documented
- [ ] **IsTestProject metadata**: Verify `IsTestProject=true` is NOT set on source/library projects — only on actual test projects. Source projects with test metadata pull test dependencies into library.
- [ ] **NuGet.config / package source integrity**: Check that package sources point to trusted feeds (nuget.org, Azure Artifacts, private feeds). No unauthenticated sources for production packages.

### Python Checklist

- [ ] **`pip-audit` / `safety` scan**: Run and check output. Any package with known CVE must have documented mitigation.
- [ ] **`requirements.txt` freshness**: Check pinned versions against latest stable. Flag packages >1 major version behind:
  - Django / FastAPI / Flask frameworks
  - Pydantic / SQLAlchemy / Alembic
  - httpx / requests
  - pytest / coverage (test tooling)
- [ ] **Transitive dependency CVEs**: Check `pip-audit` transitive vulnerability reporting. Flag packages with indirect vulnerabilities.
- [ ] **Lock file integrity**: If using Poetry (`poetry.lock`) or Pipenv (`Pipfile.lock`):
  - Lock file committed to version control
  - No `*` (floating) version constraints in `pyproject.toml`
  - Upper bounds on major dependency versions (e.g., `fastapi>=0.100,<1.0`)
- [ ] **pyproject.toml dependency lifecycle**: Check for:
  - Deprecated packages (e.g., `pylint` plugins, old `pytest` plugins)
  - Python version compatibility (supports project target Python version)
  - Optional dependencies separated from core dependencies
- [ ] **License compatibility**: Use `pip-licenses` to list all dependency licenses. Flag GPL/AGPL licenses in dependency tree for review.
- [ ] **Test dependency isolation**: Test dependencies (`pytest`, `pytest-cov`, `pytest-asyncio`, `mypy`) in `[tool.poetry.group.dev.dependencies]` or `[project.optional-dependencies]`, not in core dependencies.

### JavaScript/TypeScript Checklist

- [ ] **`npm audit` / `yarn audit` / `pnpm audit`**: Run and check output. Flag high/critical vulnerabilities. Check for audit resolution (`npm audit fix` suggestions).
- [ ] **Lock file integrity**:
  - `package-lock.json` / `yarn.lock` / `pnpm-lock.yaml` committed to version control
  - No conflicting lock files (multiple package managers)
  - Lock file matches `package.json` dependency specifications
- [ ] **Deprecated packages**: Check for packages superseded or deprecated:
  - `moment.js` → `date-fns` / `luxon` / native `Intl`
  - `request` → `got` / `ky` / native `fetch`
  - `bluebird` → native Promise
  - `lodash` → native Array/Object methods (where possible)
  - `gulp` / `grunt` → modern build tools (esbuild, vite, turbopack)
- [ ] **Package freshness**: Key packages should be within 1 major version of latest:
  - React / Next.js / Vue / Angular versions
  - TypeScript version
  - ESLint / Prettier / related plugins
  - Tailwind CSS / PostCSS
  - Vite / esbuild / webpack
- [ ] **Transitive dependency analysis**: Check `npm ls --all` for:
  - Duplicate packages resolved to different versions
  - Packages pulled in that have own high-severity CVEs
  - Abandoned/unmaintained packages in the tree
- [ ] **ESM/CJS consistency**: All packages should have consistent module system. Mixed ESM/CJS packages can cause build issues.
- [ ] **TypeScript type packages**: `@types/*` packages should match the version of the corresponding library.

### Go Checklist

- [ ] **`govulncheck` scan**: Run and check output. Flag any known vulnerabilities in Go modules.
- [ ] **`go.sum` integrity**: Verify `go.sum` is committed and matches `go.mod` module specifications.
- [ ] **`replace` directives**: Check `go.mod` for `replace` directives pointing to local paths. These break builds on other machines. Must have documented justification.
- [ ] **Module version freshness**: Key packages within 1 major version of latest:
  - Go version itself (check `go.mod` `go` directive)
  - Gin / Echo / Chi (web frameworks)
  - GORM / sqlx (database)
  - Testify / GoMock (testing)
- [ ] **Transitive dependency audit**: Unexpected packages in `go.mod` from transitive dependencies. Flag unmaintained or abandoned modules.
- [ ] **Vendored directory**: If `vendor/` directory exists, verify it matches `go.mod` and is kept up to date with `go mod vendor`.

### Java (Maven/Gradle) Checklist

- [ ] **OWASP Dependency-Check**: If configured, review report. If not configured, recommend enabling it.
- [ ] **Dependabot / Renovate**: Check if automated dependency updates are configured. Flag if only manual updates.
- [ ] **`pom.xml` / `build.gradle` version drift**:
  - Spring Boot / Spring Framework versions — within 1 major of latest GA
  - Jackson, Hibernate, Logback versions — current patch releases
  - JUnit 5 vs JUnit 4 — recommend migration if still on JUnit 4
- [ ] **Maven BOM / Gradle platform**: Check that version alignment uses BOM (Bill of Materials) or platform plugin, not manual version pinning.
- [ ] **Transitive dependency conflicts**: Flag cases where Maven's "nearest wins" strategy might pull unexpected versions.
- [ ] **License audit**: Use `mvn license:add-third-party` or Gradle License plugin to generate third-party license report.

### Rust Checklist

- [ ] **`cargo audit` scan**: Run and check output. Flag known vulnerabilities in crates.
- [ ] **`Cargo.lock` integrity**: Lock file committed for binary crates. Library crates may omit it.
- [ ] **Dependency freshness**: Key crates within 1 major version of latest:
  - Tokio, Axum, Actix-web (async runtime/web framework)
  - Serde, Clap, Tracing (utilities)
  - SQLx / Diesel (database)
- [ ] **`cargo deny` check**: If configured, review for license conflicts, duplicate crate versions, and known CVEs.
- [ ] **Unmaintained crates**: Flag crates with no recent updates (>2 years) or that have been yanked.

---

## Anti-Patterns

### Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Known CVE ignored without mitigation | critical | Package with known CVE (CVSS >= 9.0) used without upgrade or documented mitigation | Direct exploit risk, audit fails compliance requirements | Upgrade to patched version or document compensating control |
| 2 | Floating versions without upper bound | high | Dependency specified as `>=1.0` without upper bound | Next major version may include breaking changes, break build | Pin with upper bound: `>=1.0,<2.0` or exact version |
| 3 | Unpinned transitive dependency with CVE | high | Transitive dependency dragged in at vulnerable version, no control over it | Indirect vulnerability with no direct fix available | Add explicit direct dependency at safe version, or use override mechanism |
| 4 | Outdated >1 major version | medium | Package at version 5.x when latest is 12.x | Missing security patches, performance improvements, and new features | Upgrade to latest; if breaking changes, plan incremental migration |
| 5 | Uncommitted lock file | medium | `package-lock.json`, `yarn.lock`, or `Cargo.lock` not in version control | Non-reproducible builds, unpredictable version resolution | Commit lock files for reproducible builds |

### .NET/C# Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | src project marked `IsTestProject=true` | high | Source/library project has `<IsTestProject>true</IsTestProject>` in `.csproj` | Pulls xunit, Moq, coverlet into library; breaks packing and adds unnecessary transitive deps | Only set `IsTestProject=true` on actual test projects |
| 2 | Deprecated Polly.Extensions.Http | high | Using `Polly.Extensions.Http` instead of `Microsoft.Extensions.Http.Resilience` | Legacy package, pulls old Polly version, missing new resilience patterns | Migrate to `Microsoft.Extensions.Http.Resilience` (.NET 8+) |
| 3 | Central package version drift | medium | Same package version specified differently across projects in a monorepo | Inconsistent behavior, unexpected version resolution | Use `Directory.Packages.props` with `ManagePackageVersionsCentrally="true"` |
| 4 | Major version lag on core package | medium | Critical infra package (WolverineFx, FluentValidation, Refit) behind by 2+ major versions | Missing features, may have known CVEs, upgrade becomes harder over time | Plan incremental upgrade; test breaking changes one major at a time |
| 5 | Vendored code without license check | medium | Third-party code copied into source tree without verifying license compatibility | License violation risk (GPL/AGPL in proprietary code) | Check license in vendor header; document in LICENSE or NOTICE file |
| 6 | xUnit v2 in modern project | low | Project on .NET 8+ still using xUnit v2 | Missing v3 features (better assertions, analyzer integration) | Upgrade to xUnit v3 for modern projects |

### Python Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Floating dependencies in production | high | `fastapi`, `sqlalchemy` specified without version pin in `requirements.txt` | Unpredictable deployments, breaking changes from minor/patch | Pin exact versions or use lock file |
| 2 | No lock file | medium | Using `requirements.txt` without hashes or lock file | Non-reproducible builds, supply chain risk | Use Poetry (`poetry.lock`) or pip-tools with hash pinning |
| 3 | GPL/AGPL dependency in proprietary project | high | Dependency with GPL license in proprietary/commercial project | License violation, legal risk | Replace with MIT/Apache-2.0 compatible alternative |
| 4 | Stale test tooling | low | `pytest` or `pytest-cov` more than 1 major version behind | Missing new features, may have unpatched CVEs | Upgrade to latest stable pytest |

### JavaScript/TypeScript Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Abandoned dependency | high | Package with no updates in >2 years, deprecated by author | Unpatched CVEs, no community support, compatibility issues | Replace with maintained alternative |
| 2 | Mixed lock files | medium | Both `package-lock.json` and `yarn.lock` present | Build system confusion, CI may use wrong lock file | Choose one package manager, delete the other lock file |
| 3 | `lodash` used for simple operations | low | Importing entire lodash library for `.map()` / `.filter()` helpers | Unnecessary bundle size, native methods available | Use native `Array.prototype.map()`, `Object.entries()`, etc. |
| 4 | Missing lock file integrity check | medium | Lock file updated without reviewing changes | Malicious package injection in dependency tree goes unnoticed | Review lock file diffs in PRs, use `npm ci` / `yarn --frozen-lockfile` in CI |

---

## Code Examples (Bad/Good Pairs)

### .NET/C# Example 1: Central Package Version Bump

```xml
<!-- ❌ BAD: Directory.Packages.props with stale packages, version drift -->
<Project>
  <PropertyGroup>
    <ManagePackageVersionsCentrally>true</ManagePackageVersionsCentrally>
  </PropertyGroup>
  <ItemGroup>
    <PackageVersion Include="WolverineFx" Version="5.30.0" />
    <!-- Latest is 6.3.0 — behind by major version, missing features -->
    <PackageVersion Include="FluentValidation" Version="11.9.0" />
    <!-- Latest is 12.0.0 — major version lag -->
    <PackageVersion Include="Refit" Version="8.0.0" />
    <!-- Latest is 10.0.0 — 2 major versions behind -->
    <PackageVersion Include="xunit" Version="2.9.3" />
    <!-- xUnit v3 available for .NET 8+ -->
  </ItemGroup>
</Project>

<!-- ✅ GOOD: Directory.Packages.props with current packages -->
<Project>
  <PropertyGroup>
    <ManagePackageVersionsCentrally>true</ManagePackageVersionsCentrally>
  </PropertyGroup>
  <ItemGroup>
    <!-- WolverineFx: target 6.x for new projects, or plan incremental upgrade -->
    <PackageVersion Include="WolverineFx" Version="6.3.0" />
    <!-- FluentValidation: target 12.x after testing breaking changes -->
    <PackageVersion Include="FluentValidation" Version="12.0.0" />
    <!-- Refit: migrate incrementally, v9→v10 documented migrations -->
    <PackageVersion Include="Refit" Version="10.0.0" />
    <!-- xUnit v3 for modern projects -->
    <PackageVersion Include="xunit.v3" Version="1.0.0" />
    <PackageVersion Include="xunit.runner.visualstudio" Version="3.0.0" />
  </ItemGroup>
</Project>
```

### .NET/C# Example 2: Proper IsTestProject Scoping

```xml
<!-- ❌ BAD: Source project marked as test project — pulls test dependencies into library -->
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
    <IsTestProject>true</IsTestProject>  <!-- WRONG: This is a source library! -->
  </PropertyGroup>
  <ItemGroup>
    <PackageReference Include="xunit" Version="2.9.3" />
    <PackageReference Include="Moq" Version="4.20.72" />
    <PackageReference Include="Microsoft.NET.Test.Sdk" Version="17.12.0" />
  </ItemGroup>
</Project>

<!-- ✅ GOOD: Source project clean, test dependencies isolated in proper test project -->
<!-- src/FamilyOffice.Modules.People/FamilyOffice.Modules.People.csproj -->
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
    <!-- No IsTestProject — this is a library, not a test project -->
  </PropertyGroup>
  <ItemGroup>
    <PackageReference Include="FluentValidation" />
  </ItemGroup>
</Project>

<!-- tests/FamilyOffice.Tests.Unit/FamilyOffice.Tests.Unit.csproj -->
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
    <IsTestProject>true</IsTestProject>  <!-- Correct: this IS a test project -->
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
// ❌ BAD: Legacy Polly.Extensions.Http — pulls old Polly and transitive deps
// In .csproj:
// <PackageReference Include="Polly.Extensions.Http" Version="3.0.0" />
// <PackageReference Include="Polly" Version="7.2.4" />

services.AddHttpClient("ExternalApi")
    .AddTransientHttpErrorPolicy(policy =>
        policy.WaitAndRetryAsync(3, retryAttempt => TimeSpan.FromSeconds(Math.Pow(2, retryAttempt))));

// ✅ GOOD: Microsoft.Extensions.Http.Resilience — modern, built-in, less transitive deps
// In .csproj:
// <PackageReference Include="Microsoft.Extensions.Http.Resilience" Version="9.0.0" />

services.AddHttpClient("ExternalApi")
    .AddStandardResilienceHandler();
```

### Python Example 1: Floating Dependencies

```python
# ❌ BAD: requirements.txt with floating versions — unpredictable deployments
fastapi>=0.100.0
sqlalchemy>=2.0.0
httpx>=0.25.0
# No upper bound — next major version may break

# ✅ GOOD: requirements.txt with pinned versions
fastapi==0.115.6
sqlalchemy==2.0.36
httpx==0.28.1
```

### JavaScript/TypeScript Example 1: npm Audit Findings

```bash
# ❌ BAD: npm audit with critical/high findings ignored
$ npm audit
# critical  Critical severity vulnerabilities found in lodash 4.17.20
# high     High severity vulnerabilities found in axios 0.21.1
# (no action taken, findings remain in production)

# ✅ GOOD: npm audit findings addressed
$ npm audit fix
# Or: manually update to patched versions
$ npm install lodash@4.17.21 axios@1.7.0
$ npm audit  # Now: found 0 vulnerabilities
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
  "description": "What dependency issue was found and why it matters. Include package name, current version, and known CVE if applicable.",
  "recommendation": "Specific fix: upgrade target version, migration path, or documented mitigation.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "security|maintainability|compliance|performance"
}
```

### Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Severity Guidelines below.
- **file**: Relative path from project root. Use forward slashes. Never absolute paths.
- **line**: 1-based line number where the issue starts in the manifest file.
- **endLine**: Optional. Line where the issue ends.
- **description**: Factual, specific. Include package name, current version, known CVEs, and license.
- **recommendation**: Actionable. Include the target version and any migration notes.
- **technology**: Primary technology of the finding.
- **category**: `security` for CVE findings, `maintainability` for version drift, `compliance` for license issues.

### Summary Report

After all findings, emit a summary:

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
| **critical** | Known CVE with CVSS >= 9.0, or GPL-licensed dependency in proprietary code | Package with remote code execution CVE, AGPL library in commercial product |
| **high** | Known CVE with CVSS 7-9, deprecated core package, or src project marked as test project | Active CVE with clear exploit, Polly.Extensions.Http in production, IsTestProject=true on library |
| **medium** | Package >1 major version behind, uncommitted lock file, minor license concerns | WolverineFx v5 vs v6, no lock file, vendored code without license header |
| **low** | Minor version behind latest, non-critical update available, test tooling staleness | Covelet or Playwright a minor version behind, deprecated dev-only package |

### Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding has known CVE with published exploit | +1 level | All technologies |
| Finding is in production/run-time dependency (not dev/test) | +1 level | All technologies |
| Finding affects authentication/authorization library | +1 level | All technologies |
| Finding is in encryption/cryptography package | +1 level | All technologies |
| Package is unmaintained for >2 years | +1 level | All technologies |

### Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding is in test-only dependency | -1 level (min low) | All technologies |
| Finding has documented compensating control | -1 level (min low) | All technologies |
| CVE is marked disputed or has no known exploit | -1 level (min low) | All technologies |
| Package is scheduled for removal in next sprint | -1 level (min low) | All technologies |

### Technology-Specific Severity Adjustments

#### .NET/C#
- Known CVE in production NuGet package → critical
- Central package version drift >1 major → medium
- src project with `IsTestProject=true` → high
- Deprecated `Polly.Extensions.Http` → high (legacy transitive deps)

#### Python
- Floating version without upper bound in `requirements.txt` → high
- GPL dependency in proprietary project → critical
- No lock file in production project → medium

#### JavaScript/TypeScript
- Abandoned package with known CVE → critical
- Mixed lock files → medium
- Deprecated package with active replacement (e.g., `moment.js`) → medium

---

## Integration Notes

### Squad Dependency Graph

```
audit-dependencies ◄─────────────────┐  (foundation for security and license audits)
audit-security ◄─────────────────────┤  (consumes CVE data from dependencies)
audit-license ◄──────────────────────┤  (consumes license data from dependencies)
audit-refactoring ◄──────────────────┤  (consumes deprecated package data)
audit-code-quality ──────────────────┤
audit-architecture ──────────────────┤
audit-test-coverage ─────────────────┤
audit-consolidated ◄─────────────────┘  (consumes all)
audit-consistency-coherence            (consumes all)
```

### Upstream Dependencies

- *(none)* — Dependencies squad is a foundation squad and does not consume other squad findings.

### Downstream Consumers

- **audit-security**: Consumes CVE findings from dependencies squad to correlate with code-level vulnerabilities.
- **audit-license**: Consumes license findings for license compliance report.
- **audit-refactoring**: Consumes deprecated package findings to recommend migration paths.
- **audit-consolidated**: Aggregates all squad findings into unified report.
- **audit-consistency-coherence**: Validates cross-domain alignment.

### Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-dependencies | audit-security | Dependencies focuses on CVE in third-party packages; security focuses on code-level vulnerabilities |
| audit-dependencies | audit-license | Dependencies checks license of packages; license checks license of the project's own code |
| audit-dependencies | audit-refactoring | Dependencies flags deprecated packages; refactoring suggests API-level migration patterns |

### Recommended Execution Order

For a full audit, this squad should run first:

1. **audit-dependencies** ← This squad (foundation — establish baseline of third-party risk)
2. **audit-security** — Code-level vulnerability scan (uses CVE data)
3. **audit-architecture** — Structural analysis
4. **audit-code-quality** — Implementation quality
5. **audit-refactoring** — Modernization (uses deprecated package data)
6. **audit-license** — License compliance (uses license data)
7. Then all remaining squads

### Output Integration

When squads run sequentially:
1. This squad runs first — no upstream findings to read.
2. Output early findings in the standardized Section 7 format.
3. Subsequent squads (audit-security, audit-license, audit-refactoring) will consume this squad's findings.
4. Ensure each finding is self-contained with enough context for downstream consumers.