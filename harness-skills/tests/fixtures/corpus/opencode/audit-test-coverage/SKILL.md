---
name: audit-test-coverage
description: >-
  Test coverage auditing: identifies untested code paths, evaluates assertion
  effectiveness, detects test smells, assesses integration and E2E test quality,
  and recommends coverage improvements across multi-language codebases with
  various testing frameworks. Produces structured findings with severity ratings
  and concrete test improvement recommendations.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java"]
version: "2.0.0"
---

# Test Coverage & Quality Audit

## USE FOR

- Test coverage gap identification — untested functions, classes, modules, error paths
- Assertion quality review — weak assertions (`Assert.True`, bare `assertTrue`), missing edge cases
- Test smell detection — brittle tests, order-dependent, slow, test helpers in wrong locations
- xUnit/NUnit/MSTest pattern validation (.NET)
- pytest fixture reuse and parametrize assessment (Python)
- Jest/Vitest coverage threshold and test isolation review (JS)
- Integration test gap analysis — missing Testcontainers, mock assessment
- E2E test quality — permanently skipped tests, shared fixture leaks, prerequisite checks
- Test project structure — correct `IsTestProject` metadata, proper src project refs
- Mutation testing guidance — suggest tests that catch production code mutations
- Unit vs integration vs E2E test balance assessment

## DO NOT USE FOR

- Writing tests (use `audit-refactoring`)
- CI pipeline config (use `audit-cicd`)
- Code quality in prod code (use `audit-code-quality`)
- Security vuln testing (use `audit-security`)
- Performance/benchmark (use `audit-fault-tolerance`)

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

### Test Framework Detection

| Signal | Pattern | Detected Framework |
|--------|---------|--------------------|
| xUnit | `*.csproj` containing `xunit` | xUnit |
| NUnit | `*.csproj` containing `nunit` | NUnit |
| MSTest | `*.csproj` containing `MSTest` | MSTest |
| pytest | `pytest.ini`, `pyproject.toml` with `[tool.pytest]`, `conftest.py` | pytest |
| unittest | `*test*.py` with `unittest.TestCase` | unittest |
| Jest | `jest.config.*`, `package.json` with `jest` | Jest |
| Vitest | `vitest.config.*`, `package.json` with `vitest` | Vitest |
| Go testing | `*_test.go` files | Go testing |
| JUnit | `*Test.java` with `@Test` annotations | JUnit |
| Playwright | `playwright.config.*`, `*.csproj` with `Playwright` | Playwright E2E |

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
| DB migrations | `migrations/`, `Migrations/`, `alembic/`, `db/migrate/` | Schema mgmt |
| OTel | `otel-collector-config.yml`, `opentelemetry` in deps | Observability |
| Testcontainers | `*.cs` referencing `Testcontainers` | Integration test containers |

### Discovery Procedure

1. Scan root dir for language/test framework signals.
2. Identify test dirs (`tests/`, `test/`, `__tests__/`, `*Tests/`).
3. Identify test framework by config/dep analysis.
4. Identify E2E framework (Playwright, Cypress, Selenium).
5. Identify CI/CD by workflow file presence.
6. Identify infrastructure by Docker/Testcontainers configs.
7. Select matching checklist from Section 4 based on detected tech.
8. If multiple test frameworks, run all applicable checklists.

---

## Technology-Specific Checklists

### .NET/C# Checklist

- [ ] **xUnit/NUnit test patterns**:
  - `[Fact]` for deterministic, `[Theory]` with `[InlineData]` for parameterized
  - No void return in async tests — must return `Task`
  - No `IDisposable` field init in ctor when `IAsyncLifetime` better
  - Fluent assertions over raw `Assert.True`
- [ ] **Assertion quality**:
  - `Assert.Equal(expected, actual)` over `Assert.True(expected == actual)` — better failure messages
  - `Assert.NotNull` before accessing members
  - No `Assert.True(true)` or tautological assertions
  - All thrown exceptions asserted with specific type: `Assert.ThrowsAsync<T>(...)`
- [ ] **Moq/NSubstitute proper usage**:
  - No `Mock.Of<T>()` when setup needed — use `new Mock<T>()` with explicit setup
  - Verify calls: `_mock.Verify(x => x.Method(), Times.Once())`
  - No over-mocking (>5 mocks suggests decomposition)
  - Mocks reset between tests
