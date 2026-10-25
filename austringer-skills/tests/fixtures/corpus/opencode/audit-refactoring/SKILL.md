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

- C# modernization — file-scoped namespaces, global usings, records, pattern matching, switch expressions
- Obsolete API detection — deprecated calls, outdated patterns, legacy constructs
- Async/await anti-patterns — `async void`, `.Result`, `.Wait()`, sync-over-async
- LINQ misuse — inefficient queries, multiple enumeration, premature materialization
- Duplicate code — copy-paste, similar blocks across files
- DTO mapping bugs — same value mapped to two fields, field order swap errors
- Python modernization — f-strings over `.format()`, `pathlib` over `os.path`, type unions, `match/case`
- JavaScript modernization — `var` to `let`/`const`, callback to async/await, CommonJS to ESM
- Read-only query optimization — tracking vs `AsNoTracking()`
- Technical debt inventory — score and prioritize refactoring opportunities

## DO NOT USE FOR

- Code style/naming (use `audit-code-quality`)
- Architecture/layer isolation (use `audit-architecture`)
- Security vulnerability scanning (use `audit-security`)
- Test coverage analysis (use `audit-test-coverage`)
- Dependency CVE/version auditing (use `audit-dependencies`)
- Writing code or changes (audit-only)

---

## Target Discovery

Agent MUST auto-detect project structure. Do NOT hardcode repo names.

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
| Terraform | `*.tf` | IaC |
| DB migrations | `migrations/`, `Migrations/`, `alembic/`, `db/migrate/` | Schema management |
| OTel | `otel-collector-config.yml`, `opentelemetry` deps | Observability |

### Discovery Procedure

1. **Scan root** for file patterns above.
2. **Identify primary language** by file count.
3. **Identify frameworks** by config analysis.
4. **Identify CI/CD** by workflow files.
5. **Identify infrastructure** by Docker/ORM/OTel configs.
6. **Select matching checklist** from Section 4.
7. **Multiple languages** (monorepo/polyglot) → run all applicable checklists.

---

## Technology-Specific Checklists

### .NET/C# Checklist

- [ ] **C# modernization**: Legacy patterns with modern equivalents:
  - Block-scoped → file-scoped namespaces
  - Missing global usings (`GlobalUsings.cs` or `ImplicitUsings`)
  - Classes that should be `record` types (immutable data carriers)
  - `switch` → switch expressions
  - `if`/`else` chains → pattern matching
  - Missing expression-bodied members
- [ ] **Obsolete API usage**:
  - `WebHostBuilder` → `HostBuilder` (.NET 6+)
  - `IWebHostBuilder` → `IHostBuilder`
  - `AddMvc()` → `AddControllers()` / `AddRazorPages()`
  - `AddEntityFramework*` → `AddDbContext` / `AddDbContextFactory`
  - Legacy `Polly.Extensions.Http` → `Microsoft.Extensions.Http.Resilience`
  - `xUnit` v2 → v3
- [ ] **Async/await anti-patterns**:
  - `async void` → `async Task`
  - `.Result` / `.Wait()` → `await`
  - `Task.Run` for CPU-bound in ASP.NET Core → remove
  - Missing `ConfigureAwait(false)` in library code
  - `async` without `await` → remove `async`, return `Task.CompletedTask`
- [ ] **LINQ misuse**:
  - Multiple enumeration of `IEnumerable` → `.ToList()` once
  - `.Where().Where()` chains → combine
  - `.Select(x => x).Where(...)` → no-op selects
  - `.Count()` when `.Any()` suffices
  - Missing `.AsNoTracking()` on read-only queries
- [ ] **Duplicate code**: Similar blocks (>80% match) across files:
  - Copy-paste handlers with minor variations
  - Repeated validation → shared validator
  - Repeated mapping → Mapster/automapper profiles
- [ ] **Copy/paste mapping bugs**: Same source → two DTO fields:
  ```csharp
  // Bug: FirstName mapped to both fields
  dto.FirstName = entity.FirstName;
  dto.LastName = entity.FirstName;  // ← should be entity.LastName
  ```
- [ ] **Missing expression-bodied members**: Single-line props/methods with full block.
- [ ] **Record type candidates**: Immutable data carriers with value equality.

### Python Checklist

