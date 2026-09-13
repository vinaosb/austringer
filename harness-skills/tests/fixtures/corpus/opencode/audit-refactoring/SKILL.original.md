---
name: audit-refactoring
description: >-
  Refactoring and modernization auditing: detects obsolete patterns, identifies
  library upgrade opportunities, evaluates syntax modernization candidates,
  surfaces duplicate code, and recommends specific rewrites across multi-language
  codebases. Produces prioritized findings with severity ratings and concrete
  before/after code examples.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java"]
version: "2.0.0"
---

# Refactoring & Modernization Audit

## USE FOR

- C# modernization detection — file-scoped namespaces, global usings, record types, pattern matching, switch expressions
- Obsolete API usage identification — deprecated library calls, outdated patterns, legacy constructs
- Async/await anti-pattern detection — `async void`, `.Result`, `.Wait()`, sync-over-async
- LINQ misuse — inefficient queries, multiple enumeration, premature materialization
- Duplicate code detection — copy-paste programming, similar code blocks across files
- DTO mapping bug detection — same value mapped to two DTO fields, field order swap errors
- Python modernization — f-strings over `.format()`, `pathlib` over `os.path`, type unions, `match/case`
- JavaScript modernization — `var` to `let`/`const`, callback to async/await, CommonJS to ESM
- Read-only query optimization — tracking vs `AsNoTracking()` for queries that never modify entities
- Technical debt inventory — score and prioritize refactoring opportunities

## DO NOT USE FOR

- Code style and naming conventions (use `audit-code-quality`)
- Architecture and layer isolation review (use `audit-architecture`)
- Security vulnerability scanning (use `audit-security`)
- Test coverage analysis (use `audit-test-coverage`)
- Dependency CVE and version auditing (use `audit-dependencies`)
- Writing code or making changes (this is audit-only, not modification)

---

## Target Discovery

The agent MUST auto-detect the project structure. Do NOT hardcode repository names.

### Language Detection

| Signal | File Pattern | Detected Language |
|--------|-------------|-------------------|
| C# source files | `*.cs`, `*.csproj`, `*.sln`, `*.slnx` | .NET/C# |
| Python source files | `*.py`, `*.pyi`, `pyproject.toml` | Python |
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

- [ ] **C# modernization**: Detect legacy patterns that have modern equivalents:
  - Block-scoped → file-scoped namespaces
  - Missing global usings (check for `GlobalUsings.cs` or `ImplicitUsings` enablement)
  - Classes that should be `record` types (immutable data carriers with value equality)
  - `switch` statements that should be switch expressions
  - `if`/`else` chains that should use pattern matching
  - Missing expression-bodied members for single-line methods/properties
- [ ] **Obsolete API usage**: Detect deprecated framework calls:
  - `WebHostBuilder` → `HostBuilder` (.NET 6+)
  - `IWebHostBuilder` → `IHostBuilder`
  - `AddMvc()` → `AddControllers()` / `AddRazorPages()` (when not using both)
  - `AddEntityFramework*` → `AddDbContext` / `AddDbContextFactory`
  - Legacy `Polly.Extensions.Http` → `Microsoft.Extensions.Http.Resilience`
  - `xUnit` v2 patterns → v3 patterns
- [ ] **Async/await anti-patterns**: Critical refactoring targets:
  - `async void` methods (except UI event handlers) → `async Task`
  - `.Result` or `.Wait()` on Task → `await`
  - `Task.Run` for CPU-bound work in ASP.NET Core → remove (server already handles threading)
  - Missing `ConfigureAwait(false)` in library code (optional but recommended for libraries)
  - `async` methods that don't use `await` → remove `async` keyword, return `Task.CompletedTask`
- [ ] **LINQ misuse**:
  - Multiple enumeration of `IEnumerable` (call `.ToList()` once)
  - `.Where().Where()` chains that could be combined
  - `.Select(x => x).Where(...)` no-op selects
  - `.Count()` when `.Any()` suffices
  - Missing `.AsNoTracking()` on read-only EF Core queries
