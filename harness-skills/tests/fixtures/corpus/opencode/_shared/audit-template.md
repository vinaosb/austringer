# Audit Skill Template v3.2

> **Purpose**: Canonical structure for all audit skills. Follow exactly. Every section required. Replace placeholder with squad-specific. Remove this header from final file.
>
> **v3.0**: 10 new required finding fields (`root_cause`, `impact`, `references`, `effort_estimate`, `detection_method`, `verification_command`, `confidence`, `fix_examples`, `related_findings`, `fp_risk_self_assessment`) — §7. Findings fix-ready, no re-investigation. §11 FP/FN protocol: self-assess FP risk, enumerate ≥3 FNs. Missing required = REJECTED.
>
> **v3.1**: 7 OPTIONAL trending fields (`fingerprint`, `unblocks`, `blocked_by`, `first_seen`, `last_seen`, `status`, `previous_severity`) — orchestrator auto-computes. `files_examined` in summary — negative space. §12 Coverage Accounting + §13 Delta Tracking. Enables `audit diff`, fix DAG, coverage heat map, SARIF/markdown, baseline, `--fail-on`.
>
> **v3.2**: §14 Squad-Level File Persistence — every squad writes findings DIRECTLY to disk (`.audit/in-progress/{run-id}/squads/{name}.json`) before returning. + §15 Chunked Atomic Write Protocol. Combined with orchestrator checkpoint + `audit resume` (see `audit-consolidated` v3.2 §18/19 + `commands/audit.md` v3.2), failed orchestrator ≠ lost squad work. 45-min restart → 5-10 min resume.

---

## Section 1: YAML Frontmatter

```yaml
---
name: audit-{squad-name}
description: >-
  {Language-agnostic 2-3 sentence description of what this squad audits.
   Must NOT reference any specific repository, framework, or language.
   Describe the AUDIT DOMAIN, not the target.}
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java", "Rust"]
version: "3.0.0"
author: {optional-author}
---
```

### Frontmatter Rules
1. `name` must start with `audit-` and use kebab-case.
2. `description` must be usable without modification for ANY project in ANY language.
3. `languages` array lists all languages the squad CAN audit. Remove languages only if the
   squad is fundamentally impossible to apply (e.g., a Python-specific linter check).
4. `version` is always `"3.2.0"` for the rewritten skills (v3.2 = resilience: squad-level disk persistence + checkpoint + resume).

---

## Section 2: USE FOR / DO NOT USE FOR

### USE FOR
{List specific audit tasks this squad performs. Cover all applicable languages.}

- {General audit task applicable to all languages}
- {.NET-specific audit task, e.g., "Analyzer warning violations (CAxxxx) in C# projects"}
- {Python-specific audit task, e.g., "Type hint coverage and correctness in Python modules"}
- {JavaScript/TypeScript audit task, e.g., "Strict mode compliance and dependency audit in JS/TS"}
- {Go-specific audit task, e.g., "Error handling patterns and context propagation in Go services"}
- {Java-specific audit task, e.g., "Null safety and dependency management in Maven/Gradle projects"}
- {Rust-specific audit task, e.g., "Unsafe block usage and error handling in Rust crates"}

### DO NOT USE FOR
{Clear boundaries. Cross-reference other squads.}

- {Out-of-scope task} (use `audit-{other-squad}`)
- {Another out-of-scope task} (use `audit-{other-squad}`)
- {Yet another boundary} (use `{different-tool-or-squad}`)

---

## Section 3: Target Discovery

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

## Section 4: Technology-Specific Checklists

### 4.1 .NET/C# Checklist

- [ ] **CAxxxx analyzer warnings**: Check for suppressed analyzer warnings (`#pragma warning disable`),
  justified or not. `TreatWarningsAsErrors` should be enabled.
- [ ] **Nullable reference types**: `#nullable enable` in all files. No `null!` field initialization
  suppressions without justification.
- [ ] **Async patterns**: No `async void` except event handlers. No `.Result`, `.Wait()`, or
  `Task.Run` for CPU-bound work in ASP.NET Core. All async methods return `Task`/`Task<T>`/`ValueTask<T>`.
- [ ] **DI registration**: All services registered with correct lifetimes (Singleton/Scoped/Transient).
  No captive dependencies (Singleton depending on Scoped).
- [ ] **Middleware ordering**: Authentication before authorization. Exception handling early in pipeline.
  HSTS/HTTPS redirect in production.
- [ ] **EF Core query patterns**: No N+1 queries (missing `Include`/`ThenInclude`). No client-side
  evaluation. AsNoTracking for read-only queries. Compiled queries for hot paths.
- [ ] **Health checks**: All external dependencies have health check registrations. Readiness and
  liveness probes configured separately.
- [ ] **Authentication/Authorization**: `[Authorize]` on endpoints requiring auth. Claims-based access
  with minimal required claims. No hardcoded roles — use policy-based auth.
- [ ] **Encryption at rest**: Sensitive fields encrypted before storage. Key rotation mechanism exists.
  Keys never stored in source code.
- [ ] **Configuration**: Secrets from environment variables or secret manager, never in `appsettings.json`.
  `IOptions<T>` pattern for strongly-typed config.
- [ ] **File-scoped namespaces**: Project enforces `csharp_style_namespace_declarations = file_scoped`.
- [ ] **Central package management**: `Directory.Packages.props` exists and is used.
- [ ] **Global usings**: Check for `GlobalUsings.cs` or `ImplicitUsings` enabled.

### 4.2 Python Checklist

- [ ] **Type hints**: All public functions have type annotations. `mypy` or `pyright` passes with
  `strict` mode. No `Any` without justification.
- [ ] **Dependency management**: `pyproject.toml` with pinned versions (or `requirements.txt` with
  hashes). No floating versions in production (`>=` without upper bound).
- [ ] **Async patterns**: No blocking calls in async functions (`time.sleep`, `requests.get` in
  `async def`). Use `asyncio.sleep`, `httpx.AsyncClient` instead.
- [ ] **Security scanning**: `bandit` configured and passes. No `subprocess` with `shell=True`.
  No `eval()`/`exec()` on user input. No `pickle.loads` on untrusted data.
- [ ] **Testing**: `pytest` configured with coverage threshold. Tests in `tests/` directory.
  Fixtures in `conftest.py`. Parametrized tests for edge cases.
- [ ] **Error handling**: No bare `except:` clauses. Specific exception types caught. Custom
  exception hierarchy for domain errors.
- [ ] **Environment management**: `.env.example` exists. `.env` in `.gitignore`. No secrets in
  source code.
- [ ] **Logging**: Structured logging (`structlog` or `logging-json`). No `print()` statements in
  production code. Log levels used correctly.
- [ ] **Import organization**: Standard library → third-party → local. No wildcard imports
  (`from module import *`).
- [ ] **Docstrings**: Public functions/classes have docstrings following Google or NumPy style.

### 4.3 JavaScript/TypeScript Checklist

- [ ] **Strict mode**: `"strict": true` in `tsconfig.json`. `"use strict"` in JS files.
- [ ] **Type checking**: No `any` type without `eslint-disable` justification. `strictNullChecks`
  enabled. No non-null assertions (`!`) without justification.
- [ ] **Dependency audit**: `npm audit` / `yarn audit` passes with no high/critical vulnerabilities.
  `package-lock.json` / `yarn.lock` committed.
- [ ] **Testing**: Test framework configured (Jest/Vitest). Coverage threshold set. Tests co-located
  or in `__tests__/` / `tests/`.
- [ ] **Security**: `eslint-plugin-security` configured. No `eval()`. No `innerHTML` assignment with
  user input. HTTP-only cookies for session tokens.
- [ ] **Error handling**: Async errors caught with try/catch or `.catch()`. No unhandled promise
  rejections. Error boundaries in React.
- [ ] **Environment variables**: `.env.example` exists. Secrets via environment variables, not
  hardcoded. `process.env` validated at startup.
- [ ] **Module system**: ESM preferred (`"type": "module"` in `package.json` or `.mjs` extension).
  Named exports over default exports.
- [ ] **Build configuration**: Bundler configured (esbuild, webpack, vite). Source maps in
  development. Tree-shaking enabled for production.
