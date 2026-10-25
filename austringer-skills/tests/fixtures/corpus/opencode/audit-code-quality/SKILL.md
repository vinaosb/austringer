---
name: audit-code-quality
description: >-
  Code quality auditing: detects code smells, measures cyclomatic complexity,
  identifies dead code, enforces naming conventions, and evaluates readability
  metrics across multi-language codebases. Produces structured findings with
  severity ratings and actionable refactoring recommendations.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java"]
version: "2.0.0"
---

# Code Quality & Complexity Audit

## USE FOR

- Code smell detection across Python, JS/TS, C#, Go, Java
- Cyclomatic complexity — flag functions >10 branches
- Dead code — unreachable code, unused imports/vars/branches
- Naming conventions — PEP 8, camelCase/PascalCase, C# conventions
- Duplicate code — similar blocks indicating copy-paste
- Readability — long functions (>50 lines), deep nesting (>4 levels), long params (>5)
- File metrics — LOC, comment ratio, class count, god class detection
- Analyzer warnings (CAxxxx) with TreatWarningsAsErrors compliance
- File-scoped namespace + `var` preference in C#
- Type hint completeness + PEP 8 in Python
- ESLint/TypeScript strict mode compliance

## DO NOT USE FOR

- Architecture/layer isolation (use `audit-architecture`)
- Security vulnerabilities (use `audit-security`)
- Modernization/library upgrades (use `audit-refactoring`)
- Test coverage/assertion quality (use `audit-test-coverage`)
- Dependency CVE/license (use `audit-dependencies`)
- CI/CD pipeline validation (use `audit-cicd`)

---

## Target Discovery

Agent MUST auto-detect project structure. Do NOT hardcode repository names.

### Language Detection

| Signal | File Pattern | Detected Language |
|--------|-------------|-------------------|
| C# source files | `*.cs`, `*.csproj`, `*.sln`, `*.slnx` | .NET/C# |
| Python source files | `*.py`, `*.pyi`, `pyproject.toml`, `requirements.txt` | Python |
| JavaScript source files | `*.js`, `*.mjs`, `*.cjs` | JavaScript |
| TypeScript source files | `*.ts`, `*.tsx`, `tsconfig.json` | TypeScript |
| Go source files | `*.go`, `go.mod` | Go |
| Java source files | `*.java`, `pom.xml`, `build.gradle` | Java |
| Rust source files | `*.rs`, `Cargo.toml`, `Cargo.lock` | Rust |

### Framework Detection

| Signal | File Pattern | Detected Framework |
|--------|-------------|--------------------|
| ASP.NET Core | `Program.cs` + `*.csproj` with `Microsoft.AspNetCore` | ASP.NET Core |
| Blazor | `*.razor`, `*.razor.cs` | Blazor |
| EF Core | `*.csproj` with `Microsoft.EntityFrameworkCore` | Entity Framework Core |
| Django | `manage.py`, `settings.py` | Django |
| FastAPI | `*.py` importing `fastapi` | FastAPI |
| Express | `package.json` listing `express` | Express.js |
| NestJS | `package.json` listing `@nestjs/core` | NestJS |
| Spring Boot | `pom.xml`/`build.gradle` with `spring-boot` | Spring Boot |

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
| Terraform | `*.tf` | IaC |
| Database migrations | `migrations/`, `Migrations/`, `alembic/`, `db/migrate/` | Schema |
| OTel | `otel-collector-config.yml`, `opentelemetry` in deps | Observability |
| Grafana | `grafana/`, `grafana.ini`, `provisioning/` | Dashboards |
| Prometheus | `prometheus.yml`, `prometheus/` | Metrics |

### Discovery Procedure

1. **Scan root** for file patterns.
2. **Identify language** by file count.
3. **Identify frameworks** by config analysis.
4. **Identify CI/CD** by workflow files.
5. **Identify infrastructure** by Docker/ORM/observability.
6. **Select checklist** from Section 4.
7. **Multiple languages** → run all applicable.

---

## Technology-Specific Checklists

### .NET/C# Checklist