- [ ] **Duplicate code detection**: Similar blocks (>80% match) across files:
  - Copy-paste handlers with minor variations
  - Repeated validation logic → extract to shared validator
  - Repeated mapping logic → use Mapster/automapper profiles
- [ ] **Copy/paste mapping bugs**: Same source value mapped to two DTO fields:
  ```csharp
  // Bug: FirstName mapped to both fields after copy-paste
  dto.FirstName = entity.FirstName;
  dto.LastName = entity.FirstName;  // ← should be entity.LastName
  ```
- [ ] **Missing expression-bodied members**: Properties and methods that could be single-line
  expression-bodied but use full block syntax.
- [ ] **Record type candidates**: Classes that are immutable data carriers with value equality
  requirements but implemented as regular classes.

### Python Checklist

- [ ] **Python 3.10+ patterns**: Detect legacy patterns with modern replacements:
  - `Union[X, Y]` → `X | Y` type union syntax
  - `Optional[X]` → `X | None`
  - `if/elif` chains → `match/case` (structural pattern matching)
  - `typing.Dict` → `dict` (built-in generics since Python 3.9)
  - `typing.List` → `list`
  - `typing.Tuple` → `tuple`
- [ ] **Legacy format strings**: Detect and flag:
  - `"Hello %s" % name` → `f"Hello {name}"`
  - `"Hello {}".format(name)` → `f"Hello {name}"`
  - String concatenation for building messages → f-strings
- [ ] **Outdated library usage**:
  - `os.path` → `pathlib.Path`
  - `collections.namedtuple` → `dataclasses.dataclass`
  - `typing.TypedDict` (pre-3.11) → direct class syntax
  - `requests` in async code → `httpx`
- [ ] **Duplicate code**: Similar function bodies, repeated validation patterns, copy-pasted
  data transformation logic.
- [ ] **Legacy class patterns**: Old-style classes, `object` base class in Python 3,
  `__init__` with long parameter lists → dataclasses.

### JavaScript/TypeScript Checklist

- [ ] **ES2022+ patterns**: Detect legacy patterns with modern replacements:
  - `var` → `let` / `const`
  - Callback functions → arrow functions (where `this` binding not needed)
  - `.then()` chains → `async/await`
  - String concatenation → template literals
  - `arguments` object → rest parameters (`...args`)
  - `for` loops → `.map()`, `.filter()`, `.reduce()` where appropriate
- [ ] **Legacy callback patterns**: Functions using `(err, result)` callback style instead of
  Promises or async/await.
- [ ] **CommonJS vs ESM**:
  - `require()` / `module.exports` → `import` / `export`
  - `__dirname` / `__filename` → `import.meta.url`
  - Detect mixed module systems in same project
- [ ] **Outdated React patterns** (if React detected):
  - Class components → functional components with hooks
  - `componentDidMount` → `useEffect`
  - `this.setState` → `useState`
  - String refs → `useRef` / callback refs
  - Higher-order components → hooks where appropriate
- [ ] **Duplicate code**: Similar component logic, repeated API call patterns,
  copy-pasted utility functions.
- [ ] **TypeScript modernization**: Type assertions → type guards, `any` → discriminated unions,
  missing generic constraints.

### Go Checklist

- [ ] **Go 1.21+ modernization**: Detect legacy patterns with modern equivalents:
  - `ioutil.ReadAll` / `ioutil.ReadFile` → `os.ReadFile` (Go 1.16+)
  - `ioutil.WriteFile` → `os.WriteFile`
  - `ioutil.TempDir` → `os.MkdirTemp`
  - `ioutil.NopCloser` → `io.NopCloser`
  - `ioutil.Discard` → `io.Discard`
- [ ] **Error wrapping**: Detect legacy error handling:
  - `fmt.Errorf("error: %s", err)` → `fmt.Errorf("error: %w", err)` (with `%w` for `errors.Is`/`errors.As`)
  - Missing error wrapping in middleware/layers
  - `log.Fatal` in library code
- [ ] **Context propagation**: Legacy patterns:
  - `context.WithTimeout` results not deferred (context leak)
  - Background context used where request context should be passed
  - Missing context cancellation checks with `ctx.Done()`