- [ ] **Integration test gaps**:
  - Testcontainers for DB-dependent tests
  - No direct calls to external APIs — use mock/recorded responses
  - Test DB seeded consistently per run
  - Migration tests verify schema applies cleanly
- [ ] **E2E test issues**:
  - No permanently skipped tests — use prerequisite checks instead
  - `[CollectionDefinition]` for shared expensive resources
  - Fresh page per test (no shared IPage state leaks)
  - E2E config fails loudly for missing config
- [ ] **Test naming**: `MethodName_Scenario_ExpectedResult` or descriptive sentence. No `Test1`, `Test2`.
- [ ] **Missing edge case coverage**: Null/empty inputs, boundary values, error paths, concurrent access
- [ ] **Test project structure**: `IsTestProject=true` only on test projects, not source. Correct project refs. Shared fixtures in test utility project.

### Python Checklist

- [ ] **pytest coverage**: Threshold in `pyproject.toml`. Test files mirror `src/` structure. Branch coverage (`--cov-branch`).
- [ ] **Fixture reuse**: Common in `conftest.py`, not duplicated. Scope appropriate (function/class/module/session).
- [ ] **Parametrize**: `@pytest.mark.parametrize` for multiple inputs. No copy-pasted test functions. `ids` for readable names.
- [ ] **Mock hygiene**: `patch` scope limited in fixture/context manager. No patches leaking between tests. `autospec=True`.
- [ ] **Test isolation**: No order-dependent tests. DB in transactions/test DB. File system uses `tmp_path`.
- [ ] **Edge case coverage**: Empty lists, None, empty strings, type errors, boundary values.

### JavaScript/TypeScript Checklist

- [ ] **Jest/Vitest coverage**: Threshold in config. Branch, function, line all measured. `--coverage` in CI.
- [ ] **Test isolation**: No shared mutable state. `beforeEach`/`afterEach` for cleanup. Randomize with `--randomize`/`--shuffle`.
- [ ] **Snapshot maintenance**: Reviewed on update. Large snapshots (>50 lines) replaced with specific assertions. Committed to VCS.
- [ ] **Async patterns**: All async tests use `async/await`. Proper timeout config. No floating promises.
- [ ] **Mock patterns**: `jest.mock()` at file level, `jest.fn()` inline. Restoration in `afterEach`. No partial mocking hiding bugs.
- [ ] **Edge case coverage**: Null/undefined/empty. Error paths. Concurrent ops and race conditions.

### Go Checklist

- [ ] **Table-driven tests**: Multiple input/output cases using table-driven with subtests (`t.Run`). No copy-paste.
- [ ] **Test file naming**: `*_test.go` naming. `TestXxx` functions.
- [ ] **Parallel execution**: `t.Parallel()` for independent tests. Missing on slow tests flagged.
- [ ] **Integration tests**: Build tag (`//go:build integration`). No direct external API calls without mock.
- [ ] **Mock patterns**: Interface-based mocking with `testify/mock`. No hand-written mocks returning defaults silently.
- [ ] **Benchmark tests**: `BenchmarkXxx` for perf-sensitive code. `b.ReportAllocs()`.
- [ ] **Edge case coverage**: Empty inputs, nil slices, boundary values, errors, context cancellation.
- [ ] **Fixtures**: Test data in `testdata/`. No hardcoded paths. `t.TempDir()` for temp files.
- [ ] **Error path testing**: All error returns tested. Mocked error returns verified.

### Java Checklist

- [ ] **JUnit 5 usage**: `@Test` from Jupiter, not JUnit 4. `@DisplayName`. `@ParameterizedTest` with `@CsvSource`/`@ValueSource`/`@MethodSource`.
- [ ] **Assertion quality**: AssertJ/Hamcrest over raw `assertEquals`. `assertThrows` for exceptions. Chained assertions.
- [ ] **Mockito patterns**: `@Mock`/`@InjectMocks` with `MockitoExtension`. No `MockitoJUnitRunner`. `verify()` with call counts. No over-mocking.
- [ ] **Integration test gaps**: Testcontainers for DB. `@SpringBootTest` only necessary (prefer `@WebMvcTest`, `@DataJpaTest`).
- [ ] **Test isolation**: `@DirtiesContext` only necessary. Per-class DB seeding. `@BeforeEach` cleanup.
- [ ] **Coverage thresholds**: JaCoCo/Sonar in `pom.xml`/`build.gradle`. Branch coverage measured.
- [ ] **Edge case coverage**: Nulls, empty collections, boundary values, error paths, `Optional` empty case.
- [ ] **Test structure**: `src/test/java` mirrors `src/main/java`. No test deps in compile scope.