- [ ] **Python 3.10+ patterns**:
  - `Union[X, Y]` → `X | Y`
  - `Optional[X]` → `X | None`
  - `if/elif` → `match/case`
  - `typing.Dict` → `dict` (Python 3.9+)
  - `typing.List` → `list`
  - `typing.Tuple` → `tuple`
- [ ] **Legacy format strings**:
  - `"Hello %s" % name` → `f"Hello {name}"`
  - `"Hello {}".format(name)` → `f"Hello {name}"`
  - String concatenation → f-strings
- [ ] **Outdated library usage**:
  - `os.path` → `pathlib.Path`
  - `collections.namedtuple` → `dataclasses.dataclass`
  - `typing.TypedDict` (pre-3.11) → direct class syntax
  - `requests` in async → `httpx`
- [ ] **Duplicate code**: Similar functions, repeated validation, copy-pasted transforms.
- [ ] **Legacy class patterns**: Old-style classes, `object` base in Python 3, long `__init__` → dataclasses.

### JavaScript/TypeScript Checklist

- [ ] **ES2022+ patterns**:
  - `var` → `let` / `const`
  - Callbacks → arrow functions
  - `.then()` → `async/await`
  - String concatenation → template literals
  - `arguments` → rest params (`...args`)
  - `for` loops → `.map()`, `.filter()`, `.reduce()`
- [ ] **Legacy callbacks**: `(err, result)` style → Promise/async/await
- [ ] **CommonJS vs ESM**:
  - `require()` / `module.exports` → `import` / `export`
  - `__dirname` → `import.meta.url`
- [ ] **Outdated React** (if React detected):
  - Class components → functional + hooks
  - `componentDidMount` → `useEffect`
  - `this.setState` → `useState`
  - String refs → `useRef`
  - HOCs → hooks
- [ ] **Duplicate code**: Similar components, repeated API calls, copy-pasted utils.
- [ ] **TypeScript**: Type assertions → type guards, `any` → discriminated unions.

### Go Checklist

- [ ] **Go 1.21+ modernization**:
  - `ioutil.ReadAll` / `ioutil.ReadFile` → `os.ReadFile`
  - `ioutil.WriteFile` → `os.WriteFile`
  - `ioutil.TempDir` → `os.MkdirTemp`
  - `ioutil.NopCloser` → `io.NopCloser`
  - `ioutil.Discard` → `io.Discard`
- [ ] **Error wrapping**:
  - `fmt.Errorf("error: %s", err)` → `fmt.Errorf("error: %w", err)`
  - Missing `%w` in middleware/layers
  - `log.Fatal` in library code
- [ ] **Context propagation**:
  - `context.WithTimeout` not deferred (leak)
  - Background ctx should be request ctx
  - Missing `ctx.Done()` cancellation checks
- [ ] **Synchronization**:
  - `sync/atomic` value ops → typed APIs (Go 1.19+)
  - `sync.Mutex` → `sync.RWMutex` for read-heavy
  - Missing `errgroup` for goroutines
- [ ] **Duplicate code**: Similar handlers, repeated validation, copy-pasted utils across packages.
- [ ] **Old-style tests**: No table-driven tests, missing `t.Parallel()`, no subtests.

### Java Checklist

- [ ] **Java 17+ modernization**:
  - `if/else` null checks → `Optional` / `orElseThrow`
  - Anonymous inner classes → lambdas / method references
  - `for (int i = 0; i < list.size(); i++)` → enhanced for / streams
  - `Date` / `Calendar` → `java.time.LocalDate` / `Instant`
  - `StringBuffer` → `StringBuilder`
  - `new HashMap<>()` → `Map.of()`, `List.of()` (small immutable)
- [ ] **Record types**: Immutable data carriers with `equals`/`hashCode`/`toString` boilerplate → `record`
- [ ] **Pattern matching**:
  - `instanceof` + cast → pattern matching for `instanceof` (Java 16+)
  - `switch` → enhanced switch expressions (Java 14+)
  - Missing sealed classes
- [ ] **Stream API**:
  - `for` with filter/map → stream pipelines
  - Manual null checks → `Optional` chaining
  - Blocking calls in stream ops
- [ ] **Obsolete APIs**:
  - `javax.*` → `jakarta.*`
  - Spring XML → Java config / annotations
  - JUnit 4 → JUnit 5
  - `MockitoJUnitRunner` → `MockitoExtension`