- [ ] **Synchronization modernization**:
  - `sync/atomic` value operations → `sync/atomic` typed APIs (Go 1.19+)
  - Raw `sync.Mutex` → `sync.RWMutex` for read-heavy patterns
  - Missing `errgroup` for goroutine coordination
- [ ] **Duplicate code**: Similar handler bodies, repeated validation logic, copy-pasted utility
  functions across packages.
- [ ] **Old-style tests**: Table-driven tests not used where applicable. Missing `t.Parallel()`.
  Subtests not used for grouped assertions.

### Java Checklist

- [ ] **Java 17+ modernization**: Detect legacy patterns with modern equivalents:
  - `if/else` null checks → `Optional` / `Optional.orElseThrow`
  - Anonymous inner classes → lambda expressions / method references
  - `for (int i = 0; i < list.size(); i++)` → enhanced for-each / streams
  - Legacy `Date` / `Calendar` → `java.time.LocalDate` / `Instant`
  - `StringBuffer` → `StringBuilder`
  - `new HashMap<>()`, `new ArrayList<>()` → `Map.of()`, `List.of()` for small immutable collections
- [ ] **Record types**: Detect classes that should be `record` types:
  - Immutable data carriers with `equals`/`hashCode`/`toString` boilerplate
  - DTO classes with only constructor + getters
- [ ] **Pattern matching**: Detect legacy patterns:
  - `instanceof` check + cast → pattern matching for `instanceof` (Java 16+)
  - `switch` statements → enhanced `switch` expressions (Java 14+)
  - Missing sealed classes/interface hierarchies
- [ ] **Stream API usage**: Detect legacy patterns:
  - `for` loops with filtering/mapping → stream pipelines
  - Manual null checks after stream operations → `Optional` chaining
  - Blocking calls in stream operations
- [ ] **Obsolete APIs**: Detect deprecated usage:
  - `javax.*` → `jakarta.*` (Jakarta EE migration)
  - `Spring` XML config → Java config / annotation-based
  - `JUnit 4` → `JUnit 5` (Jupiter)
  - `MockitoJUnitRunner` → `MockitoExtension`
- [ ] **Duplicate code**: Similar controller/service methods, repeated validation logic,
  copy-pasted JPA repository queries.
- [ ] **Try-with-resources**: Manual `try { } finally { close() }` → try-with-resources for all
  `AutoCloseable` resources (since Java 7).

### Rust Checklist

- [ ] **Rust edition modernization**: Detect legacy patterns:
  - `extern crate` → edition 2018+ `use` imports
  - `dyn Trait` missing for trait objects (added in 2018, required in 2021+)
  - `try!()` → `?` operator
  - `#[macro_use]` → `use` macro imports
- [ ] **Error handling modernization**:
  - Manual `Box<dyn Error>` → `anyhow::Result` for application code
  - Manual error types → `thiserror` derive macros
  - `.unwrap()` / `.expect()` in library code → propagate with `?`
- [ ] **Concurrency modernization**:
  - `Arc<Mutex<T>>` for read-heavy data → `arc_swap` / `RwLock`
  - `thread::spawn` → `tokio::spawn` in async projects
  - Missing `rayon` for CPU-bound parallel work
- [ ] **Clippy modernization opportunities**:
  - Run `cargo clippy` with pedantic mode for modernization suggestions
  - Check for deprecated method usage
  - Check for replacement patterns (e.g., `.map().flatten()` → `.flat_map()`)
- [ ] **Duplicate code**: Similar function bodies across modules, copy-pasted error handling patterns,
  repeated serialization/deserialization logic.

---

## Anti-Patterns

### Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Sync-over-async | high | Blocking on async code (`.Result`, `.Wait()`, `await` inside sync) | Deadlocks in ASP.NET, thread pool starvation | Propagate `async` through entire call chain |
| 2 | Copy-paste with bugs | high | Duplicated code with subtle field-order bugs | One copy gets fixed, other doesn't; silent data corruption | Extract shared function, use mapping library |
| 3 | Legacy patterns accumulating | medium | New code uses old patterns because surrounding code does | Technical debt grows, onboarding harder | Modernize incrementally, enforce new patterns in new code |
| 4 | Premature abstraction | low | Extracting shared function for code used in only one place | Adds indirection without reducing duplication | Wait for third occurrence before abstracting (Rule of Three) |