---

## Anti-Patterns

### Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Permanently skipped tests | high | Tests with hardcoded Skip and no re-enable path | Dead tests give false confidence | Prerequisite checks skip dynamically |
| 2 | Tautological assertions | high | Always-pass assertions (`Assert.True(true)`, `expect(true).toBe(true)`) | Zero test value, false coverage | Assert actual computed values |
| 3 | Shared mutable state | high | Tests modifying shared state without cleanup | Order-dependent failures, flaky | Each test gets fresh state; cleanup in afterEach/Dispose |
| 4 | Fragile integration tests | medium | Direct external API calls (network, DB without container) | Flaky, slow, fails on network issues | Testcontainers, mock providers, recorded responses |
| 5 | Missing error path tests | medium | Only happy path tested | Production errors untested, break silently | Test all error paths: exceptions, null returns, timeouts |

### .NET/C# Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | E2E permanently skipped | high | `[Fact(Skip = "E2E not configured")]` never runs | Zero E2E coverage, no one notices | Dynamic prerequisite check |
| 2 | Integration tests calling external APIs | high | Real external services without mocking | Fragile, network issues, slow, costs | Mock providers or recorded responses |
| 3 | Single IPage reused across E2E tests | high | Shared Playwright page across test methods | State leaks between tests | Fresh page per test via fixture |
| 4 | E2E config falling back silently | medium | Missing env vars fall back to localhost:5000 | Tests appear to pass but not testing real env | Fail loudly: throw if missing |
| 5 | src project marked IsTestProject=true | medium | Source project has `IsTestProject>true</IsTestProject>` | Mixes test deps into library, breaks packing | Only actual test projects get `IsTestProject=true` |
| 6 | Missing [CollectionDefinition] | medium | Tests sharing expensive resources without collection definition | Parallel execution conflicts | Use `[CollectionDefinition]` and `[Collection]` |

### Python Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Patches leaking between tests | high | `@patch` at class level without cleanup | Order-dependent failures | `autospec=True`, fixture-scoped patches, context managers |
| 2 | No branch coverage | medium | Only line coverage | Condition branches untested | Enable `--cov-branch` |
| 3 | Copy-pasted test functions | medium | Minor variations duplicated | Maintenance burden | `@pytest.mark.parametrize` |
| 4 | Hardcoded file paths | medium | Absolute/hardcoded relative paths | Breaks on diff machines/CI | `tmp_path` or `pathlib` relative |

### JavaScript/TypeScript Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Floating promises | high | `async` tests without `await` on inner promises | Tests complete before assertions, false passes | Always await, enable `no-floating-promises` |
| 2 | Snapshots for large objects | medium | >50 line snapshots comparing entire API responses | Brittle, any field breaks test | Assert specific fields |
| 3 | No mock cleanup | medium | `jest.fn()` mocks not restored between tests | State leaks, order-dependent failures | `jest.restoreAllMocks()` in afterEach |
| 4 | Missing timeout config | low | Long-running async without timeout | Tests hang indefinitely in CI | Set per-test or global timeout |

### Go Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Missing t.Parallel() | medium | Independent tests not parallel | Slower CI | Add `t.Parallel()` at start |
| 2 | No table-driven tests | medium | Separate functions per input/output | Copy-paste, missed edge cases | Table-driven with `t.Run` subtests |
| 3 | Integration tests without build tag | medium | Real deps not separated with build tag | Tests fail when deps unavailable | `//go:build integration` |
| 4 | Hardcoded testdata paths | low | Absolute/fragile relative paths | Breaks on diff machines | `t.TempDir()`, `testdata/` with `filepath.Join` |

### Java Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | JUnit 4 mixed with JUnit 5 | medium | `@Test` from `org.junit.Test` alongside Jupiter | Conflicting runners, tests skipped | Migrate to `org.junit.jupiter.api.Test` |
| 2 | No @ParameterizedTest | medium | Copy-pasted test methods | Maintenance burden, missed edge cases | `@ParameterizedTest` with `@CsvSource`/`@MethodSource` |
| 3 | MockitoJUnitRunner instead of Extension | low | `@RunWith(MockitoJUnitRunner.class)` | JUnit 4-specific | `@ExtendWith(MockitoExtension.class)` |
| 4 | @SpringBootTest when slice suffices | medium | Full app context for focused test | Slow, unnecessary deps | `@WebMvcTest`, `@DataJpaTest`, `@JsonTest` |