- [ ] **Duplicate code**: Similar controllers/services, repeated validation, copy-pasted JPA queries.
- [ ] **Try-with-resources**: Manual `try { } finally { close() }` → try-with-resources for `AutoCloseable`.

### Rust Checklist

- [ ] **Edition modernization**:
  - `extern crate` → edition 2018+ `use` imports
  - Missing `dyn Trait` for trait objects
  - `try!()` → `?` operator
  - `#[macro_use]` → `use` macro imports
- [ ] **Error handling**:
  - Manual `Box<dyn Error>` → `anyhow::Result`
  - Manual error types → `thiserror`
  - `.unwrap()` / `.expect()` in library → `?`
- [ ] **Concurrency**:
  - `Arc<Mutex<T>>` read-heavy → `arc_swap` / `RwLock`
  - `thread::spawn` → `tokio::spawn` in async
  - Missing `rayon` for CPU-bound parallel
- [ ] **Clippy modernization**:
  - Run `cargo clippy --pedantic`
  - Check deprecated methods
  - `.map().flatten()` → `.flat_map()`
- [ ] **Duplicate code**: Similar functions across modules, repeated error handling patterns.

---

## Anti-Patterns

### Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Sync-over-async | high | Blocking on async (`.Result`, `.Wait()`) | Deadlocks in ASP.NET, thread pool starvation | Propagate `async` through call chain |
| 2 | Copy-paste with bugs | high | Duplicated code with field-order bugs | Silent data corruption | Extract shared function, use mapping library |
| 3 | Legacy patterns accumulating | medium | New code uses old patterns | Tech debt grows, onboarding harder | Modernize incrementally, enforce in new code |
| 4 | Premature abstraction | low | Extracting for single-use code | Adds indirection without reduction | Wait for third occurrence (Rule of Three) |

### .NET/C# Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Read-only queries with tracking | high | No `AsNoTracking()` on read-only queries | Memory waste, slower, change tracker overhead | Add `.AsNoTracking()` to all read-only queries |
| 2 | DTO projection bugs | high | Same source mapped to two fields after copy-paste | Silent data corruption | Review mappings; use Mapster/automapper |
| 3 | Mapster mapping duplicates | medium | Same logic repeated in `TypeAdapterConfig` | Inconsistencies, maintenance burden | Centralize mapping configs |
| 4 | Block-scoped in file-scoped project | low | `namespace Foo { }` with file-scoped enforced | Style inconsistency, build warnings | `namespace Foo;` |
| 5 | Classes that should be records | medium | Manual equality on immutable data carriers | Boilerplate, error-prone | `record` / `record struct` |

### Python Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Legacy format strings | medium | `%` or `.format()` instead of f-strings | Less readable, no expression embedding | `f"Hello {name}"` |
| 2 | `os.path` over `pathlib` | low | String-based path manipulation | Platform-dependent, error-prone | `pathlib.Path` |
| 3 | `requests` in async | high | Sync HTTP blocking event loop | Blocks async runtime | `httpx.AsyncClient` |
| 4 | Old-style Union types | low | `Union[str, int]` instead of `str \| int` | Verbose in Python 3.10+ | `str \| int` |

### JavaScript/TypeScript Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | `var` declarations | medium | `var` instead of `let`/`const` | Function-scoped, hoisted, redeclaration | `const` default, `let` when reassignment |
| 2 | Callback-style async | medium | `(err, result) => {}` instead of Promises | Pyramid of doom, fragile error handling | async/await |
| 3 | CommonJS in ESM project | medium | `require()` in `"type": "module"` project | Inconsistent module system | `import` / `export` |
| 4 | Class components in React | low | Class when hooks available | More boilerplate | Functional + hooks |

### Go Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | `ioutil` package | low | Using deprecated `ioutil` | May be removed | `os.ReadFile`, `os.WriteFile`, etc. |
| 2 | Error not wrapped | medium | `fmt.Errorf("error: %s", err)` without `%w` | Prevents `errors.Is`/`errors.As` | `fmt.Errorf("context: %w", err)` |
| 3 | Raw mutex for read-heavy | low | `sync.Mutex` when most ops reads | Unnecessary contention | `sync.RWMutex` |
| 4 | No table-driven tests | medium | Separate test for each input | Duplication, harder to add cases | Table-driven with subtests |