### .NET/C# Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Read-only queries with tracking | high | EF Core queries that never modify entities but don't use `AsNoTracking()` | Unnecessary memory allocation, slower performance, change tracker overhead | Add `.AsNoTracking()` to all read-only queries |
| 2 | DTO projection bugs | high | Same source value mapped to two DTO fields after copy-paste | Silent data corruption — wrong data in production | Review all mapping assignments; use Mapster/automapper for consistency |
| 3 | Mapster mapping duplicates | medium | Same mapping logic repeated in multiple `TypeAdapterConfig` registrations | Inconsistencies emerge, maintenance burden | Centralize mapping configs in one registration location |
| 4 | Block-scoped namespaces in file-scoped project | low | Using `namespace Foo { }` when project enforces file-scoped | Style inconsistency, may cause build warnings | Use `namespace Foo;` file-scoped declaration |
| 5 | Classes that should be records | medium | Immutable data carriers implemented as classes with manual equality | Boilerplate equality/hashcode, error-prone | Use `record` or `record struct` for value semantics |

### Python Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Legacy format strings | medium | Using `%` formatting or `.format()` instead of f-strings | Less readable, harder to maintain, no expression embedding | Use f-strings: `f"Hello {name}"` |
| 2 | `os.path` over `pathlib` | low | String-based path manipulation instead of object-oriented | Less readable, platform-dependent separators, error-prone | Use `pathlib.Path` for all path operations |
| 3 | `requests` in async code | high | Synchronous HTTP client blocking the event loop | Blocks entire async runtime, defeats purpose of async | Use `httpx.AsyncClient` for async HTTP |
| 4 | Old-style Union types | low | `Union[str, int]` instead of `str \| int` | Verbose, less readable in Python 3.10+ | Use `str \| int` type union syntax |

### JavaScript/TypeScript Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | `var` declarations | medium | Using `var` instead of `let`/`const` | Function-scoped, hoisted, allows redeclaration | Use `const` by default, `let` when reassignment needed |
| 2 | Callback-style async | medium | `(err, result) => {}` instead of Promises/async | Pyramid of doom, error handling is fragile | Use async/await or Promise chains |
| 3 | CommonJS in ESM project | medium | `require()` / `module.exports` in project using `"type": "module"` | Inconsistent module system, bundler confusion | Use `import` / `export` syntax throughout |
| 4 | Class components in React | low | Class components when hooks are available | More boilerplate, harder to share logic | Use functional components with hooks |

### Go Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | `ioutil` package usage | low | Using deprecated `ioutil` functions instead of `os` / `io` package equivalents | Deprecated since Go 1.16, may be removed in future Go versions | Use `os.ReadFile`, `os.WriteFile`, `io.NopCloser` etc. |
| 2 | Error not wrapped | medium | `fmt.Errorf("error: %s", err)` without `%w` | Prevents `errors.Is` / `errors.As` from unwrapping | Use `%w` verb: `fmt.Errorf("context: %w", err)` |
| 3 | Raw mutex for read-heavy | low | `sync.Mutex` when most operations are reads | Unnecessary contention, slower read paths | Use `sync.RWMutex` for read-heavy workloads |
| 4 | No table-driven tests | medium | Separate test functions for each input case | Duplication, harder to add cases, less readable | Use table-driven tests with subtests |

### Java Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | `java.util.Date` / `Calendar` | low | Using legacy date/time APIs instead of `java.time` | Mutable, thread-unsafe, poor API design | Use `java.time.LocalDate`, `Instant`, `ZonedDateTime` |
| 2 | Manual resource management | medium | `try { ... } finally { close() }` instead of try-with-resources | Verbose, error-prone (forgotten close) | Use try-with-resources for all `AutoCloseable` types |
| 3 | `JUnit 4` in modern project | medium | Using `@Test` from `org.junit.Test` (JUnit 4) instead of JUnit 5 Jupiter | Missing JUnit 5 features (param tests, extensions) | Migrate to `org.junit.jupiter.api.Test` and JUnit 5 API |
| 4 | Legacy `for` loop with streams candidate | low | Manual iteration with filtering/mapping when streams would be clearer | Verbose, harder to parallelize, more error-prone | Use Stream API for filtering, mapping, and reduction