---

## Code Examples (Bad/Good Pairs)

### .NET/C# Example 1: Prerequisite Check vs Permanent Skip

```csharp
// ❌ BAD: Permanently skipped — never runs, gives false confidence
[Fact(Skip = "E2E tests need Playwright installed")]
public async Task Login_ValidCredentials_NavigatesToDashboard()
{
    var page = await _browser.NewPageAsync();
    await page.GotoAsync("http://localhost:5000/login");
    await page.FillAsync("#email", "admin@test.com");
    await page.FillAsync("#password", "password");
    await page.ClickAsync("#login-button");
    await Expect(page).ToHaveURLAsync("**/dashboard");
}

// ✅ GOOD: Dynamic prerequisite check — skips only when conditions not met
[Fact]
public async Task Login_ValidCredentials_NavigatesToDashboard()
{
    var baseUrl = Environment.GetEnvironmentVariable("E2E_BASE_URL");
    if (string.IsNullOrEmpty(baseUrl))
    {
        throw new SkipException("E2E_BASE_URL not configured. Set environment variable to run E2E tests.");
    }

    await using var browser = await Playwright.CreateBrowserAsync();
    var page = await browser.NewPageAsync();
    await page.GotoAsync($"{baseUrl}/login");
    await page.FillAsync("#email", "admin@test.com");
    await page.FillAsync("#password", "password");
    await page.ClickAsync("#login-button");
    await Expect(page).ToHaveURLAsync("**/dashboard");
}
```

### .NET/C# Example 2: Testcontainers for Integration Tests

```csharp
// ❌ BAD: Using shared development database — fragile, not isolated
public class PersonRepositoryTests : IClassFixture<SharedDatabaseFixture>
{
    private readonly SharedDatabaseFixture _fixture;

    public PersonRepositoryTests(SharedDatabaseFixture fixture) { _fixture = fixture; }

    [Fact]
    public async Task Create_SavesPerson()
    {
        var repo = new PersonRepository(_fixture.DbContext);
        var person = new Person { FirstName = "Test", LastName = "User" };
        await repo.CreateAsync(person);
    }
}

// ✅ GOOD: Testcontainers — isolated database per test run
public class PersonRepositoryTests : IAsyncLifetime
{
    private PostgreSqlContainer _container = null!;
    private FamilyOfficeDbContext _dbContext = null!;

    public async Task InitializeAsync()
    {
        _container = new PostgreSqlBuilder().WithImage("postgres:16-alpine").Build();
        await _container.StartAsync();
        var options = new DbContextOptionsBuilder<FamilyOfficeDbContext>()
            .UseNpgsql(_container.GetConnectionString()).Options;
        _dbContext = new FamilyOfficeDbContext(options);
        await _dbContext.Database.EnsureCreatedAsync();
    }

    public async Task DisposeAsync() { await _dbContext.DisposeAsync(); await _container.DisposeAsync(); }

    [Fact]
    public async Task Create_SavesPerson()
    {
        var repo = new PersonRepository(_dbContext);
        var person = new Person { FirstName = "Test", LastName = "User" };
        await repo.CreateAsync(person);
        var saved = await _dbContext.People.FindAsync(person.Id);
        Assert.Equal("Test", saved!.FirstName);
    }
}
```

### .NET/C# Example 3: Proper IsTestProject Scoping

```xml
<!-- ❌ BAD: Source project marked as test project -->
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
    <IsTestProject>true</IsTestProject>  <!-- WRONG: source project! -->
  </PropertyGroup>
  <ItemGroup>
    <PackageReference Include="xunit" Version="2.9.3" />
    <PackageReference Include="Moq" Version="4.20.72" />
  </ItemGroup>
</Project>

<!-- ✅ GOOD: Source project without test metadata -->
<!-- src/FamilyOffice.Modules.People/FamilyOffice.Modules.People.csproj -->
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup><TargetFramework>net10.0</TargetFramework></PropertyGroup>
</Project>

<!-- tests/FamilyOffice.Tests.Unit/FamilyOffice.Tests.Unit.csproj -->
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
    <IsTestProject>true</IsTestProject>  <!-- Correct -->
  </PropertyGroup>
  <ItemGroup>
    <PackageReference Include="xunit" Version="2.9.3" />
    <PackageReference Include="Moq" Version="4.20.72" />
  </ItemGroup>
</Project>
```