### Java Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | `java.util.Date` / `Calendar` | low | Legacy date/time APIs | Mutable, thread-unsafe | `java.time.LocalDate`, `Instant` |
| 2 | Manual resource management | medium | `try { } finally { close() }` | Verbose, error-prone | Try-with-resources |
| 3 | JUnit 4 in modern project | medium | `org.junit.Test` instead of JUnit 5 | Missing features | Migrate to JUnit 5 |
| 4 | Legacy `for` with streams candidate | low | Manual iteration when streams clearer | Verbose, harder to parallelize | Stream API |

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

All audit findings MUST be emitted as structured JSON objects.

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

- **severity**: One of `critical`, `high`, `medium`, `low`. See Severity Guidelines.
- **file**: Relative from project root. Forward slashes. Never absolute.
- **line**: 1-based line where finding starts.
- **endLine**: Optional. Where finding ends (multi-line).
- **description**: Factual, specific. Include legacy pattern + modern replacement.
- **recommendation**: Actionable. Include before/after code snippet.
- **technology**: Primary tech.
- **category**: Typically `maintainability` or `performance`.

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
| **critical** | Data integrity or correctness risk | DTO mapping bug copying wrong field, sync-over-async causing deadlocks |
| **high** | Significant performance/maintainability risk | Missing `AsNoTracking()` on hot queries, `async void`, duplicate code with bugs |
| **medium** | Degraded maintainability, outdated patterns | Legacy format strings, block-scoped namespaces, callback patterns |
| **low** | Style modernization, minor improvements | `var` to `let`/`const`, `os.path` to `pathlib`, missing expression-bodied member |

### Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Data mapping/projection bug | +1 level | All |
| Causes production deadlocks/crashes | +1 level | .NET async |
| Hot path (frequently called) | +1 level | Performance-related |
| Multiple tenants/users | +1 level | Multi-tenant |
| Financial/billing calculation | +1 level | Financial |

### Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Test/demo code only | -1 level (min low) | All |
| Existing TODO/FIXME acknowledging it | -1 level (min low) | All |
| Deprecated code scheduled for removal | -1 level (min low) | All |
| Modern equivalent preview/unstable only | -1 level (min low) | All |

### Technology-Specific Severity Adjustments

#### .NET/C#
- Sync-over-async in ASP.NET Core → high
- Missing `AsNoTracking()` on read-only → medium
- DTO projection with copy-paste bug → critical
- Block-scoped namespace → low

#### Python
- `requests` in async → high
- Legacy format strings → low
- `os.path` instead of `pathlib` → low
- Old Union in Python 3.10+ → low

#### JavaScript/TypeScript
- `var` → low
- Callback-style async → medium
- CommonJS in ESM → medium
- Class components in React → low

#### Go
- `ioutil` → low
- Error not wrapped with `%w` → medium
- Raw `sync.Mutex` for read-heavy → low
- No table-driven tests → medium

#### Java
- Legacy `Date` / `Calendar` → low
- Manual resource management → medium
- JUnit 4 in modern project → medium
- Legacy `for` with streams candidate → low

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

- **audit-code-quality**: Consumes code-quality findings (god classes, dead code, smells) to prioritize modernization.
- **audit-architecture**: May reference architecture findings for structural issues requiring refactoring.
- **audit-dependencies**: May reference dependency data for deprecated package migration paths.

### Downstream Consumers

- **audit-consolidated**: Aggregates all findings.
- **audit-consistency-coherence**: Cross-domain alignment validation.

### Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-refactoring | audit-code-quality | Code-quality identifies problems; refactoring recommends specific modernization |
| audit-refactoring | audit-architecture | Architecture: structural issues; Refactoring: code-level pattern modernization |
| audit-refactoring | audit-dependencies | Dependencies: version/CVE; Refactoring: deprecated API usage patterns |

### Recommended Execution Order

1. **audit-dependencies** — Third-party risk baseline
2. **audit-architecture** — Structural analysis
3. **audit-code-quality** — Implementation quality
4. **audit-refactoring** ← This squad (uses quality/architecture to prioritize)
5. **audit-test-coverage** — Tests may need updates after refactoring recommendations

### Output Integration

When squads run sequentially:
1. Read previous squad's findings from shared output.
2. Deduplicate: skip findings already reported by `audit-code-quality`.
3. Cross-reference: escalate findings confirmed by multiple squads.
4. Output in standardized Finding Schema format for aggregation.