---

## Code Examples (Bad/Good Pairs)

### .NET/C# Example 1: AsNoTracking for Read Queries

```csharp
// ❌ BAD: Change tracking enabled for read-only query — wastes memory and CPU
public async Task<List<PersonDto>> GetAllPeopleAsync()
{
    var people = await _dbContext.People
        .Include(p => p.Addresses)
        .Include(p => p.Contacts)
        .Where(p => p.IsActive)
        .ToListAsync();

    return people.Select(p => new PersonDto
    {
        Id = p.Id,
        FullName = p.FullName,
        Email = p.Contacts.FirstOrDefault()?.Value
    }).ToList();
}

// ✅ GOOD: AsNoTracking for read-only queries — no change tracker overhead
public async Task<List<PersonDto>> GetAllPeopleAsync()
{
    return await _dbContext.People
        .AsNoTracking()
        .Include(p => p.Addresses)
        .Include(p => p.Contacts)
        .Where(p => p.IsActive)
        .Select(p => new PersonDto
        {
            Id = p.Id,
            FullName = p.FullName,
            Email = p.Contacts.FirstOrDefault()!.Value
        })
        .ToListAsync();
}
// Also: projection inside IQueryable avoids loading unnecessary data
```

### .NET/C# Example 2: DTO Mapping Bug from Copy-Paste

```csharp
// ❌ BAD: Copy-paste bug — same value mapped to two fields
public static PersonDto ToDto(Person entity)
{
    return new PersonDto
    {
        FirstName = entity.FirstName,
        LastName = entity.FirstName,    // ← BUG: should be entity.LastName
        Email = entity.Email,
        Phone = entity.Phone,
        City = entity.Address?.City ?? string.Empty,
        State = entity.Address?.City ?? string.Empty,  // ← BUG: should be .State
    };
}

// ✅ GOOD: Each field mapped to correct source, use Mapster for consistency
public static PersonDto ToDto(Person entity)
{
    return entity.Adapt<PersonDto>();  // Mapster handles mapping by convention
}

// Or explicit mapping with correct field references:
public static PersonDto ToDto(Person entity)
{
    return new PersonDto
    {
        FirstName = entity.FirstName,
        LastName = entity.LastName,
        Email = entity.Email,
        Phone = entity.Phone,
        City = entity.Address?.City ?? string.Empty,
        State = entity.Address?.State ?? string.Empty,
    };
}
```

### .NET/C# Example 3: Record Types for Immutable Data

```csharp
// ❌ BAD: Regular class for immutable data — manual equality, verbose
public class MoneyAmount
{
    public decimal Value { get; }
    public string Currency { get; }

    public MoneyAmount(decimal value, string currency)
    {
        Value = value;
        Currency = currency;
    }

    public override bool Equals(object? obj) =>
        obj is MoneyAmount other && Value == other.Value && Currency == other.Currency;

    public override int GetHashCode() => HashCode.Combine(Value, Currency);

    public override string ToString() => $"{Value} {Currency}";
}

// ✅ GOOD: Record type — value equality, immutability, conciseness
public record MoneyAmount(decimal Value, string Currency);
// Gets Equals, GetHashCode, ToString, ==, !=, with-expression support for free
```

### Python Example 1: Legacy to Modern Python

```python
# ❌ BAD: Legacy patterns — % formatting, os.path, old Union syntax
import os
from typing import Union, Optional

def get_config_path(env: Union[str, None] = None) -> Optional[str]:
    if env is None:
        env = "default"
    base = os.path.dirname(os.path.abspath(__file__))
    config_file = os.path.join(base, "configs", "%s.yaml" % env)
    if os.path.exists(config_file):
        return config_file
    return None

# ✅ GOOD: Modern Python 3.10+ — f-strings, pathlib, type union syntax
from pathlib import Path

def get_config_path(env: str | None = None) -> str | None:
    env = env or "default"
    config_file = Path(__file__).parent / "configs" / f"{env}.yaml"
    return str(config_file) if config_file.exists() else None
```