- [ ] **CAxxxx analyzer warnings**: Check suppressed warnings (`#pragma warning disable`). `TreatWarningsAsErrors` should be enabled. Common suppressions:
  - CA1716: Reserved keywords as property names (ViewModels)
  - CA1707: Underscore in member names (scaffold/reporting files)
  - CA1031: Catch-all handlers (must document justification)
  - CA1822: Static method candidates
  - CA1711: EventHandler suffix on non-event members
  - CA1848: LoggerMessage for high-perf logging
  - CA1869: JsonSerializerOptions caching
- [ ] **TreatWarningsAsErrors**: Verify in `Directory.Build.props` or project files. All warnings block build.
- [ ] **File-scoped namespaces**: Enforced via `csharp_style_namespace_declarations = file_scoped:warning`. No block-scoped.
- [ ] **`var` preferred**: Everywhere including built-in types. `csharp_style_var_for_built_in_types = true:warning`.
- [ ] **Dead code**: Unused params, unreachable branches, unused private methods, commented-out blocks. IDE0051, IDE0052, IDE0059.
- [ ] **God classes**: >300 LOC, >10 public methods, or >20 fields → split.
- [ ] **Naming**: PascalCase properties/methods, camelCase locals/params, `_camelCase` private fields. No `this.`.
- [ ] **Expression-bodied**: Properties when single-line. Methods only when single expression.
- [ ] **No `this.` qualification**: Fields, properties, methods — no `this.` prefix.
- [ ] **EnforceCodeStyleInBuild**: Verify code style in build config.
- [ ] **AnalysisLevel**: `latest-recommended` for max analyzer coverage.

### Python Checklist

- [ ] **PEP 8**: Line length >88 (black), indentation, blank lines. `flake8` or `ruff`.
- [ ] **Type hints**: All public functions annotated. `mypy --strict` passes. No `Any` without justification. Return types always specified.
- [ ] **Complexity (McCabe)**: Functions >10 flagged. Use `mccabe` or `ruff`.
- [ ] **Dead imports**: Unused via `autoflake`/`ruff`. Star imports (`from module import *`) = high severity.
- [ ] **God modules**: >500 LOC, >15 top-level functions or >8 classes → split.
- [ ] **Long functions**: >50 lines → extract.
- [ ] **Deep nesting**: >4 levels → early returns or extraction.
- [ ] **Magic numbers**: Unexplained literals → named constants or enums.
- [ ] **String formatting**: f-strings only (Python 3.6+), no `.format()` or `%`.

### JavaScript/TypeScript Checklist

- [ ] **ESLint**: Configured and passing. No `eslint-disable` without justification. TS ESLint parser for `.ts`/`.tsx`.
- [ ] **TypeScript strict**: `"strict": true` in `tsconfig.json`. `strictNullChecks` on. No `any` without justification.
- [ ] **Dead exports**: Exported but never imported. `ts-prune` or `eslint-plugin-unused-imports`.
- [ ] **Complex functions**: >10 branches or 50 lines → extract.
- [ ] **Callback hell**: Nested callbacks >3 levels → async/await or Promise chains.
- [ ] **Console.log in prod**: Use proper logging library.
- [ ] **Naming**: camelCase vars/functions, PascalCase classes/types/interfaces, UPPER_SNAKE_CASE constants.
- [ ] **Optional chaining**: `?.` instead of manual null checks.
- [ ] **Nullish coalescing**: `??` instead of `||` for falsy-but-valid (0, empty string).

### Go Checklist

- [ ] **Error handling**: No ignored errors (`_ = someFunc()`). Wrap with context (`fmt.Errorf("doing X: %w", err)`). Custom error types for domain.
- [ ] **Naming**: CamelCase exported, camelCase unexported. No snake_case. Acronyms all-caps (`HTTP`, `URL`, `ID`).
- [ ] **Complexity**: >10 branches or 50 lines. Nesting >4 levels.
- [ ] **Dead code**: Unused functions/vars/imports via `go vet`. Commented-out blocks.
- [ ] **God packages**: >500 LOC, >10 top-level functions or >5 types → split.
- [ ] **Context propagation**: All I/O functions accept `context.Context` first. No context in structs.
- [ ] **Struct initialization**: No zero-value where invalid. Constructor functions for complex types.
- [ ] **Goroutine management**: No unbounded spawns. `sync.WaitGroup` or errgroup for coordination.

### Java Checklist