- [ ] **Linting**: ESLint configured with TypeScript parser. Prettier for formatting.
  Pre-commit hooks (lint-staged + husky).

### 4.4 Go Checklist

- [ ] **Error handling**: No ignored errors (`_ = someFunc()`). Errors wrapped with context
  (`fmt.Errorf("doing X: %w", err)`). Custom error types for domain errors.
- [ ] **Context propagation**: All functions that do I/O accept `context.Context` as first parameter.
  Context passed through call chain, not stored in structs.
- [ ] **Module versioning**: `go.mod` has explicit Go version. Dependencies pinned. `go.sum` committed.
  No `replace` directives pointing to local paths in production.
- [ ] **Testing**: Table-driven tests for functions with multiple cases. `t.Parallel()` where safe.
  Test files named `*_test.go`. Integration tests tagged with build tags.
- [ ] **Concurrency**: No shared mutable state without synchronization. Channels preferred over
  shared memory. `sync.Mutex` / `sync.RWMutex` used correctly. No goroutine leaks.
- [ ] **Resource management**: `defer` for cleanup (Close, Body.Close). HTTP response bodies always
  closed. File handles closed.
- [ ] **Struct initialization**: No zero-value initialization where zero values are invalid.
  Constructor functions (`NewXxx`) for complex types.
- [ ] **API design**: Interfaces defined by consumers, not producers. Small interfaces (1-3 methods).
  Accept interfaces, return structs.

### 4.5 Java Checklist

- [ ] **Null safety**: No `null` returns from public methods. `Optional<T>` for nullable returns.
  `@NonNull` / `@Nullable` annotations. Null checks before method calls on external input.
- [ ] **Dependency management**: Maven `pom.xml` or Gradle `build.gradle` with pinned versions.
  Dependency vulnerability scanning configured (OWASP Dependency-Check / Dependabot).
- [ ] **Testing**: JUnit 5 preferred. Tests in `src/test/java`. Integration tests separated from unit
  tests. Test coverage threshold enforced.
- [ ] **Security scanning**: SpotBugs with FindSecBugs plugin. No SQL string concatenation. Prepared
  statements used. Input validation on all public API endpoints.
- [ ] **Exception handling**: Checked exceptions for recoverable errors. Unchecked for programming
  errors. No catching `Exception` and ignoring. Custom exception hierarchy.
- [ ] **Resource management**: Try-with-resources for all `AutoCloseable` types. No unclosed streams
  or connections.
- [ ] **Concurrency**: `java.util.concurrent` preferred over `synchronized`. Thread-safe collections.
  No shared mutable static state.
- [ ] **Logging**: SLF4J with Logback or Log4j2. No `System.out.println` in production. Structured
  logging for machine-readable output.

### 4.6 Rust Checklist

- [ ] **Unsafe usage**: `unsafe` blocks documented with safety invariants. Minimized unsafe surface
  area. No unnecessary unsafe.
- [ ] **Error handling**: `Result<T, E>` for fallible operations. No `unwrap()` or `expect()` in
  library code. `thiserror` / `anyhow` for error types. `?` operator used consistently.
- [ ] **Dependency audit**: `cargo audit` passes. `Cargo.lock` committed for binaries. Dependencies
  from trusted registries only.
- [ ] **Testing**: Unit tests in same file (`#[cfg(test)]`). Integration tests in `tests/` directory.
  Property-based testing with `proptest` for complex logic.
- [ ] **Clippy**: `cargo clippy` passes with no warnings. Pedantic lints enabled where practical.
- [ ] **Memory safety**: No memory leaks (verify Drop implementations). No use-after-free patterns.
  Lifetimes annotated correctly.
- [ ] **Concurrency**: `Send` and `Sync` bounds understood. No data races. `Arc<Mutex<T>>` / channels
  for shared state. `rayon` for parallel iteration.
- [ ] **Panic safety**: No panics in library code. `unwrap` only in tests. `expect` with descriptive
  messages only in application code where crash is acceptable.

---

## Section 5: Anti-Patterns

Each anti-pattern has: severity, technology tag, description, why harmful, correct pattern.

### 5.1 Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Hardcoded secrets | critical | API keys, passwords, tokens in source code | Secrets leak via version control, logs, error reports | Use environment variables or secret manager (Vault, OpenBao, AWS Secrets Manager) |
| 2 | Catching and swallowing exceptions | high | Empty catch blocks or logging without rethrow | Errors silently fail, making debugging impossible | Catch specific exceptions, log with context, rethrow or handle explicitly |
| 3 | God class/module | medium | Single class/module with >10 responsibilities | Violates SRP, hard to test, hard to maintain, merge conflicts | Split into focused classes/modules with single responsibility |
| 4 | Copy-paste code | medium | Duplicated logic across files | Changes require multiple edits, inconsistencies emerge | Extract shared functions, use composition or inheritance |
| 5 | Magic numbers/strings | low | Unexplained literal values | Unclear intent, fragile to changes | Named constants or enums |

### 5.2 .NET/C# Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | `async void` methods | high | Async methods returning void instead of Task | Exceptions crash the process, cannot be awaited | Return `Task` or `Task<T>`; `async void` only for event handlers |
| 2 | `.Result` / `.Wait()` on async | high | Synchronously blocking on async code | Causes deadlocks in ASP.NET / UI contexts | Use `await` throughout the call chain |
| 3 | Captive DI dependencies | high | Singleton service depends on Scoped service | Scoped service becomes singleton, causing concurrency bugs | Match dependency lifetimes; Singleton→Singleton, Scoped→Scoped |
| 4 | TenantId on entities | high | Declaring tenant ID as explicit entity property | Conflicts with Finbuckle multi-tenancy shadow property | Let Finbuckle manage tenant ID as shadow property |
| 5 | `IsMultiTenant()` in EF configs | high | Calling `builder.IsMultiTenant()` per entity | Conflicts with centralized multi-tenant configuration | Only call `IsMultiTenant()` in shared base configuration |
| 6 | Missing `AsNoTracking()` | medium | Read-only queries with change tracking enabled | Unnecessary memory allocation, slower performance | Use `AsNoTracking()` for read-only queries |
| 7 | Caching DEKs externally | critical | Storing data encryption keys in Redis/Dragonfly | Keys accessible to other tenants, network exposure | In-process memory only for DEK caching |

### 5.3 Python Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | `shell=True` in subprocess | critical | Passing user input to shell commands | Command injection vulnerability | Use `shell=False`, pass arguments as list |
| 2 | `eval()`/`exec()` on user input | critical | Dynamically executing user-provided strings | Remote code execution | Use `ast.literal_eval()` for literals, or validate+parse explicitly |
| 3 | `pickle.loads` on untrusted data | critical | Deserializing untrusted pickle data | Arbitrary code execution | Use `json` or `msgpack` for untrusted data |
| 4 | Bare `except:` clauses | high | Catching all exceptions without type | Hides bugs, makes debugging impossible | Catch specific exception types |
| 5 | Mutable default arguments | medium | `def foo(items=[])` | List persists across calls, causing shared state | Use `None` default, create new list inside function |
| 6 | Global mutable state | high | Module-level dicts/lists modified at runtime | Thread-unsafe, hard to test, unpredictable behavior | Pass state explicitly or use class instances |

### 5.4 JavaScript/TypeScript Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | `eval()` with user input | critical | Executing user-provided strings | Remote code execution | Use JSON.parse for data, template literals for strings |
| 2 | `innerHTML` with user input | high | Setting innerHTML from untrusted source | XSS vulnerability | Use `textContent`, `createElement`, or sanitization library |
| 3 | `any` type in TypeScript | medium | Using `any` instead of specific types | Defeats type checking, runtime errors | Use specific types, `unknown` for truly unknown, or generics |
| 4 | Unhandled promise rejections | high | Missing `.catch()` or try/catch around async | Crashes, unhandled rejection warnings | Always handle errors in async code |
| 5 | Callback hell | medium | Deeply nested callbacks | Unreadable, unmaintainable code | Use async/await or Promise chains |
| 6 | Synchronous require in hot paths | low | `require()` inside request handlers | Blocks event loop | Pre-load modules at startup |