### JavaScript/TypeScript Example 1: Callback to Async/Await

```javascript
// ❌ BAD: Callback pyramid of Doom
function fetchUserData(userId, callback) {
    db.query("SELECT * FROM users WHERE id = ?", [userId], function(err, rows) {
        if (err) return callback(err);
        var user = rows[0];
        db.query("SELECT * FROM orders WHERE user_id = ?", [userId], function(err2, orders) {
            if (err2) return callback(err2);
            callback(null, { user: user, orders: orders });
        });
    });
}

// ✅ GOOD: Async/await — flat, readable, proper error handling
async function fetchUserData(userId) {
    const user = await db.query("SELECT * FROM users WHERE id = ?", [userId]);
    const orders = await db.query("SELECT * FROM orders WHERE user_id = ?", [userId]);
    return { user: user[0], orders };
}
```

### Go Example 1: Deprecated ioutil → os Package

```go
// ❌ BAD: Deprecated ioutil functions — may be removed in future Go versions
import "io/ioutil"

func loadConfig(path string) ([]byte, error) {
    return ioutil.ReadFile(path)
}

func saveConfig(path string, data []byte) error {
    return ioutil.WriteFile(path, data, 0644)
}

// ✅ GOOD: Modern os package equivalents
import "os"

func loadConfig(path string) ([]byte, error) {
    return os.ReadFile(path)
}

func saveConfig(path string, data []byte) error {
    return os.WriteFile(path, data, 0644)
}
```

### Java Example 1: Legacy Date → java.time

```java
// ❌ BAD: Legacy Date/Calendar — mutable, confusing API, thread-unsafe
public Date calculateExpiry(Date createdDate, int daysValid) {
    Calendar cal = Calendar.getInstance();
    cal.setTime(createdDate);
    cal.add(Calendar.DAY_OF_MONTH, daysValid);
    return cal.getTime();
}

// ✅ GOOD: java.time — immutable, clear API, thread-safe
public Instant calculateExpiry(Instant createdDate, int daysValid) {
    return createdDate.plus(daysValid, ChronoUnit.DAYS);
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
  "description": "What was found and why it matters. Include the legacy code pattern and its modern equivalent.",
  "recommendation": "Specific refactoring with before/after code example. Reference modern API or pattern.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|CrossLanguage",
  "category": "maintainability|architecture|performance|security|compliance|testing|observability"
}
```

### Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Severity Guidelines below.
- **file**: Relative path from project root. Use forward slashes. Never absolute paths.
- **line**: 1-based line number where the finding starts.
- **endLine**: Optional. Line where the finding ends (for multi-line issues).
- **description**: Factual, specific. Include the legacy pattern and its modern replacement.
- **recommendation**: Actionable. Include a before/after code snippet showing the refactoring.
- **technology**: Primary technology of the finding.
- **category**: Primary category — for refactoring, typically `maintainability` or `performance`.

### Summary Report

After all findings, emit a summary:

```json
{
  "summary": {
    "squad": "audit-refactoring",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#", "Python"],
    "totalFindings": 20,
    "bySeverity": {
      "critical": 0,
      "high": 4,
      "medium": 10,
      "low": 6
    },
    "byCategory": {
      "maintainability": 12,
      "performance": 5,
      "architecture": 2,
      "security": 1
    },
    "filesScanned": 38,
    "filesWithFindings": 15
  }
}
```

---

## Severity Guidelines

### Base Severity Definitions

| Level | Definition | Refactoring Example |
|-------|-----------|-------------------|
| **critical** | Immediate data integrity or correctness risk from code pattern | DTO mapping bug copying wrong field, sync-over-async causing production deadlocks |
| **high** | Significant performance or maintainability risk | Missing `AsNoTracking()` on hot queries, `async void` outside event handlers, duplicate code with subtle bugs |
| **medium** | Degraded maintainability, outdated patterns, moderate duplication | Legacy format strings, block-scoped namespaces, missing record type opportunity, callback patterns |
| **low** | Style modernization, minor improvement opportunities | `var` to `let`/`const`, `os.path` to `pathlib`, missing expression-bodied member |

### Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding involves data mapping/projection bug | +1 level | All technologies |
| Finding causes production deadlocks or crashes | +1 level | .NET async patterns |
| Finding is in hot path (frequently called) | +1 level | Performance-related findings |
| Finding affects multiple tenants or users | +1 level | Multi-tenant systems |
| Finding involves financial/billing data calculation | +1 level | Financial systems |

### Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding is in test/demo code only | -1 level (min low) | All technologies |
| Finding has existing TODO/FIXME acknowledging it | -1 level (min low) | All technologies |
| Finding is in deprecated code scheduled for removal | -1 level (min low) | All technologies |
| Modern equivalent only available in preview/unstable version | -1 level (min low) | All technologies |

### Technology-Specific Severity Adjustments

#### .NET/C#
- Sync-over-async in ASP.NET Core → high (deadlock risk)
- Missing `AsNoTracking()` on read-only query → medium (performance)
- DTO projection with copy-paste bug → critical (data integrity)
- Block-scoped namespace in file-scoped project → low (style)

#### Python
- `requests` in async function → high (blocks event loop)
- Legacy format strings → low (readability)
- `os.path` instead of `pathlib` → low (modernization)
- Old Union syntax in Python 3.10+ → low (style)

#### JavaScript/TypeScript
- `var` declarations → low (style)
- Callback-style async → medium (readability)
- CommonJS in ESM project → medium (consistency)
- Class components in React → low (modernization)

#### Go
- `ioutil` package usage → low (deprecated API)
- Error not wrapped with `%w` → medium (error inspection)
- Raw `sync.Mutex` for read-heavy → low (optimization)
- No table-driven tests → medium (maintainability)

#### Java
- Legacy `Date` / `Calendar` → low (modernization)
- Manual resource management → medium (resource leak risk)
- JUnit 4 in modern project → medium (missing features)
- Legacy `for` loop with streams candidate → low (style)

---

## Integration Notes

### Squad Dependency Graph

```
audit-code-quality ──────────────────┐
audit-architecture ──────────────────┤
audit-refactoring ◄──────────────────┤  (consumes code-quality and architecture findings)
audit-test-coverage ─────────────────┤
audit-consolidated ◄─────────────────┘  (consumes all)
audit-consistency-coherence            (consumes all)
```

### Upstream Dependencies

- **audit-code-quality**: Refactoring consumes code-quality findings (god classes, dead code, smells)
  to prioritize modernization recommendations.
- **audit-architecture**: Refactoring may reference architecture findings for structural issues that
  require refactoring (layer violations, coupling problems).
- **audit-dependencies**: Refactoring may reference dependency data for deprecated package migration paths.

### Downstream Consumers

- **audit-consolidated**: Aggregates all squad findings into unified report.
- **audit-consistency-coherence**: Validates cross-domain alignment between squad findings.

### Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-refactoring | audit-code-quality | Code-quality identifies problems; refactoring recommends specific modernization approaches |
| audit-refactoring | audit-architecture | Architecture identifies structural issues; refactoring focuses on code-level pattern modernization |
| audit-refactoring | audit-dependencies | Dependencies tracks version/CVE; refactoring flags deprecated API usage patterns |

### Recommended Execution Order

For a full audit, this squad should run after:

1. **audit-dependencies** — Establish baseline of third-party risk
2. **audit-architecture** — Structural analysis
3. **audit-code-quality** — Implementation quality
4. **audit-refactoring** ← This squad (uses quality/architecture findings to prioritize)

Then downstream:
5. **audit-test-coverage** — Tests may need updates after refactoring recommendations

### Output Integration

When squads run sequentially:
1. Read the previous squad's findings from the shared output location.
2. Deduplicate: skip findings already reported by `audit-code-quality` (refactoring focuses on fix, not re-identification).
3. Cross-reference: escalate findings confirmed by multiple squads.
4. Output in the standardized Finding Schema format above for aggregation.