- [ ] **Naming**: PascalCase classes, camelCase methods/vars, UPPER_SNAKE_CASE constants. No Hungarian.
- [ ] **Complexity**: >10 branches or 50 lines. Nesting >4 levels.
- [ ] **Dead code**: Unused imports, private methods, unreachable branches. IDE inspections.
- [ ] **God classes**: >300 LOC, >15 methods or >10 fields.
- [ ] **Null safety**: No `null` returns from public methods. `Optional<T>` for nullable. `@NonNull`/`@Nullable`.
- [ ] **Exception handling**: No bare `catch (Exception)` or `catch (Throwable)`. Specific types. Try-with-resources for `AutoCloseable`.
- [ ] **Resource management**: No unclosed streams/connections. Try-with-resources for I/O.
- [ ] **Concurrency**: `java.util.concurrent` over `synchronized`. Thread-safe collections. No shared mutable static state.
- [ ] **Logging**: SLF4J + Logback/Log4j2. No `System.out.println`. Structured logging.

### Rust Checklist

- [ ] **Unsafe usage**: Documented safety invariants. Minimized surface. No unnecessary unsafe.
- [ ] **Error handling**: `Result<T, E>` for fallible ops. No `unwrap()`/`expect()` in library code. `thiserror`/`anyhow`.
- [ ] **Naming**: snake_case functions/vars, PascalCase types/traits, SCREAMING_SNAKE_CASE constants.
- [ ] **Complexity**: >10 branches or 50 lines. Pattern matches >3 levels.
- [ ] **Dead code**: `#[allow(dead_code)]` on unused items. Clippy flags unused imports.
- [ ] **God modules**: >400 LOC, >10 public functions or >5 types → split by domain.
- [ ] **Clippy**: `cargo clippy` passes. Pedantic lints where practical.
- [ ] **Concurrency**: `Send`/`Sync` bounds understood. No data races. `Arc<Mutex<T>>`/channels for shared state.

---

## Anti-Patterns

### Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | God class/module | high | >10 responsibilities or >300 LOC | SRP violation, hard to test, merge conflicts | Focused classes/modules |
| 2 | Copy-paste code | medium | >80% similar blocks across files | Multiple edits needed, inconsistencies | Extract shared functions, composition |
| 3 | Magic numbers/strings | medium | Unexplained literals | Unclear intent, fragile | Named constants or enums |
| 4 | Deep nesting | medium | >4 levels | Hard to read/test, high complexity | Early returns, guard clauses, extract |
| 5 | Long parameter lists | low | >5 parameters | Hard to call, extend, error-prone | Options object, builder pattern |

### .NET/C# Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | God handler (CRUD/auth/encryption) | high | One handler: create, read, update, delete, auth, encryption | SRP violation, untestable | One command per handler |
| 2 | Stub returning empty silently | high | Mock returning empty collections without logging | Tests pass but validate nothing | Meaningful data or `NotImplementedException` |
| 3 | Validator true for unknown countries | high | Unknown countries pass validation | Bypasses validation | False for unknown, maintain whitelist |
| 4 | Unjustified CA1031 suppression | medium | `#pragma warning disable CA1031` without reason | Swallows all exceptions | Catch specific; document catch-all reason |
| 5 | Block-scoped in file-scoped project | low | `namespace Foo { ... }` in strict project | Inconsistent style | `namespace Foo;` file-scoped |

### Python Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | God module | high | .py >500 LOC, >15 functions | Hard to navigate/test/maintain | Split by responsibility |
| 2 | Star imports | medium | `from module import *` | Namespace pollution, breaks static analysis | `from module import func` |
| 3 | Bare except | high | `except:` without type | Catches KeyboardInterrupt, hides bugs | Specific exception types |
| 4 | Mutable default args | high | `def foo(items=[])` | List persists across calls | `None` default, new list inside |
| 5 | Missing type hints on public API | medium | Public functions without annotations | Unclear contracts, breaks IDE/mypy | Type hints on all public functions |