### Python Example 1: Parametrize Instead of Copy-Paste

```python
# ❌ BAD: Copy-pasted test functions
def test_calculate_tax_zero_rate():
    result = calculate_tax(100.0, 0.0)
    assert result == 100.0

def test_calculate_tax_standard_rate():
    result = calculate_tax(100.0, 0.2)
    assert result == 120.0

# ✅ GOOD: Parametrized — one function, multiple cases
@pytest.mark.parametrize(
    "amount, rate, expected",
    [
        (100.0, 0.0, 100.0),
        (100.0, 0.2, 120.0),
        (0.0, 0.2, 0.0),
        (100.0, -0.1, None),
    ],
    ids=["zero_rate", "standard_rate", "zero_amount", "negative_rate"],
)
def test_calculate_tax(amount, rate, expected):
    if rate < 0:
        with pytest.raises(ValueError):
            calculate_tax(amount, rate)
    else:
        assert calculate_tax(amount, rate) == expected
```

### JavaScript/TypeScript Example 1: Snapshot vs Specific Assertions

```typescript
// ❌ BAD: Large snapshot
test("getUser returns user data", async () => {
    const user = await getUser(1);
    expect(user).toMatchSnapshot(); // 80-line snapshot, any change breaks
});

// ✅ GOOD: Specific assertions on relevant fields
test("getUser returns user data", async () => {
    const user = await getUser(1);
    expect(user.id).toBe(1);
    expect(user.email).toBe("test@example.com");
    expect(user.role).toBe("admin");
});
```

### Go Example 1: Table-Driven Tests

```go
// ❌ BAD: Copy-pasted test functions
func TestCalculateTaxZeroRate(t *testing.T) {
    result := calculateTax(100.0, 0.0)
    if result != 100.0 { t.Errorf("expected 100.0, got %f", result) }
}

// ✅ GOOD: Table-driven test with subtests
func TestCalculateTax(t *testing.T) {
    tests := []struct {
        name string; amount float64; rate float64; expected float64; wantErr bool
    }{
        {"zero rate", 100.0, 0.0, 100.0, false},
        {"standard rate", 100.0, 0.2, 120.0, false},
        {"zero amount", 0.0, 0.2, 0.0, false},
        {"negative rate", 100.0, -0.1, 0, true},
    }
    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            result, err := calculateTax(tt.amount, tt.rate)
            if tt.wantErr && err == nil { t.Error("expected error") }
            if !tt.wantErr && result != tt.expected { t.Errorf("expected %f, got %f", tt.expected, result) }
        })
    }
}
```

### Java Example 1: Parameterized Tests

```java
// ❌ BAD: Copy-pasted test methods
@Test void calculateTaxZeroRate() {
    assertEquals(100.0, TaxCalculator.calculate(100.0, 0.0));
}
@Test void calculateTaxStandardRate() {
    assertEquals(120.0, TaxCalculator.calculate(100.0, 0.2));
}

// ✅ GOOD: Parameterized test with @CsvSource
@ParameterizedTest
@CsvSource({ "100.0, 0.0, 100.0", "100.0, 0.2, 120.0", "0.0, 0.2, 0.0" })
@DisplayName("Calculate tax for valid rates")
void calculateTax(double amount, double rate, double expected) {
    assertEquals(expected, TaxCalculator.calculate(amount, rate));
}
```

---

## Output Format

All audit findings MUST be emitted as structured JSON objects.

### Finding Schema

```json
{
  "severity": "critical|high|medium|low",
  "file": "relative/path/to/test/file",
  "line": 42,
  "endLine": 45,
  "description": "What was found and why. Include test smell, gap, or anti-pattern.",
  "recommendation": "Specific fix with brief code example. Reference testing pattern.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|CrossLanguage",
  "category": "testing|maintainability|architecture|performance|security|compliance|observability"
}
```

### Field Rules

- **severity**: One of `critical`, `high`, `medium`, `low`. See Severity Guidelines.
- **file**: Relative from project root. Forward slashes. Never absolute.
- **line**: 1-based line where finding starts.
- **endLine**: Optional. Line where finding ends.
- **description**: Factual, specific. Include test pattern and why problematic.
- **recommendation**: Actionable. Include code snippet when possible.
- **technology**: Primary tech.
- **category**: `testing` or `maintainability` typically.

