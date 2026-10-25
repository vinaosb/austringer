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

- Code smell detection across Python, JavaScript/TypeScript, C#, Go, and Java codebases
- Cyclomatic complexity analysis — flag functions/methods exceeding 10 branches
- Dead code identification — unreachable code, unused imports, unused variables, unreachable branches
- Naming convention enforcement — PEP 8 for Python, camelCase/PascalCase for JS/TS, C# conventions
- Duplicate code detection — similar blocks across files indicating copy-paste programming
- Code readability scoring — long functions (>50 lines), deep nesting (>4 levels), long parameter lists (>5 params)
- File-level metrics — lines of code, comment ratio, class count, god class detection
- Analyzer warning violations (CAxxxx) in .NET/C# projects with TreatWarningsAsErrors compliance
- File-scoped namespace enforcement and `var` preference validation in C# codebases
- Type hint completeness and PEP 8 compliance in Python modules
- ESLint/TypeScript strict mode compliance in JavaScript/TypeScript projects

## DO NOT USE FOR

- Architecture and layer isolation review (use `audit-architecture`)
- Security vulnerability scanning (use `audit-security`)
- Modernization and library upgrade recommendations (use `audit-refactoring`)
- Test coverage and assertion quality review (use `audit-test-coverage`)
- Dependency CVE and license auditing (use `audit-dependencies`)
- CI/CD pipeline validation (use `audit-cicd`)

---

## Target Discovery

The agent MUST auto-detect the project structure. Do NOT hardcode repository names.

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
| Terraform | `*.tf` | Infrastructure as Code |
| Database migrations | `migrations/`, `Migrations/`, `alembic/`, `db/migrate/` | Schema management |
| OTel | `otel-collector-config.yml`, `opentelemetry` in dependencies | Observability |
| Grafana | `grafana/`, `grafana.ini`, `provisioning/` | Dashboards |
| Prometheus | `prometheus.yml`, `prometheus/` | Metrics |

### Discovery Procedure

1. **Scan root directory** for all file patterns above.
2. **Identify primary language** by file count (most source files wins).
3. **Identify frameworks** by config file analysis.
4. **Identify CI/CD** by workflow file presence.
5. **Identify infrastructure** by Docker/ORM/observability configs.
6. **Select the matching checklist** from Section 4 based on detected technologies.
7. **If multiple languages detected** (monorepo, polyglot), run all applicable checklists.

---

## Technology-Specific Checklists

### .NET/C# Checklist