### JavaScript/TypeScript Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | `any` type in library code | high | `any` instead of specific types | Defeats type checking | Specific types, `unknown`, generics |
| 2 | Callback hell | medium | Nested callbacks >3 levels | Unreadable, error-prone | async/await or flat Promises |
| 3 | Console.log in prod | low | `console.log` in production | Performance, exposes data | Structured logging library |
| 4 | Unhandled rejections | high | Missing `.catch()` or try/catch | Crashes, warnings | Always handle async errors |
| 5 | Default exports | low | `export default function()` | Hard to refactor, poor tree-shaking | Named exports |

### Go Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Ignored errors | high | `_ = someFunc()` or no error check | Silent failures, data loss | `if err != nil` always |
| 2 | Missing context propagation | medium | I/O without `context.Context` | No timeout/cancellation, goroutine leaks | Context as first param |
| 3 | God package | high | .go >500 LOC, >10 types | Hard to navigate/test | Split by responsibility |
| 4 | Zero-value struct without validation | medium | Invalid zero-value state | Bugs from uninitialized fields | Constructor with validation |

### Java Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Catching Exception and ignoring | high | `catch (Exception e) {}` | Swallows all errors | Specific types, log and rethrow |
| 2 | Null returns | medium | `null` instead of `Optional<T>` | NPE risk in callers | `Optional<T>` or empty collection |
| 3 | God class | high | >300 LOC, >15 methods | SRP violation, merge conflicts | Split by responsibility |
| 4 | System.out.println in prod | low | stdout instead of logging | No levels, no formatting | SLF4J throughout

---

## Code Examples (Bad/Good Pairs)

### .NET/C# Example 1: God Handler Split into Focused Handlers

```csharp
// ❌ BAD: Single handler does everything — CRUD, auth, encryption
public class PersonHandler
{
    public async Task<Result> Handle(CreatePersonCommand cmd) { /* create logic */ }
    public async Task<Result> Handle(UpdatePersonCommand cmd) { /* update logic */ }
    public async Task<Result> Handle(DeletePersonCommand cmd) { /* delete + auth check */ }
    public async Task<PersonDto> Handle(GetPersonQuery query) { /* read + decrypt */ }
    public async Task<List<PersonDto>> Handle(ListPeopleQuery query) { /* list + filter */ }
    private bool CanAccess(Person person) { /* authorization */ }
    private string Decrypt(string value) { /* encryption */ }
    // 400+ lines, impossible to test in isolation
}

// ✅ GOOD: Focused handlers — one responsibility each
public class CreatePersonHandler
{
    public async Task<Result> Handle(CreatePersonCommand cmd) { /* create only */ }
}

public class GetPersonHandler
{
    private readonly IEncryptionService _encryption;
    public async Task<PersonDto> Handle(GetPersonQuery query) { /* read + decrypt */ }
}
```

### .NET/C# Example 2: Proper Analyzer Suppression with Justification

```csharp
// ❌ BAD: Suppression without reason — hides the warning but not the problem
#pragma warning disable CA1031
try
{
    await _service.ProcessAsync(data);
}
catch (Exception)
{
    _logger.LogError("Processing failed");
}
#pragma warning restore CA1031

// ✅ GOOD: Documented justification with specific exception handling
#pragma warning disable CA1031 // Intentionally catch-all: external service may throw unknown exceptions.
                                // Logged with full context for diagnosis. See ADR-0042.
try
{
    await _service.ProcessAsync(data);
}
catch (HttpRequestException ex)
{
    _logger.LogError(ex, "HTTP failure processing data {DataId}", data.Id);
    throw;
}
catch (TimeoutException ex)
{
    _logger.LogError(ex, "Timeout processing data {DataId}", data.Id);
    throw;
}
catch (Exception ex)
{
    _logger.LogError(ex, "Unexpected error processing data {DataId}", data.Id);
    throw;
}
#pragma warning restore CA1031
```

### .NET/C# Example 3: Validator Bypass Prevention