---

## Section 6: Code Examples (Bad/Good Pairs)

### 6.1 .NET/C# Examples

#### Example 1: Async/Await — Blocking on Async Code

```csharp
// ❌ BAD: Synchronously blocking — causes deadlocks in ASP.NET
public async Task<User> GetUserAsync(int id)
{
    var result = _dbContext.Users.FindAsync(id).Result;
    return result;
}

// ✅ GOOD: Properly async throughout
public async Task<User?> GetUserAsync(int id)
{
    var result = await _dbContext.Users.FindAsync(id);
    return result;
}
```

#### Example 2: Dependency Injection — Captive Dependency

```csharp
// ❌ BAD: Singleton depends on Scoped service
services.AddSingleton<IReportGenerator, ReportGenerator>();
// ReportGenerator constructor: public ReportGenerator(IDbContext dbContext)

// ✅ GOOD: Match lifetimes — both scoped
services.AddScoped<IReportGenerator, ReportGenerator>();
// Or restructure to use IServiceProvider/IScopeFactory if Singleton is required
```

#### Example 3: EF Core — Missing AsNoTracking

```csharp
// ❌ BAD: Change tracking enabled for read-only query
public async Task<List<Order>> GetRecentOrdersAsync()
{
    return await _dbContext.Orders
        .Where(o => o.CreatedAt > DateTime.UtcNow.AddDays(-30))
        .ToListAsync();
}

// ✅ GOOD: AsNoTracking for read-only queries
public async Task<List<Order>> GetRecentOrdersAsync()
{
    return await _dbContext.Orders
        .AsNoTracking()
        .Where(o => o.CreatedAt > DateTime.UtcNow.AddDays(-30))
        .ToListAsync();
}
```

### 6.2 Python Examples

#### Example 1: Subprocess — Command Injection

```python
# ❌ BAD: shell=True with user input — command injection
import subprocess

def delete_file(filename):
    subprocess.run(f"rm {filename}", shell=True)

# ✅ GOOD: shell=False, arguments as list
import subprocess

def delete_file(filename):
    # Also validate filename against path traversal
    if ".." in filename or "/" in filename:
        raise ValueError("Invalid filename")
    subprocess.run(["rm", filename], shell=False, check=True)
```

#### Example 2: Error Handling — Swallowing Exceptions

```python
# ❌ BAD: Bare except with pass
def process_data(data):
    try:
        result = transform(data)
    except:
        pass  # Error silently swallowed

# ✅ GOOD: Specific exception, logged with context
import logging

logger = logging.getLogger(__name__)

def process_data(data):
    try:
        result = transform(data)
    except ValueError as e:
        logger.error("Transform failed for data=%s: %s", data[:100], e)
        raise
```

#### Example 3: Type Safety — Missing Type Hints

```python
# ❌ BAD: No type hints, unclear contract
def calculate_total(items, tax_rate):
    total = sum(i["price"] * i["quantity"] for i in items)
    return total * (1 + tax_rate)

# ✅ GOOD: Type hints with proper models
from dataclasses import dataclass

@dataclass(frozen=True)
class LineItem:
    price: float
    quantity: int

def calculate_total(items: list[LineItem], tax_rate: float) -> float:
    subtotal = sum(item.price * item.quantity for item in items)
    return subtotal * (1 + tax_rate)
```

### 6.3 JavaScript/TypeScript Examples

#### Example 1: XSS — innerHTML with User Input

```javascript
// ❌ BAD: XSS vulnerability
function displayUserName(name) {
    document.getElementById("user-name").innerHTML = name;
}

// ✅ GOOD: textContent escapes HTML automatically
function displayUserName(name) {
    document.getElementById("user-name").textContent = name;
}
```

#### Example 2: Error Handling — Unhandled Promise

```javascript
// ❌ BAD: Unhandled promise rejection
async function fetchUserData(userId) {
    const response = await fetch(`/api/users/${userId}`);
    const data = await response.json();
    return data;
}

// ✅ GOOD: Error handling with meaningful messages
async function fetchUserData(userId) {
    try {
        const response = await fetch(`/api/users/${userId}`);
        if (!response.ok) {
            throw new Error(`Failed to fetch user ${userId}: ${response.status}`);
        }
        return await response.json();
    } catch (error) {
        logger.error("fetchUserData failed", { userId, error });
        throw new UserFetchError(userId, { cause: error });
    }
}
```

#### Example 3: TypeScript — Type Safety

```typescript
// ❌ BAD: any type defeats type checking
function processConfig(config: any) {
    return config.database.url;
}

// ✅ GOOD: Specific types with runtime validation
interface DatabaseConfig {
    url: string;
    poolSize: number;
}

interface AppConfig {
    database: DatabaseConfig;
}

function processConfig(config: AppConfig): string {
    return config.database.url;
}
```

---

## Section 7: Output Format

All audit findings MUST be emitted as structured JSON objects. One finding per object.

**v3.0 cardinal rule**: A finding is only complete when a reviewer can implement the fix
**without re-investigating the codebase**. Every required field below MUST be populated.
Findings missing required fields are REJECTED by `audit-consolidated`.

### 7.1 Finding Schema (v3.0 — fix-ready, no re-investigation needed)

```json
{
  "id": "{squad-short}-001",
  "severity": "critical|high|medium|low",
  "file": "relative/path/to/file",
  "line": 42,
  "endLine": 45,
  "title": "Short headline (max 90 chars) — what + where",
  "description": "Full factual explanation: what was found, why it matters, the actual problematic code pattern. Include metric values, exact counts, vulnerable data flow. NOT a hedge. Be specific enough that a reviewer who has never seen this code understands the issue.",
  "root_cause": "WHY this issue exists, not WHAT. Deeper than description. Examples: 'T105a submodule split was started but abandoned mid-refactor (see commit abc123)'; 'Package added in T22 spike without audit; cargo-deny not in CI to catch it'; 'API contract drifted from spec during v1.4 — see decisions/api-compat-v20.md'. Quote the historical/convention source when possible.",
  "impact": "Concrete consequences if NOT fixed. NOT 'potential issue' — be specific. Examples: 'RCE on any host running buffup install (attacker controls GitHub release assets in MITM)'; '5-15min CI time waste per PR (100+ tests skipped)'; 'Maintainer onboarding friction — 10.5k LOC file is unreviewable'. Quantify when possible.",
  "recommendation": "Specific fix strategy — the architectural approach, not just 'fix it'. Reference a pattern, existing code to mirror, or doc link.",
  "fix_examples": {
    "before": "Exact problematic code snippet (max 30 lines). Use real code from the file at the cited line range.",
    "after": "Concrete corrected code (max 30 lines). If multiple valid fixes, show the recommended one and mention alternatives in `recommendation`."
  },
  "fix_guide": "Numbered actionable steps to implement the fix. Each step verifiable. Example: '1) Extract CalculateBaseRate() lines 45-55  2) Add unit test for extracted fn  3) Verify cargo test passes  4) Run clippy --all-targets'",
  "effort_estimate": {
    "minutes": 240,
    "human_readable": "4 hours",
    "confidence": "high|medium|low",
    "breakdown": ["30min: read existing pattern", "2h: implement", "30min: tests", "1h: review"]
  },
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|Rust|CrossLanguage",
  "category": "security|architecture|performance|compliance|maintainability|testing|observability",
  "detection_method": {
    "tool": "manual-review|grep|rg|ast-grep|cargo-audit|cargo-deny|clippy|npm-audit|pip-audit|go-vulncheck|scanner-X",
    "query": "Exact search/grep pattern or command that reproduces this finding. Example: `rg --type rust 'Mutex::lock\\(\\)\\.unwrap'`, or `cargo audit -i ethers`, or 'manual review of CI workflow file'",
    "notes": "Optional — any caveat about why this method found it (or what would miss it)"
  },
  "verification_command": "A copy-pasteable shell command that reproduces/confirms the finding. Examples: `rg -n 'unwrap\\(\\)' crates/buff-lang-cli/src/commands/doc.rs`, `cargo tree -i ethers --target all`, `grep -c 'continue-on-error' .github/workflows/ci.yml`. MUST be runnable from project root.",
  "references": [
    "URLs to CVE entries, OWASP chapters, RFC sections, official docs, style guides, ADRs in the repo, related AGENTS.md sections, related plan files. Use full URLs or repo-relative paths."
  ],
  "related_findings": ["{squad-short}-002", "{other-squad}-007"],
  "confidence": {
    "score": 0.92,
    "rationale": "Why this confidence level. Examples: 'Verified by reading lines 6241-9791 directly; cross-confirmed by audit-refactoring'; 'Medium — claim is plausible but unable to fully verify dependency tree without Cargo.lock analysis'; 'High — cargo tree -i output attached'"
  },
  "fp_risk_self_assessment": {
    "risk_level": "low|medium|high",
    "reasoning": "Self-critical analysis: under what conditions would this be a FALSE POSITIVE? Examples: 'Low — verified directly in code, no mitigating circumstances'; 'Medium — would be FP if test code is exempted from the no-unwrap rule (verify in AGENTS.md)'; 'High — claim assumes default-features=true; if a Cargo.toml overrides this, finding is void'",
    "verified_against_conventions": true,
    "convention_files_checked": ["AGENTS.md", "CONTRIBUTING.md", ".sisyphus/plans/buff-conventions.md"]
  }
}
```