### Summary Report

```json
{
  "summary": {
    "squad": "audit-test-coverage",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#", "Python"],
    "totalFindings": 25,
    "bySeverity": { "critical": 2, "high": 5, "medium": 12, "low": 6 },
    "byCategory": { "testing": 18, "maintainability": 4, "architecture": 2, "performance": 1 },
    "filesScanned": 60,
    "filesWithFindings": 22
  }
}
```

---

## Severity Guidelines

### Base Severity Definitions

| Level | Definition | Test Coverage Example |
|-------|-----------|----------------------|
| **critical** | Critical business logic with zero tests, tests that never run | Financial calc with no tests, permanently skipped E2E |
| **high** | Core functions <50% branch coverage, anti-patterns hiding bugs | Tautological assertions, shared mutable state, fragile external API |
| **medium** | Missing edge case tests, structure issues, moderate smells | Missing error paths, no parametrize, snapshot bloat |
| **low** | Test organization issues, minor improvements, naming | File naming, fixture org, missing test IDs |

### Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding in financial/billing calc with no tests | +1 level | All tech |
| Finding affects auth/authorization test coverage | +1 level | All tech |
| Finding in data validation with no error path tests | +1 level | All tech |
| Skipped tests >10% of total | +1 level | All tech |
| Finding in multi-tenant isolation boundary with no tests | +1 level | Multi-tenant |

### Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding in deprecated code | -1 level (min low) | All tech |
| Finding in demo/seed code | -1 level (min low) | All tech |
| Test file exists but needs improvement | -1 level (min low) | All tech |

### Technology-Specific Severity Adjustments

#### .NET/C#
- Permanently skipped E2E → high (zero E2E coverage)
- src project `IsTestProject=true` → medium (build contamination)
- Shared IPage across E2E → high (state leaks)
- Missing [CollectionDefinition] → medium (parallel conflicts)

#### Python
- Patches leaking between tests → high (order-dependent)
- Missing branch coverage → medium (incomplete picture)
- Copy-pasted test functions → medium (maintainability)
- No conftest.py fixture sharing → low (organization)

#### JavaScript/TypeScript
- Floating promises → high (false passes)
- Large snapshots → medium (brittle)
- No mock cleanup → medium (state leaks)
- Missing coverage threshold → medium (no enforcement)

#### Go
- Missing t.Parallel() → medium (CI speed)
- No table-driven tests → medium (maintainability)
- Integration tests without build tag → medium (isolation)
- Hardcoded testdata paths → low (portability)

#### Java
- JUnit 4 mixed with JUnit 5 → medium (runner conflicts)
- No @ParameterizedTest → medium (maintainability)
- @SpringBootTest when slice suffices → medium (test speed)

---

## Integration Notes

### Squad Dependency Graph

```
audit-code-quality ──────────────────┐
audit-architecture ──────────────────┤
audit-refactoring ───────────────────┤
audit-test-coverage ◄────────────────┤  (references quality/architecture findings)
audit-consolidated ◄─────────────────┘  (consumes all)
audit-consistency-coherence            (consumes all)
```

### Upstream Dependencies

- **audit-code-quality**: Identifies god classes and complex functions needing test coverage
- **audit-architecture**: Finds architectural boundaries needing integration tests

### Downstream Consumers

- **audit-consolidated**: Aggregates all findings
- **audit-consistency-coherence**: Cross-domain alignment validation

### Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-test-coverage | audit-code-quality | Test-coverage: test quality; code-quality: prod code quality |
| audit-test-coverage | audit-refactoring | Test-coverage: test gaps; refactoring: test restructuring |
| audit-test-coverage | audit-cicd | Test-coverage: tests; cicd: test pipeline config |

### Recommended Execution Order

1. **audit-dependencies** — Third-party risk baseline
2. **audit-security** — Code-level vuln scan
3. **audit-architecture** — Structural analysis
4. **audit-code-quality** — Impl quality
5. **audit-test-coverage** ← This squad

### Output Integration

When squads run sequentially:
1. Read previous squad's findings from shared output location.
2. Deduplicate: skip already reported findings.
3. Cross-reference: escalate confirmed by multiple squads.
4. Output in standardized Finding Schema format for aggregation.