- [ ] **CAxxxx analyzer warnings**: Check for suppressed analyzer warnings (`#pragma warning disable`),
  justified or not. `TreatWarningsAsErrors` should be enabled. Common suppressions to audit:
  - CA1716: Reserved keywords used as property names (common in ViewModels with Event/Delegate properties)
  - CA1707: Underscore in member names (common in scaffold files and reporting models)
  - CA1031: Catch-all exception handlers (must have documented justification)
  - CA1822: Static method candidates (methods that don't use instance data)
  - CA1711: EventHandler suffix on non-event members
  - CA1848: LoggerMessage requirement for high-performance logging
  - CA1869: JsonSerializerOptions caching (should be cached and reused)
- [ ] **TreatWarningsAsErrors compliance**: Verify `TreatWarningsAsErrors` is enabled in
  `Directory.Build.props` or project files. All warnings should block the build.
- [ ] **File-scoped namespaces**: Project enforces
  `csharp_style_namespace_declarations = file_scoped:warning`. No block-scoped namespaces.
- [ ] **`var` preferred**: Verify `var` is used everywhere including built-in types, per
  `csharp_style_var_for_built_in_types = true:warning`.
- [ ] **Dead code**: Unused parameters, unreachable branches, unused private methods, commented-out
  code blocks. Use IDE analyzers (IDE0051, IDE0052, IDE0059).
- [ ] **God classes**: Classes exceeding 300 LOC. Flag classes with >10 public methods or >20 fields.
  Split into focused single-responsibility classes.
- [ ] **Naming conventions**: PascalCase for properties and methods, camelCase for locals and parameters,
  `_camelCase` for private fields. No `this.` qualification for fields/properties/methods.
- [ ] **Expression-bodied members**: Properties should use expression-bodied syntax when single-line.
  Methods only when single expression.
- [ ] **No `this.` qualification**: Fields, properties, and methods should not use `this.` prefix.
- [ ] **EnforceCodeStyleInBuild**: Verify code style enforcement is enabled in build configuration.
- [ ] **AnalysisLevel**: Should be `latest-recommended` for maximum analyzer coverage.

### Python Checklist

- [ ] **PEP 8 violations**: Line length >88 (black default), incorrect indentation, missing blank lines
  between top-level definitions. Use `flake8` or `ruff` for detection.
- [ ] **Type hint completeness**: All public functions have type annotations. `mypy --strict` passes.
  No `Any` without justification. Return types always specified.
- [ ] **Complexity (McCabe)**: Functions with McCabe complexity >10 flagged. Nested conditionals
  and loops counted. Use `mccabe` or `ruff` for detection.
- [ ] **Dead imports**: Unused imports detected by `autoflake` or `ruff`. Star imports
  (`from module import *`) flagged as high severity.
- [ ] **God modules**: Files exceeding 500 LOC. Modules with >15 top-level functions or >8 classes.
  Flag for splitting into focused modules.
- [ ] **Long functions**: Functions exceeding 50 lines. Flag for extraction into smaller functions.
- [ ] **Deep nesting**: Code nested >4 levels. Flag for early returns or extraction.
- [ ] **Magic numbers**: Unexplained literal values. Should be named constants or enums.
- [ ] **String formatting**: Use f-strings (Python 3.6+), not `.format()` or `%` formatting.

### JavaScript/TypeScript Checklist

- [ ] **ESLint violations**: ESLint configured and passing. No `eslint-disable` without justification.
  TypeScript ESLint parser for `.ts`/`.tsx` files.
- [ ] **TypeScript strict mode**: `"strict": true` in `tsconfig.json`. `strictNullChecks` enabled.
  No `any` type without `eslint-disable` justification.
- [ ] **Dead exports**: Exported functions/types never imported elsewhere. Use `ts-prune` or
  `eslint-plugin-unused-imports` for detection.
- [ ] **Complex functions**: Functions exceeding 10 branches or 50 lines. Flag for extraction.
- [ ] **Callback hell**: Deeply nested callbacks (>3 levels). Should use async/await or Promise chains.
- [ ] **Console.log in production**: `console.log` statements left in production code. Use proper
  logging library instead.
- [ ] **Naming conventions**: camelCase for variables/functions, PascalCase for classes/types/interfaces.
  UPPER_SNAKE_CASE for constants.
- [ ] **Optional chaining**: Use `?.` instead of manual null checks where applicable.
- [ ] **Nullish coalescing**: Use `??` instead of `||` for falsy-but-valid values (0, empty string).

### Go Checklist

- [ ] **Error handling**: No ignored errors (`_ = someFunc()`). Errors wrapped with context (`fmt.Errorf("doing X: %w", err)`). Custom error types for domain errors.
- [ ] **Naming conventions**: CamelCase exported, camelCase unexported. No snake_case or PascalCase for variables. Acronyms all-caps (`HTTP`, `URL`, `ID`).
- [ ] **Complexity**: Functions exceeding 10 branches or 50 lines. Deep nesting (>4 levels).
- [ ] **Dead code**: Unused functions, variables, imports flagged by `go vet`. Commented-out code blocks.
- [ ] **God packages**: Packages exceeding 500 LOC with >10 top-level functions or >5 types. Flag for splitting.
- [ ] **Context propagation**: All I/O functions accept `context.Context` as first parameter. No context stored in structs.
- [ ] **Struct initialization**: No zero-value initialization where zero values are invalid. Constructor functions for complex types.
- [ ] **Goroutine management**: No unbounded goroutine spawns. Missing `sync.WaitGroup` or errgroup for coordinating goroutines.

### Java Checklist

- [ ] **Naming conventions**: PascalCase for classes, camelCase for methods/variables, UPPER_SNAKE_CASE for constants. No Hungarian notation.
- [ ] **Complexity**: Methods exceeding 10 branches or 50 lines. Nested conditionals >4 levels.
- [ ] **Dead code**: Unused imports, unused private methods, unreachable branches. Use IDE inspections (IntelliJ, Eclipse).
- [ ] **God classes**: Classes exceeding 300 LOC with >15 methods or >10 fields.
- [ ] **Null safety**: No `null` returns from public methods. `Optional<T>` for nullable returns. `@NonNull` / `@Nullable` annotations.
- [ ] **Exception handling**: No bare `catch (Exception)` or `catch (Throwable)`. Specific exception types. Try-with-resources for all `AutoCloseable` types.
- [ ] **Resource management**: No unclosed streams or connections. Try-with-resources for file/network I/O.
- [ ] **Concurrency**: `java.util.concurrent` preferred over `synchronized`. Thread-safe collections. No shared mutable static state.
- [ ] **Logging**: SLF4J with Logback or Log4j2. No `System.out.println` in production. Structured logging.

### Rust Checklist

- [ ] **Unsafe usage**: `unsafe` blocks documented with safety invariants. Minimized unsafe surface area. No unnecessary unsafe.
- [ ] **Error handling**: `Result<T, E>` for fallible operations. No `unwrap()` or `expect()` in library code. `thiserror` / `anyhow` for error types.
- [ ] **Naming conventions**: snake_case for functions/variables, PascalCase for types/traits, SCREAMING_SNAKE_CASE for constants.
- [ ] **Complexity**: Functions exceeding 10 branches or 50 lines. Nested pattern matches >3 levels.
- [ ] **Dead code**: `#[allow(dead_code)]` on items never used. Unused imports flagged by Clippy.
- [ ] **God modules**: Files exceeding 400 LOC with >10 public functions or >5 types. Split by domain.
- [ ] **Clippy lints**: `cargo clippy` passes. Pedantic lints enabled where practical.
- [ ] **Concurrency**: `Send` and `Sync` bounds understood. No data races. `Arc<Mutex<T>>` / channels for shared state.

---

## Anti-Patterns

### Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | God class/module | high | Single class/module with >10 responsibilities or >300 LOC | Violates SRP, hard to test, causes merge conflicts, resists change | Split into focused classes/modules with single responsibility |
| 2 | Copy-paste code | medium | Duplicated logic across files (>80% similar blocks) | Changes require multiple edits, inconsistencies emerge | Extract shared functions, use composition or inheritance |
| 3 | Magic numbers/strings | medium | Unexplained literal values in code | Unclear intent, fragile to changes, scattered updates | Named constants or enums |
| 4 | Deep nesting | medium | Code nested >4 levels | Hard to read, hard to test, high cyclomatic complexity | Early returns, guard clauses, extract methods |
| 5 | Long parameter lists | low | Functions with >5 parameters | Hard to call correctly, hard to extend, error-prone | Options object, builder pattern, parameter object |

### .NET/C# Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | God handler combining CRUD/queries/access checks/encryption | high | Single handler class managing create, read, update, delete, authorization, and encryption | Violates SRP, untestable in isolation, changes cascade | Split into focused handlers: one command per handler |
| 2 | Stub providers returning empty results silently | high | Mock/stub implementations returning empty collections without logging | Tests pass but don't validate real behavior, bugs hide | Return meaningful test data or throw `NotImplementedException` |
| 3 | Validator returning true for unknown countries | high | Country validation logic treats unrecognized countries as valid | Bypasses validation entirely, allows invalid data | Return false for unknown countries, maintain whitelist |
| 4 | Unjustified CA1031 suppression | medium | `#pragma warning disable CA1031` without documented reason | Catches and swallows all exceptions silently, hides bugs | Catch specific exceptions; document reason for catch-all |
| 5 | Block-scoped namespaces in file-scoped project | low | Using `namespace Foo { ... }` when project enforces file-scoped | Inconsistent style, may cause build warnings in strict mode | Use `namespace Foo;` file-scoped declaration |

### Python Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | God module | high | Single `.py` file with >500 LOC, >15 functions | Hard to navigate, test, and maintain | Split into focused modules by responsibility |
| 2 | Star imports | medium | `from module import *` | Pollutes namespace, unclear what's used, breaks static analysis | Import only what's needed: `from module import func` |
| 3 | Bare except clauses | high | `except:` without specific exception type | Catches KeyboardInterrupt, SystemExit, hides all bugs | Catch specific exception types |
| 4 | Mutable default arguments | high | `def foo(items=[])` | List persists across calls, shared mutable state | Use `None` default, create new list inside function |
| 5 | Missing type hints on public API | medium | Public functions without parameter/return annotations | Unclear contracts, breaks IDE support, mypy can't check | Add type hints to all public functions |

### JavaScript/TypeScript Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | `any` type in library code | high | Using `any` instead of specific types | Defeats type checking, runtime errors in consumers | Use specific types, `unknown`, or generics |
| 2 | Callback hell | medium | Deeply nested callbacks (>3 levels) | Unreadable, unmaintainable, error-prone | Use async/await or flat Promise chains |
| 3 | Console.log in production | low | `console.log` statements in production code | Performance overhead, exposes internal data | Use structured logging library |
| 4 | Unhandled promise rejections | high | Missing `.catch()` or try/catch around async | Crashes, unhandled rejection warnings | Always handle errors in async code |
| 5 | Default exports | low | `export default function()` | Hard to refactor, poor IDE support, tree-shaking issues | Use named exports: `export function name()` |

### Go Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Ignored errors | high | `_ = someFunc()` or `someFunc()` without checking error return | Silent failures, production data loss | Always check errors with `if err != nil` |
| 2 | Missing context propagation | medium | I/O functions not accepting `context.Context` | No timeout/cancellation, goroutine leaks | Pass context as first parameter |
| 3 | God package | high | Single `.go` file or package >500 LOC, >10 types | Hard to navigate, test, and maintain | Split into focused packages by responsibility |
| 4 | Zero-value struct without validation | medium | Using zero-value structs that represent invalid state | Production bugs from uninitialized fields | Constructor functions with validation |

### Java Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Catching Exception and ignoring | high | `catch (Exception e) {}` with empty block | Silently swallows all errors, impossible to diagnose | Catch specific types, log and rethrow |
| 2 | Null returns from public methods | medium | Returning `null` instead of `Optional<T>` | NullPointerException risk in callers | Return `Optional<T>` or empty collection |
| 3 | God class | high | Single Java class >300 LOC, >15 methods | Violates SRP, merge conflicts, untestable | Split into focused classes by responsibility |
| 4 | System.out.println in production | low | Output to stdout instead of proper logging | No log levels, no formatting, performance overhead | Use SLF4J logger throughout

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

All audit findings MUST be emitted as structured JSON objects. One finding per object.

### Finding Schema

```json
{
  "severity": "critical|high|medium|low",
  "file": "relative/path/to/file",
  "line": 42,
  "endLine": 45,
  "description": "What was found and why it matters. Include the problematic code pattern and metric values.",
  "recommendation": "Specific fix with brief code example. Reference pattern or naming convention.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|CrossLanguage",
  "category": "maintainability|architecture|performance|security|compliance|testing|observability"
}
```

### Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Severity Guidelines below.
- **file**: Relative path from project root. Use forward slashes. Never absolute paths.
- **line**: 1-based line number where the finding starts.
- **endLine**: Optional. Line where the finding ends (for multi-line issues).
- **description**: Factual, specific. Include the problematic code fragment and metric value.
- **recommendation**: Actionable. Include a code snippet showing the fix when possible.
- **technology**: Primary technology of the finding. Use `CrossLanguage` only for patterns spanning multiple files in different languages.
- **category**: Primary category — for code quality, typically `maintainability`, `architecture`, or `performance`.

### Summary Report

After all findings, emit a summary:

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
| **critical** | Immediate data integrity or system reliability risk | Validator bypass that allows all inputs, stub returning empty silently in critical path |
| **high** | Significant maintainability or correctness risk | God class >300 LOC, cyclomatic complexity >20, unjustified catch-all suppression |
| **medium** | Degraded code quality or readability | Dead code, complexity 10-20, missing type hints, PEP 8 violations, deep nesting |
| **low** | Best practice deviation, style improvement | Naming convention violations, missing docstrings, minor formatting |

### Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding is in authentication/authorization code | +1 level | All technologies |
| Finding is in data validation/encryption code | +1 level | All technologies |
| Finding affects multiple tenants or users | +1 level | Multi-tenant systems |
| Finding is in a public-facing endpoint | +1 level | Web applications |
| Finding involves financial/billing data | +1 level | Financial systems |
| God class is >500 LOC (not just >300) | +1 level | All technologies |

### Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding is in test/demo code only | -1 level (min low) | All technologies |
| Finding has existing TODO/FIXME comment acknowledging it | -1 level (min low) | All technologies |
| Finding is in generated/scaffolded code | -1 level (min low) | All technologies |

### Technology-Specific Severity Adjustments

#### .NET/C#
- Unjustified CA1031 suppression in production handler → high (hides exceptions)
- File-scoped namespace violation in strict project → low (style)
- God handler combining CRUD/auth/encryption → high (SRP violation)
- Missing `var` preference in strict project → low (style)

#### Python
- Missing type hints on public API → medium (maintainability)
- Star import in production code → medium (namespace pollution)
- Function complexity >20 → high (maintainability)
- f-string not used (legacy `.format()`) → low (modernization)

#### JavaScript/TypeScript
- `any` type in library code → medium (type safety)
- Unhandled promise rejection → high (reliability)
- Console.log in production → low (style)
- Dead export → low (cleanup)

#### Go
- Ignored errors in I/O paths → high (silent failure)
- Missing context propagation in network code → medium (timeout/cancellation)
- God package >500 LOC → high (maintainability)
- Zero-value struct without validation → medium (bug risk)

#### Java
- Null return from public method → medium (NPE risk)
- Catching Exception and ignoring → high (silent failure)
- God class >300 LOC → high (maintainability)
- System.out.println in production → low (style)

---

## Integration Notes

### Squad Dependency Graph

```
audit-dependencies ──────────────────┐
audit-security ──────────────────────┤
audit-architecture ──────────────────┤
audit-code-quality ◄─────────────────┤  (may reference architecture findings)
audit-test-coverage ─────────────────┤
audit-refactoring ◄──────────────────┘  (consumes code-quality findings)
audit-consolidated ◄──────────────────── (consumes all)
audit-consistency-coherence            (consumes all)
```

### Upstream Dependencies

- **audit-architecture**: Code quality may reference architecture findings for layer isolation issues
  (e.g., god classes that violate clean architecture boundaries).
- **audit-dependencies**: Code quality may reference dependency data for deprecated package patterns.

### Downstream Consumers

- **audit-refactoring**: Consumes code-quality findings to recommend modernization approaches and
  refactoring priorities.
- **audit-consolidated**: Aggregates all squad findings into unified report.
- **audit-consistency-coherence**: Validates cross-domain alignment between squad findings.

### Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-code-quality | audit-architecture | Code-quality focuses on implementation smells; architecture focuses on layer/module structure |
| audit-code-quality | audit-refactoring | Code-quality identifies problems; refactoring recommends modernization approaches |
| audit-code-quality | audit-security | Code-quality focuses on maintainability; security focuses on vulnerability exploitation |

### Recommended Execution Order

For a full audit, this squad should run after:

1. **audit-dependencies** — Establish baseline of third-party risk
2. **audit-architecture** — Structural analysis
3. **audit-code-quality** ← This squad

Then downstream:
4. **audit-refactoring** — Uses code-quality findings for modernization recommendations
5. **audit-test-coverage** — Tests quality of code identified by this squad

### Output Integration

When squads run sequentially:
1. Read the previous squad's findings from the shared output location.
2. Deduplicate: skip findings already reported by a previous squad.
3. Cross-reference: escalate findings confirmed by multiple squads.
4. Output in the standardized Finding Schema format above for aggregation.