### 7.2 Field Rules (v3.1 — required fields + optional trending/triage fields)

**Required fields** (REJECTED if missing — same as v3.0):

| Field | Required | Purpose | Format |
|-------|----------|---------|--------|
| `id` | ✅ | Stable cross-reference ID | `{squad-short}-NNN` (zero-padded 3-digit) |
| `severity` | ✅ | Impact level | `critical` / `high` / `medium` / `low` |
| `file` | ✅ | File path rel to project root | Forward slashes, never absolute |
| `line` | ✅ | Start line (1-based) | int |
| `endLine` | ⚪ optional | End line for multi-line issues | int or omit |
| `title` | ✅ | Headline for reports/changelogs | ≤90 chars, "what + where" |
| `description` | ✅ | WHAT + WHY-IT-MATTERS | Full paragraph, includes code pattern |
| `root_cause` | ✅ | WHY it exists (historical/process) | Quote source if possible |
| `impact` | ✅ | Concrete consequences if unfixed | Quantified when possible |
| `recommendation` | ✅ | Fix strategy (architectural) | Pattern + reference |
| `fix_examples.before` | ✅ | Actual problematic code | Real snippet, ≤30 lines |
| `fix_examples.after` | ✅ | Actual corrected code | Real snippet, ≤30 lines |
| `fix_guide` | ✅ | Numbered verifiable steps | `1) ... 2) ...` |
| `effort_estimate` | ✅ | Time to fix | `{minutes, human_readable, confidence, breakdown}` |
| `technology` | ✅ | Tech stack | e.g. `Rust`, `.NET`, `CrossLanguage` |
| `category` | ✅ | Primary category | `security`/`architecture`/etc. |
| `detection_method.tool` | ✅ | How it was found | One of the enumerated values or new tool name |
| `detection_method.query` | ✅ | Reproducible search | Exact grep/rg pattern |
| `verification_command` | ✅ | Copy-pasteable confirm cmd | Runnable from project root |
| `references` | ✅ | External/internal links | URLs or repo-relative paths (≥1 entry) |
| `related_findings` | ⚪ optional | Cross-reference IDs | List of `{squad}-NNN` |
| `confidence.score` | ✅ | Self-confidence 0.0-1.0 | float |
| `confidence.rationale` | ✅ | Why this score | Reasoning |
| `fp_risk_self_assessment.risk_level` | ✅ | Self-FP-risk rating | `low`/`medium`/`high` |
| `fp_risk_self_assessment.reasoning` | ✅ | Conditions for FP | Self-critical |
| `fp_risk_self_assessment.verified_against_conventions` | ✅ | Did you check repo rules? | bool |
| `fp_risk_self_assessment.convention_files_checked` | ✅ | Which convention sources | List of paths |

**Optional v3.1 fields** (orchestrator auto-computes if absent — squad MAY populate to override):

| Field | Required | Purpose | Format | Auto-compute rule |
|-------|----------|---------|--------|-------------------|
| `fingerprint` | ⚪ opt | Stable ID across runs (survives line-number drift) | hex string (SHA-256, 16 chars truncated) | `sha256(file + "::" + category + "::" + normalize(root_cause))[:16]` — see Section 13.1 |
| `unblocks` | ⚪ opt | Finding IDs this fix would resolve | `["{squad}-NNN", ...]` | Inferred by orchestrator via root-cause clustering (see audit-consolidated Section 14) |
| `blocked_by` | ⚪ opt | Finding IDs that must be fixed first | `["{squad}-NNN", ...]` | Inferred by orchestrator (inverse of `unblocks`) |
| `first_seen` | ⚪ opt | ISO timestamp when fingerprint first appeared | ISO 8601 UTC | Looked up from `--previous` report; defaults to current audit timestamp |
| `last_seen` | ⚪ opt | ISO timestamp of most recent audit containing this fingerprint | ISO 8601 UTC | Always = current audit timestamp |
| `status` | ⚪ opt | Delta vs previous audit | `new` / `recurring` / `regressed` / `fixed` / `unchanged` | Computed by orchestrator (see Section 13.2) |
| `previous_severity` | ⚪ opt | Severity in previous audit (if recurring/regressed/unchanged) | `critical`/`high`/`medium`/`low`/`null` | Looked up from `--previous` report by fingerprint |
| `fix_difficulty` | ⚪ opt | Cognitive load (separate from time) | `mechanical`/`moderate`/`deep`/`expert` | Defaults to `moderate`; squad overrides for expert-system patterns |
| `test_requirements` | ⚪ opt | Tests that must accompany the fix | `[{type, description}]` | Inferred from category if absent (security→regression+edge-case, etc.) |
| `rollback_plan` | ⚪ opt | Revert strategy if fix goes wrong | string | Required only for `severity=critical` findings (orchestrator flags if missing) |
| `regulatory_clause` | ⚪ opt | Specific clause violated (if any) | `{standard, clause, url}` | Empty if not regulatory-relevant |
| `git_blame_owner` | ⚪ opt | Person who last touched the cited line | `{name, email, commit, committed_at}` | Looked up via `git blame -L line,endLine -- file` |
| `customer_impact` | ⚪ opt | Blast radius | `internal`/`external`/`internal-external` | Defaults to `internal-external` for `severity=critical` |
| `fix_batch_id` | ⚪ opt | Group findings fixable in one PR/commit | string | Computed by orchestrator via clustering on `file + category + effort` |

### 7.3 Validation Rules (enforced by audit-consolidated)

- Missing ANY required field → finding REJECTED with `validation_error` naming the missing field.
- `severity` not in enum → REJECTED.
- `line` is 0 or negative → REJECTED.
- `confidence.score` outside [0.0, 1.0] → REJECTED.
- `effort_estimate.minutes` ≤ 0 → REJECTED.
- `references` empty array → REJECTED (must cite at least one source).
- `fp_risk_self_assessment.verified_against_conventions == false` → flag with warning (not rejected, but tracked).
- Findings with `confidence.score < 0.5` → routed to FP-suspect queue in consolidated report.

### 7.4 Summary Report (v3.1 — includes coverage negative space)

After all findings, emit a summary:

```json
{
  "summary": {
    "squad": "audit-{squad-name}",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#", "TypeScript"],
    "totalFindings": 15,
    "bySeverity": { "critical": 1, "high": 3, "medium": 7, "low": 4 },
    "byCategory": { "security": 5, "performance": 4, "maintainability": 3, "architecture": 2, "testing": 1 },
    "byConfidence": { "high (>0.8)": 10, "medium (0.5-0.8)": 4, "low (<0.5)": 1 },
    "avgConfidence": 0.84,
    "fpRiskDistribution": { "low": 12, "medium": 2, "high": 1 },
    "filesScanned": 42,
    "filesWithFindings": 12,
    "files_examined": [
      {
        "path": "src/Modules/Billing/Calculator.cs",
        "lines_read": 78,
        "bytes_read": 2340,
        "examined_at": "2026-01-15T10:31:42Z",
        "findings_emitted": ["cq-001", "cq-002"],
        "verdict": "findings"
      },
      {
        "path": "src/Modules/Billing/Repository.cs",
        "lines_read": 145,
        "bytes_read": 4280,
        "examined_at": "2026-01-15T10:31:55Z",
        "findings_emitted": [],
        "verdict": "clean"
      }
    ],
    "files_skipped": [
      {
        "path": "vendor/third-party-lib/BigFile.cs",
        "reason": "vendored code — out of scope",
        "lines_not_examined": 5234
      }
    ],
    "coverage_stats": {
      "total_source_files_in_repo": 187,
      "files_examined_count": 42,
      "coverage_percent": 22.5,
      "lines_examined": 4280,
      "total_source_lines": 23410,
      "line_coverage_percent": 18.3
    },
    "effortEstimate": {
      "totalMinutes": 1440,
      "humanReadable": "~24 dev-hours",
      "bySeverity": { "critical": "8h", "high": "10h", "medium": "5h", "low": "1h" }
    },
    "falseNegativesIdentified": [
      {
        "area": "WebSocket handlers in crates/buff-lang-cli/src/ui_dev/",
        "reason": "No security squad examined WebSocket-specific attack surface (CSWSH, origin validation)",
        "suggestedSquad": "audit-security",
        "severity_if_found": "unknown"
      }
    ]
  }
}
```

---

## Section 8: Severity Guidelines

### 8.1 Base Severity Definitions

| Level | Definition | Example |
|-------|-----------|---------|
| **critical** | Immediate exploitation risk, data breach, or total system compromise | Remote code execution, SQL injection, exposed production credentials |
| **high** | Significant security, reliability, or correctness risk with clear attack vector | XSS, path traversal, authentication bypass, captive DI dependencies |
| **medium** | Degraded security posture, performance impact, or maintainability risk | Missing input validation, N+1 queries, missing type hints, verbose errors |
| **low** | Best practice deviation, code style, defense-in-depth improvement | Naming conventions, missing docstrings, minor refactoring opportunities |

### 8.2 Severity Escalation Triggers

Apply these modifiers to the base severity. Multiple triggers stack (up to critical max).

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding is in authentication/authorization code | +1 level | All technologies |
| Finding is in data encryption/key management | +1 level | All technologies |
| Finding affects multiple tenants or users | +1 level | Multi-tenant systems |
| Finding is in a public-facing endpoint | +1 level | Web applications |
| Finding involves financial/billing data | +1 level | Financial systems |
| Finding has known CVE associated | +1 level | Dependency vulnerabilities |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding is in non-production/test code only | -1 level (min low) | All technologies |
| Finding has existing mitigating control documented | -1 level (min low) | All technologies |
| Finding is in deprecated code scheduled for removal | -1 level (min low) | All technologies |
| Finding is in documentation/comments | -1 level (min low) | All technologies |

### 8.4 Technology-Specific Severity Adjustments

#### .NET/C#
- `async void` outside event handlers → high (crash risk)
- Missing `AsNoTracking` on read queries → medium (performance)
- Hardcoded connection strings → critical (credential leak)
- Missing `[Authorize]` on admin endpoints → high (auth bypass)

#### Python
- `shell=True` with any external input → critical (command injection)
- `pickle.loads` on network data → critical (RCE)
- Missing type hints on public API → medium (maintainability)
- Global mutable state → high (thread safety)

#### JavaScript/TypeScript
- `eval()` with any external input → critical (RCE)
- `innerHTML` with user input → high (XSS)
- `any` type in library code → medium (type safety)
- Missing error boundary in React → medium (UX crash)

#### Go
- Ignored errors (`_ =`) → high (silent failures)
- Missing context propagation → medium (timeout/cancellation broken)
- Unbounded goroutine spawn → high (resource exhaustion)

#### Java
- SQL string concatenation → critical (SQL injection)
- Missing try-with-resources → medium (resource leak)
- Shared mutable static state → high (thread safety)

#### Rust
- Unnecessary `unsafe` → high (memory safety bypass)
- `unwrap()` in library code → medium (panic risk)
- Missing `Cargo.lock` for binary crate → medium (reproducibility)

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
audit-tax-fiscal ───────────────────┤
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

### 9.2 Upstream Dependencies (squads whose output THIS squad consumes)

{List squads this squad reads. Common ones:}

- **audit-security**: May reference findings from `audit-dependencies` (CVE data) and `audit-cicd` (pipeline security)
- **audit-architecture**: May reference findings from `audit-code-quality` (coupling metrics)
- **audit-consolidated**: Consumes ALL other squad outputs
- **audit-consistency-coherence**: Consumes ALL other squad outputs for cross-domain validation

### 9.3 Downstream Consumers (squads that consume THIS squad's output)

- **audit-consolidated**: Aggregates all squad findings into unified report
- **audit-consistency-coherence**: Validates cross-domain alignment between squad findings

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-security | audit-dependencies | Security focuses on code vulnerabilities; dependencies focuses on CVE/license in third-party packages |
| audit-security | audit-cicd | Security focuses on code; cicd focuses on pipeline configuration |
| audit-architecture | audit-code-quality | Architecture focuses on layer/module structure; code-quality focuses on implementation smells |
| audit-code-quality | audit-refactoring | Code-quality identifies problems; refactoring recommends modernization approaches |
| audit-privacy-data | audit-regulatory-dataprotection | Privacy-data focuses on code-level PII handling; regulatory-dataprotection focuses on legal compliance |
| audit-financial-model | audit-tax-fiscal | Financial-model focuses on pricing/logic; tax-fiscal focuses on tax compliance |

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
12. **audit-consolidated** — Aggregate all findings
13. **audit-consistency-coherence** — Cross-domain validation

### 9.6 Output Integration

When squads run sequentially, each squad SHOULD:
1. Read the previous squad's findings from the shared output location.
2. Deduplicate: skip findings already reported by a previous squad.
3. Cross-reference: escalate findings confirmed by multiple squads.
4. Output in the standardized Section 7 format for aggregation.

---

## Section 10: Effort Estimation Reference

Standard effort buckets (use as defaults, override with project-specific data when available):

| Severity | Default minutes | Range | Notes |
|----------|-----------------|-------|-------|
| critical | 240 | 60-1440 (1h-3d) | Often architectural; if >1d, break into sub-tasks |
| high | 120 | 30-480 (0.5h-2d) | Typically localized fix + tests |
| medium | 60 | 15-240 | Style/perf/local refactor |
| low | 20 | 5-120 | Trivial cleanup |

### Effort Estimation Rules

1. **Confidence `low`** when: no similar pattern in repo to reference, or scope unclear.
2. **Confidence `medium`** when: similar fix exists in repo, but cross-cutting concerns.
3. **Confidence `high`** when: mechanical change, existing tests, clear pattern to mirror.
4. **Breakdown** should list each phase (read, implement, test, review) separately.
5. If total > 1 day, **MANDATE** that the fix_guide splits the work into shippable chunks.

### Cross-Finding Effort Aggregation

When multiple findings share a root cause (e.g., "no cargo-deny gate" causes C-001 + C-002), the consolidated report SHOULD aggregate them as a single effort unit with `shared_root_cause: true` to avoid double-counting.

---

## Section 11: False Positive / False Negative Detection Protocol (v3.0 MANDATORY)

**Goal**: Every finding must self-assess the risk of being a false positive, AND every squad must enumerate likely false negatives in its domain. This is mandatory cognitive hygiene — without it, audits accumulate noise and erode reviewer trust.

### 11.1 False Positive Categories (Self-Assess Each Finding Against These)

A finding is a **False Positive (FP)** if it does NOT represent a real issue. Common causes:

| FP Category | Description | Example | Mitigation |
|-------------|-------------|---------|------------|
| **FP-1: Convention Override** | Repo explicitly allows the pattern (in AGENTS.md, CONTRIBUTING, conventions doc, or ADR) | "Squad flags `unwrap()` in `tests/` as critical" — but AGENTS.md allows unwrap in test code | Read all convention files before flagging |
| **FP-2: Version Mismatch** | Claim depends on a version that doesn't match the repo | "ethers pulls cc-rs" — but Cargo.toml has `default-features = false` that disables the offending path | Run `cargo tree -i <pkg>` / `npm ls <pkg>` to verify |
| **FP-3: Scope Misunderstanding** | Pattern is intentional in this context | "Mutex::lock().unwrap()" in a context where panic-on-poison is desired (rare but valid) | Read surrounding code/comments to confirm intent |
| **FP-4: Stale Claim** | Code has changed since the pattern was added | "Missing tests" — but tests were added last week | Re-grep the file at the cited line |
| **FP-5: Heuristic Over-Fire** | Pattern matched but doesn't apply | "God class >300 LOC" — but the file is a generated/vendored file we don't own | Check if file is generated, vendored, or gitignored |
| **FP-6: Tool False Positive** | Scanner/CLI returned a hit that's noise | npm audit returned a dev-only CVE rated critical | Cross-check CVE scope (dev vs prod, transitive vs direct) |
| **FP-7: Already Mitigated** | Pattern exists but a compensating control covers it | "No checksum verification" — but a downstream tool does it | Search for compensating controls in adjacent files |
| **FP-8: Out of Scope** | Pattern is in code the squad shouldn't audit | code-quality flagging CI workflow syntax — that's cicd's job | Stay in lane; cross-reference instead of duplicating |
| **FP-9: Severity Mis-Calibration** | Real issue but wrong severity bucket | Flagged critical, actually low (or vice versa) | Apply severity escalation triggers in Section 8 |
| **FP-10: Phantom Pattern** | Squad hallucinated code that doesn't exist | "Function foo() at line 42" — but no such function exists | Always include verification_command output |

### 11.2 Self-Assessment Procedure (MANDATORY per finding)

For every finding, the squad MUST:

1. **Verify the code exists** — read the file at the cited line, confirm the pattern is real.
2. **Check conventions** — search AGENTS.md, CONTRIBUTING, `.sisyphus/decisions/`, `.sisyphus/plans/buff-conventions.md` (or project equivalents) for explicit overrides.
3. **Run verification_command** — capture the actual output, attach as part of `detection_method.notes` when possible.
4. **Cross-check version claims** — if finding depends on a dependency version or feature flag, verify via `cargo tree` / `npm ls` / `pip show` / equivalent.
5. **Score FP risk**:
   - `low` — verified directly, no convention override, no version ambiguity.
   - `medium` — plausible but couldn't fully verify (e.g., couldn't run cargo tree).
   - `high` — based on assumption that may not hold (e.g., assumed default-features but didn't verify).
6. **Document the reasoning** in `fp_risk_self_assessment.reasoning`.

### 11.3 False Negative Categories (Squad MUST Enumerate Likely FNs)

A **False Negative (FN)** is a real issue the squad did NOT flag. Each squad MUST end its run with a `falseNegativesIdentified` list (in the summary) enumerating likely FNs in its domain.

| FN Category | Description | Example | Mitigation |
|-------------|-------------|---------|------------|
| **FN-1: Unaudited Files** | Files the squad didn't read | code-quality missed `crates/buff-{framework}/src/` because there are 44 of them | List top-N files NOT examined by line count |
| **FN-2: Uncovered Patterns** | Patterns not in the checklist | security didn't audit WebSocket-specific attacks (CSWSH, origin spoofing) | Note the gap explicitly |
| **FN-3: Tool Limitations** | Tool couldn't see the issue | grep can't detect type-system issues; ast-grep missed a macro expansion | Document tool blind spots |
| **FN-4: Context Window Limits** | Squad ran out of context before finishing | Large file (10k+ LOC) only partially scanned | Cite which files/sections were truncated |
| **FN-5: Cross-Cutting Gaps** | Issue spans squads, neither covered it | supply-chain risk lives between security+cicd+license | Note the gap; route to consistency-coherence |
| **FN-6: Out-of-Scope by Design** | Squad explicitly excluded it | test-coverage didn't assess E2E tests (out of scope) | Document scope boundary |
| **FN-7: Time-Bound FNs** | New CVEs/issues since the audit window | CVE published yesterday not in cargo-audit db yet | Note the audit window cutoff |
| **FN-8: Implicit Assumptions** | Squad trusted something without verifying | Trusted Cargo.lock matches Cargo.toml without checking | Document unverified assumptions |

### 11.4 FN Enumeration Procedure (MANDATORY per squad)

Each squad MUST end its summary with a `falseNegativesIdentified` array. For each entry:

```json
{
  "area": "Specific path/pattern/module that may have unfound issues",
  "reason": "Why this is a likely FN — what was NOT examined and why",
  "suggestedSquad": "Which squad (or this squad with more time) should re-examine",
  "severity_if_found": "critical|high|medium|low|unknown",
  "suggestedVerificationCommand": "rg/grep/cargo command that would surface the FN if it exists"
}
```

**Minimum FN count**: Each squad MUST list AT LEAST 3 likely false negatives. If fewer than 3 come to mind, the squad hasn't thought hard enough about its blind spots — keep listing until 3.

### 11.5 Cognitive Discipline Rules

1. **No finding ships without FP self-assessment.** Missing `fp_risk_self_assessment` → REJECTED.
2. **No squad report ships without FN enumeration.** Missing `falseNegativesIdentified` with ≥3 entries → REJECTED.
3. **High-confidence findings can still be FPs.** Confidence ≠ FP-risk. A finding can be 0.99 confident the code is real, AND 0.5 likely to be a convention-override FP.
4. **Be brutally honest.** FNs documented now become tomorrow's coverage improvements. FPs caught now save reviewer trust.
5. **The audit-consolidated orchestrator** will independently re-verify a sample of findings (see audit-consolidated Section 5).

---

## Section 12: Coverage Accounting (v3.1 MANDATORY)

**Goal**: Measure examined, not just found. FNs (§11.4) estimated; coverage measured. 10k-LOC file with zero findings AND zero examinations ≠ clean — it's a hole.

### 12.1 Mandatory `files_examined` Reporting

Every squad MUST emit `summary.files_examined[]` — one object per file actually read (not just files with findings). Schema in §7.4.

Per file examined: `path` (repo-relative), `lines_read` (actually read, not file length), `bytes_read`, `examined_at` (ISO), `findings_emitted` (IDs, empty array if clean), `verdict` (`findings`/`clean`/`skipped-convention`).

### 12.2 Mandatory `files_skipped` Reporting

Every squad MUST also emit `summary.files_skipped[]` for files in scope that it intentionally did NOT examine:
- `path` — repo-relative path
- `reason` — why skipped (e.g., `vendored`, `generated`, `out-of-scope`, `context-window-limit`, `time-budget`)
- `lines_not_examined` — LOC that were not looked at

### 12.3 Coverage Stats (auto-computed by orchestrator)

The orchestrator computes per-squad and overall coverage stats:

```json
"coverage_stats": {
  "total_source_files_in_repo": 187,
  "files_examined_count": 42,
  "coverage_percent": 22.5,
  "lines_examined": 4280,
  "total_source_lines": 23410,
  "line_coverage_percent": 18.3,
  "by_squad": {
    "audit-code-quality": { "files": 32, "lines": 3120, "coverage_percent": 17.1 },
    "audit-security": { "files": 18, "lines": 1840, "coverage_percent": 9.6 }
  },
  "uncovered_high_risk_files": [
    {
      "path": "src/Modules/Billing/LegacyInvoice.cs",
      "loc": 1840,
      "zero_examinations": true,
      "zero_findings": true,
      "risk_score": 0.84,
      "reason": "Large file with zero coverage across all squads"
    }
  ]
}
```

### 12.4 Coverage Discipline Rules

1. **Files examined must be reported even if no findings** — negative space is the value.
2. **Files skipped must be reported with reason** — silent skipping forbidden.
3. **Coverage < 30%** → warning in consolidated report.
4. **Large files with zero coverage** (top-N by LOC) MUST appear in `uncovered_high_risk_files[]` regardless of FN enumeration — measured, not estimated.
5. **Heavily-churned files with zero coverage** (top-N by recent commits) MUST also appear — recent churn = leading indicator of new issues.

---

## Section 13: Delta Tracking Fields (v3.1 — orchestrator-computed)

**Goal**: Track findings across runs. Fixed-in-v1-then-regressed-in-v2 = worse than brand-new. Without delta tracking, every audit = snapshot, not trajectory.

### 13.1 Fingerprint Computation (deterministic, stable across line drift)

Each finding gets a `fingerprint` — a 16-char SHA-256 truncation that survives line-number drift:

```
fingerprint = sha256(
  normalize_path(file) + "::" +
  normalize_category(category) + "::" +
  normalize_root_cause(root_cause)
)[:16]
```

**Normalization rules** (so the same issue produces the same fingerprint across runs):
- `normalize_path`: lower-case, forward slashes, strip leading `./`, collapse `..` segments to absolute.
- `normalize_category`: lower-case, strip whitespace.
- `normalize_root_cause`: lower-case, strip punctuation, collapse whitespace, remove line-number references (which drift), remove version-specific tokens (which change).

**Why root_cause and not description?** Description often includes the line number or version, which drift. Root cause is the structural reason — stable.

**Squads MAY provide their own `fingerprint`** (if they have a more stable signal). If absent, orchestrator computes it.

### 13.2 Status Computation (delta vs `--previous` report)

Given the current audit's findings and the previous audit's findings (loaded via `audit --previous path/to/old.json`), each finding gets a `status`:

| Status | Condition | Action |
|--------|-----------|--------|
| `new` | Fingerprint not in previous audit | Highlight — brand-new issue (could be a recent regression in code, or just newly-discovered) |
| `recurring` | Fingerprint in previous audit, same severity | Surface aging — how long has this been open? |
| `regressed` | Fingerprint in previous audit, severity now HIGHER | Escalation — was medium, now critical |
| `improved` | Fingerprint in previous audit, severity now LOWER | Acknowledge progress |
| `unchanged` | Fingerprint in previous audit, identical severity + line range | Routine recurring |
| `fixed` | Fingerprint in previous audit, NOT in current audit | Celebrate — track as `fixed_findings[]` |

### 13.3 Delta Output Schema

The consolidated report includes a `delta` block:

```json
"delta": {
  "previous_report_path": ".sisyphus/reports/buff-audit-2026-07-26-1514.json",
  "previous_report_timestamp": "2026-07-26T15:14:00Z",
  "current_report_timestamp": "2026-08-15T10:00:00Z",
  "summary": {
    "new_count": 18,
    "recurring_count": 89,
    "regressed_count": 4,
    "improved_count": 7,
    "unchanged_count": 78,
    "fixed_count": 23
  },
  "regressed_findings": [
    {
      "fingerprint": "a3f4b2c1d9e8f7a6",
      "current_severity": "critical",
      "previous_severity": "medium",
      "current_id": "sec-014",
      "previous_id": "sec-009",
      "reason_for_regression": "Severity escalated — likely due to newly-discovered exploit or environmental change"
    }
  ],
  "fixed_findings": [
    {
      "fingerprint": "b4e5c3d2f1a0b9c8",
      "previous_severity": "high",
      "previous_id": "cq-007",
      "fixed_in_commit": "abc1234 (if discoverable via git log)",
      "fixed_at": "2026-08-10T14:23:00Z"
    }
  ],
  "aging_findings": [
    {
      "fingerprint": "c5f6d4e3a2b1c0d9",
      "current_id": "cq-001",
      "first_seen": "2026-06-04T10:00:00Z",
      "days_open": 72,
      "audits_seen_in": 4,
      "recommendation": "Long-standing critical — consider explicit accept-risk decision rather than repeated flagging"
    }
  ]
}
```

### 13.4 Aging Discipline Rules

1. **Open >90 days** with no severity change → `aging` flag — fix, escalate, or accept-risk via baseline (§17).
2. **Regressions > new findings** — regression = fix reverted or control bypassed. Surface prominently.
3. **Fixed findings celebrated** — `fixed_findings[]` shows progress, not just problems.
4. **First-seen persists across runs** — propagate from previous `first_seen` if recurring, else current timestamp.

---

## Section 14: Squad-Level File Persistence (v3.2 MANDATORY — Resilience Layer)

**Goal**: Make squad findings durable on disk the moment produced. Orchestrator death at any later phase (validation, consolidation, emit) ≠ squad data loss. + checkpoint protocol + `audit resume` (see `audit-consolidated` v3.2 + `commands/audit.md` v3.2) = 45-min restart → 5-10 min resume.

### 14.1 Why This Exists

Pre-v3.2: squad findings existed only in orchestrator's in-memory context after squad sub-tasks completed. Orchestrator killed (stale timeout, OOM, cancel) → squad sub-tasks cascade-deleted → findings lost. Two failed runs in this codebase cost ~90 min duplicated squad work.

v3.2: each squad writes output DIRECTLY to disk before returning. Orchestrator reads from disk for consolidation, not in-memory context.

### 14.2 Mandatory Squad Output File

Every squad MUST write its complete output (findings + summary + files_examined + falseNegativesIdentified) to a JSON file before returning to the orchestrator.

**File path**: `.audit/in-progress/{run-id}/squads/audit-{squad-name}.json`

- `{run-id}`: ISO timestamp `{YYYYMMDD-HHMM}` (e.g., `2026-07-26-2200`). Provided by the orchestrator in the squad invocation prompt. Unique per audit run.
- `{squad-name}`: the squad's name without `audit-` prefix (e.g., `code-quality`, `security`).

### 14.3 Squad Output File Schema

The file MUST conform to the full squad report schema (same as v3.0+ Section 7.4):

```json
{
  "report_version": "3.2",
  "squad": "audit-{squad-name}",
  "run_id": "{run-id}",
  "project": "detected-project-name",
  "timestamp": "2026-01-15T10:30:00Z",
  "technologies": ["..."],
  "findings": [
    {
      "id": "{squad-short}-001",
      "severity": "...",
      "file": "...",
      "line": 42,
      "title": "...",
      "description": "...",
      "root_cause": "...",
      "impact": "...",
      "recommendation": "...",
      "fix_examples": { "before": "...", "after": "..." },
      "fix_guide": "...",
      "effort_estimate": { "minutes": 240, "human_readable": "4 hours", "confidence": "high", "breakdown": ["..."] },
      "technology": "...",
      "category": "...",
      "detection_method": { "tool": "...", "query": "..." },
      "verification_command": "...",
      "references": ["..."],
      "related_findings": ["..."],
      "confidence": { "score": 0.92, "rationale": "..." },
      "fp_risk_self_assessment": { "risk_level": "low", "reasoning": "...", "verified_against_conventions": true, "convention_files_checked": ["..."] }
    }
  ],
  "summary": {
    "totalFindings": 15,
    "bySeverity": { "critical": 1, "high": 3, "medium": 7, "low": 4 },
    "byCategory": { "...": 5 },
    "files_examined": [
      { "path": "...", "lines_read": 78, "bytes_read": 2340, "examined_at": "...", "findings_emitted": ["..."], "verdict": "findings|clean|skipped-convention" }
    ],
    "files_skipped": [
      { "path": "...", "reason": "vendored", "lines_not_examined": 5234 }
    ],
    "falseNegativesIdentified": [
      { "area": "...", "reason": "...", "suggestedSquad": "...", "severity_if_found": "...", "suggestedVerificationCommand": "..." }
    ]
  }
}
```

### 14.4 Mandatory Write Procedure (per squad)

1. Produce findings in v3.1 schema (§7.1, §7.2, §7.4 + §11-13).
2. Build squad output file JSON per §14.3.
3. **Write to disk via `write` tool** at `.audit/in-progress/{run-id}/squads/audit-{squad-name}.json`. Create dir if missing.
4. **Verify file exists** (`ls .audit/in-progress/{run-id}/squads/audit-{squad-name}.json`).
5. Return SHORT summary — do NOT echo findings (on disk now; echo wastes tokens).
   - Return: `WROTE .audit/in-progress/{run-id}/squads/audit-{squad-name}.json (N findings, M files examined, K FNs)`
6. Write failed → retry once. Second failure → return error to orchestrator with reason.

### 14.5 Discipline Rules

1. **On-disk file = source of truth.** Orchestrator in-memory copy diverges → on-disk wins.
2. **Append-only for run duration.** Re-launched squad OVERWRITES own file. No `.{n}.json` versioning within run.
3. **Files MUST be valid JSON.** Write `.{file}.tmp` first, atomic rename:
   ```
   write .audit/in-progress/{run-id}/squads/audit-{name}.json.tmp
   rename → .audit/in-progress/{run-id}/squads/audit-{name}.json
   ```
4. **Orchestrator MUST NOT echo squad findings.** Reads disk for consolidation. Echo wastes context.
5. **`.audit/in-progress/` not gitignored by default.** Partial state visibility helps debugging. After success, move to `.audit/completed/{run-id}/` or delete (default: keep 7 days).

### 14.6 Why the Orchestrator Benefits

Pre-v3.2 orchestrator: launch 13 squads → wait → read all 13 from in-memory (~65K tokens just for findings) → validate, consolidate, emit.

v3.2 orchestrator: generate `{run-id}`, write checkpoint `phase=squad-execution` → launch 13 squads → wait — each returns 1-line "WROTE ..." ack → read squad files FROM DISK when needed (stream, not all at once) → update checkpoint per phase → consolidate + emit → on success rename `.partial.json` → final `.json`, optionally cleanup.

Orchestrator context stays small (reads what it needs when it needs). Squad data persists independently. Win-win.

### 14.7 Resume Scenario (forward reference)

Orchestrator dies mid-run → next `audit resume {run-id}`:
1. Read `.audit/in-progress/{run-id}/checkpoint.json` → determine phase.
2. List `.audit/in-progress/{run-id}/squads/*.json` → see which squads wrote output.
3. Re-launch ONLY missing squads (if any).
4. Read existing squad files for validation + consolidation.
5. Continue from next phase.

See `audit-consolidated` v3.2 §19 for full resume protocol.

### 14.8 Backward Compatibility

- v3.0/v3.1 squads (no file persistence) still work — just don't write to disk. Orchestrator detects, falls back to in-memory aggregation.
- Log entry `squad_persisted: false` for any squad without file, reason "pre-v3.2-squad".
- Resume from v3.0/v3.1 in-progress run = NOT possible (no squad files) — orchestrator reports explicitly, suggests fresh run.

---

## Section 15: Chunked Atomic Write Protocol (v3.2 MANDATORY — Anti-Stall Layer)

**Goal**: Prevent agent stall/timeout writing large files. Complete v3.1+ audit = 200-500 KB JSON. Single `write` call risks: tool size limits (silent truncation/error), agent stall during huge output emission, partial-write corruption if killed mid-emit, context-window exhaustion.

**Hard rule**: ANY file >50 KB MUST be written in chunks via temp-then-rename. Smaller files MAY be single call.

### 15.1 The Three-Step Chunked Write Pattern

For any file > 50 KB (or any file containing > 20 findings):

```
Step 1: Write the first chunk to {filename}.tmp using the `write` tool.
        (e.g., header + metadata + first 10 findings as a complete JSON document with `findings: [...]`)

Step 2: APPEND subsequent chunks to {filename}.tmp using the `edit` tool
        (insert before the closing `]` or `}` to extend arrays/objects).
        - Chunk size: 10-20 findings per edit, or ~50 KB per edit (whichever is smaller).
        - After each chunk: verify the file is still valid JSON (a quick read + parse).
        - If the agent feels context pressure, use `compress` between chunks.

Step 3: ATOMIC RENAME {filename}.tmp → {filename}
        - Verify the .tmp file is complete and valid JSON.
        - Rename (atomic on most filesystems) — single `mv`/`Move-Item` call.
        - The final {filename} only ever appears in its complete state.
```