```csharp
// ❌ BAD: Unknown countries pass validation
public class CountryValidator : AbstractValidator<Address>
{
    public CountryValidator()
    {
        RuleFor(x => x.CountryCode)
            .Must(BeValidCountry)
            .WithMessage("Invalid country code");
    }

    private bool BeValidCountry(string code)
    {
        var valid = _knownCountries.Contains(code);
        return valid; // Unknown countries return false, but if list is empty, ALL pass
    }
}

// ✅ GOOD: Explicit whitelist with empty-list protection
public class CountryValidator : AbstractValidator<Address>
{
    private static readonly HashSet<string> ValidCountries = new(StringComparer.OrdinalIgnoreCase)
    {
        "US", "BR", "PT", "LU", "DE", "FR", "ES", "GB"
        // Add countries as needed — DO NOT default to "all valid"
    };

    public CountryValidator()
    {
        RuleFor(x => x.CountryCode)
            .NotEmpty()
            .Must(code => ValidCountries.Contains(code))
            .WithMessage("Country code '{PropertyValue}' is not supported. Valid codes: {ValidCodes}",
                string.Join(", ", ValidCountries));
    }
}
```

### Python Example 1: God Module Split

```python
# ❌ BAD: 600-line utils.py with everything
def validate_email(email): ...
def validate_phone(phone): ...
def calculate_tax(amount, rate): ...
def format_currency(amount): ...
def send_email(to, subject, body): ...
def send_sms(to, message): ...
def generate_report(data): ...
# 30+ more functions, unrelated responsibilities

# ✅ GOOD: Focused modules by domain
# validators.py
def validate_email(email: str) -> bool: ...
def validate_phone(phone: str) -> bool: ...

# financial.py
def calculate_tax(amount: float, rate: float) -> float: ...
def format_currency(amount: float) -> str: ...

# notifications.py
def send_email(to: str, subject: str, body: str) -> None: ...
def send_sms(to: str, message: str) -> None: ...
```

### JavaScript/TypeScript Example 1: Eliminating Dead Exports

```typescript
// ❌ BAD: Exported but never imported anywhere
export function formatDateForDisplay(date: Date): string {
    return date.toLocaleDateString("en-US", { year: "numeric", month: "long", day: "numeric" });
}

export function parseCustomDateString(str: string): Date | null {
    // Complex parsing logic — 30 lines
    // ... but nobody imports this function
}

// ✅ GOOD: Only export what's used; internal helpers stay unexported
function parseCustomDateString(str: string): Date | null {
    // Internal helper, not exported unless needed by other modules
}

export function formatDateForDisplay(date: Date): string {
    return date.toLocaleDateString("en-US", { year: "numeric", month: "long", day: "numeric" });
}
```

### Go Example 1: Error Handling — Ignored Errors

```go
// ❌ BAD: Error ignored — silent failure
func saveUser(db *sql.DB, user User) {
    db.Exec("INSERT INTO users (name, email) VALUES ($1, $2)", user.Name, user.Email)
    // Error return ignored — failure is invisible
}

// ✅ GOOD: Error checked and propagated
func saveUser(db *sql.DB, user User) error {
    _, err := db.Exec("INSERT INTO users (name, email) VALUES ($1, $2)", user.Name, user.Email)
    if err != nil {
        return fmt.Errorf("save user %s: %w", user.Name, err)
    }
    return nil
}
```

### Java Example 1: Null Returns and Missing Optional

```java
// ❌ BAD: Returning null — caller must remember to check
public User findUser(String email) {
    User user = userRepository.findByEmail(email);
    return user; // Returns null if not found — NullPointerException risk
}

// ✅ GOOD: Return Optional — caller knows result may be absent
public Optional<User> findUser(String email) {
    return Optional.ofNullable(userRepository.findByEmail(email));
}
```

---

## Output Format

All findings as structured JSON. One per object.

### Finding Schema

```json
{
  "severity": "critical|high|medium|low",
  "file": "relative/path/to/file",
  "line": 42,
  "endLine": 45,
  "description": "What was found. Include problematic code pattern and metric values.",
  "recommendation": "Fix with code example. Reference pattern or convention.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|CrossLanguage",
  "category": "maintainability|architecture|performance|security|compliance|testing|observability"
}
```

### Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Severity Guidelines.
- **file**: Relative path. Forward slashes. Never absolute.
- **line**: 1-based start line.
- **endLine**: Optional end line.
- **description**: Factual, specific. Include code fragment and metric value.
- **recommendation**: Actionable. Code snippet for fix when possible.
- **technology**: Primary technology. `CrossLanguage` only for multi-file patterns.
- **category**: Typically `maintainability`, `architecture`, or `performance`.

### Summary Report

After all findings:

```json
{
  "summary": {
    "squad": "audit-code-quality",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#", "Python"],
    "totalFindings": 15,
    "bySeverity": {
      "critical": 0,
      "high": 3,
      "medium": 7,
      "low": 5
    },
    "byCategory": {
      "maintainability": 8,
      "architecture": 4,
      "performance": 2,
      "security": 1
    },
    "filesScanned": 42,
    "filesWithFindings": 12
  }
}
```

---

## Severity Guidelines

### Base Severity Definitions

| Level | Definition | Code Quality Example |
|-------|-----------|---------------------|
| **critical** | Data integrity or system reliability risk | Validator bypass, stub returning empty in critical path |
| **high** | Significant maintainability/correctness risk | God class >300 LOC, complexity >20, unjustified catch-all |
| **medium** | Degraded quality/readability | Dead code, complexity 10-20, missing type hints, PEP 8, deep nesting |
| **low** | Best practice/style | Naming violations, missing docstrings, formatting |

### Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Auth/authorization code | +1 level | All technologies |
| Data validation/encryption | +1 level | All technologies |
| Multiple tenants affected | +1 level | Multi-tenant |
| Public-facing endpoint | +1 level | Web apps |
| Financial/billing data | +1 level | Financial |
| God class >500 LOC | +1 level | All technologies |

### Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Test/demo code only | -1 level (min low) | All technologies |
| Existing TODO/FIXME acknowledging it | -1 level (min low) | All technologies |
| Generated/scaffolded code | -1 level (min low) | All technologies |

### Technology-Specific Severity Adjustments

#### .NET/C#
- Unjustified CA1031 in production handler → high (hides exceptions)
- File-scoped violation in strict project → low (style)
- God handler (CRUD/auth/encryption) → high (SRP)
- Missing `var` in strict project → low (style)

#### Python
- Missing type hints on public API → medium (maintainability)
- Star import in production → medium (namespace pollution)
- Complexity >20 → high (maintainability)
- Legacy `.format()` → low (modernization)

#### JavaScript/TypeScript
- `any` in library code → medium (type safety)
- Unhandled rejection → high (reliability)
- Console.log in prod → low (style)
- Dead export → low (cleanup)

#### Go
- Ignored errors in I/O → high (silent failure)
- Missing context in network code → medium (timeout/cancellation)
- God package >500 LOC → high (maintainability)
- Zero-value struct without validation → medium (bug risk)

#### Java
- Null return from public method → medium (NPE risk)
- Catching Exception and ignoring → high (silent failure)
- God class >300 LOC → high (maintainability)
- System.out.println in prod → low (style)

---

## Integration Notes

### Squad Dependency Graph

```
audit-dependencies ──────────────────┐
audit-security ──────────────────────┤
audit-architecture ──────────────────┤
audit-code-quality ◄─────────────────┤  (may reference architecture)
audit-test-coverage ─────────────────┤
audit-refactoring ◄──────────────────┘  (consumes code-quality)
audit-consolidated ◄──────────────────── (consumes all)
audit-consistency-coherence            (consumes all)
```

### Upstream Dependencies

- **audit-architecture**: Architecture findings inform god class/layer isolation issues.
- **audit-dependencies**: Dependency data for deprecated package patterns.

### Downstream Consumers

- **audit-refactoring**: Consumes findings for modernization recommendations.
- **audit-consolidated**: Aggregates all findings.
- **audit-consistency-coherence**: Validates cross-domain alignment.

### Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-code-quality | audit-architecture | Code-quality: implementation smells; architecture: layer/module structure |
| audit-code-quality | audit-refactoring | Code-quality: identifies problems; refactoring: recommends approaches |
| audit-code-quality | audit-security | Code-quality: maintainability; security: vulnerability exploitation |

### Recommended Execution Order

This squad runs after:

1. **audit-dependencies** — Third-party risk baseline
2. **audit-architecture** — Structural analysis
3. **audit-code-quality** ← This squad

Downstream:
4. **audit-refactoring** — Uses findings for modernization
5. **audit-test-coverage** — Tests quality of code identified here

### Output Integration

When running sequentially:
1. Read previous findings from shared output.
2. Deduplicate: skip already-reported findings.
3. Cross-reference: escalate confirmed by multiple squads.
4. Output in Finding Schema format for aggregation.