### 15.2 Why This Prevents Stalls

| Pattern | Risk | v3.2 Fix |
|---------|------|----------|
| Single 500KB write | Agent stalls during huge output emission; tool may truncate | Max 50KB per write call; multiple smaller emissions |
| Killed mid-write | File exists but truncated/invalid JSON | `.tmp` extension until atomic rename; final filename only appears when complete |
| Context exhaustion | Agent runs out of context before finishing write | Chunks are bounded; agent can `compress` between chunks |
| Tool size limits | Silent truncation past N KB | Each chunk ≤50KB, well under any tool limit |

### 15.3 Squad Output Files (Section 14) — Chunking Rules

A squad output file typically contains 5-15 findings + summary + files_examined. Usually <50 KB.

- **If <50 KB**: single `write` call (Section 14.4 procedure).
- **If ≥50 KB** (large squad with many findings or long `fix_examples`): apply the three-step pattern.
  - Step 1: write `.tmp` with header + first 5 findings
  - Step 2: append remaining findings in chunks of 5
  - Step 3: append summary block + close JSON, then rename

### 15.4 Consolidated Report (audit-consolidated Section 18) — Chunking Rules

The consolidated report is the biggest file (200-500 KB). Mandatory chunking:

1. **Initial skeleton** (Step A): Write `.tmp` with metadata + empty findings[] + empty structural blocks. ~5 KB.
2. **Per-squad-group findings** (Step B): After each squad group completes (or after each squad if solo), append that group's findings to `.tmp` via `edit` (insert before the closing `]` of `findings`). ~30-50 KB per squad group.
3. **Structural blocks** (Step C): Append `delta`, `coverage_map`, `fix_dependency_dag`, `false_positives`, `false_negatives`, `provenance` blocks. ~20-40 KB.
4. **Atomic rename** (Step D): Verify `.tmp` is valid JSON, rename to final filename.

### 15.5 Chunked Write Discipline Rules

1. **Every chunk MUST be valid JSON after write.** Verify after each `edit` (quick Read/jq parse).
2. **Chunk cap**: 50 KB OR 20 findings, whichever smaller. Single finding >50 KB (rare, long `fix_examples`) → split fields across edits.
3. **Atomic rename ONLY at end.** Never rename `.tmp` → final until file complete + validated.
4. **`compress` between chunks OK** if context tight. Does NOT affect on-disk state.
5. **Killed mid-chunk**: `.tmp` recoverable. Orchestrator (or resume run) reads `.tmp`, validates up to truncation, continues.
6. **NEVER write final filename directly** for files >50 KB — always `.tmp` + rename.
7. **SARIF, Markdown, GitHub comment**: same rule if >50 KB. Markdown split by section (exec summary first, per-finding appended).

### 15.6 Worked Example: Writing a 77-Finding Consolidated JSON

```
[Step A: skeleton ~5KB]
write buff-audit-{ts}-v3.json.tmp with:
  {
    "report_version": "3.2", "type": "consolidated", "project": "buff",
    "timestamp": "...", "severity_breakdown": {...},
    "findings": [],  ← empty for now
    "delta": null, "coverage_map": null, "fix_dependency_dag": null,
    "false_positives": [], "false_negatives": [],
    "provenance": {...}
  }

[Step B1: append Group 1 findings ~30KB after squad group 1 completes]
edit buff-audit-{ts}-v3.json.tmp:
  oldString: '"findings": ['
  newString: '"findings": [\n  {first 20 findings...}'

[Step B2: append Group 2 findings ~25KB]
edit buff-audit-{ts}-v3.json.tmp:
  oldString: '\n  {last finding of group 1}\n]'
  newString: '\n  {last finding of group 1},\n  {group 2 findings...}\n]'

[Step B3-B6: append groups 3-6 similarly]

[Step C: append structural blocks ~30KB]
edit buff-audit-{ts}-v3.json.tmp:
  oldString: '"delta": null,'
  newString: '"delta": {full delta block},'

[Step D: verify + rename]
Verify: Read buff-audit-{ts}-v3.json.tmp, parse as JSON, confirm findings.length == 77
Rename: Move-Item buff-audit-{ts}-v3.json.tmp → buff-audit-{ts}-v3.json
```

Each individual tool call is small (<50 KB). No single call can stall the agent. The final filename only appears when complete.

### 15.7 Backward Compatibility

Pre-v3.2 orchestrators that wrote files in a single call still work — they just don't benefit from chunking. A v3.2 orchestrator reading pre-v3.2 output files is unaffected (the file format is the same; only the write strategy differs).
