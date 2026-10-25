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

- Test coverage gap identification — untested functions, classes, modules, and error paths
- Assertion quality review — weak assertions (`Assert.True`, bare `assertTrue`), missing edge cases
- Test smell detection — brittle tests, order-dependent tests, slow tests, test helpers in wrong locations
- xUnit/NUnit/MSTest test pattern validation for .NET projects
- pytest fixture reuse and parametrize pattern assessment for Python projects
- Jest/Vitest coverage threshold and test isolation review for JavaScript projects
- Integration test gap analysis — missing Testcontainers, external API mock assessment
- E2E test quality review — permanently skipped tests, shared fixture state leaks, prerequisite checks
- Test project structure validation — correct `IsTestProject` metadata, proper src project references
- Mutation testing guidance — suggest which tests would catch production code mutations
- Unit vs integration vs E2E test balance assessment

## DO NOT USE FOR

- Writing tests (use `audit-refactoring` for test-related refactoring)
- CI pipeline configuration (use `audit-cicd`)
- Code quality in production code (use `audit-code-quality`)
- Security vulnerability testing (use `audit-security`)
- Performance/benchmark testing (use `audit-fault-tolerance`)

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
| Terraform | `*.tf` | Infrastructure as Code |
| Database migrations | `migrations/`, `Migrations/`, `alembic/`, `db/migrate/` | Schema management |
| OTel | `otel-collector-config.yml`, `opentelemetry` in dependencies | Observability |
| Testcontainers | `*.cs` referencing `Testcontainers` | Integration test containers |

### Discovery Procedure

1. **Scan root directory** for all language/test framework signals above.
2. **Identify test directories** (`tests/`, `test/`, `__tests__/`, `*Tests/`).
3. **Identify test framework** by config file/dependency analysis.
4. **Identify E2E framework** (Playwright, Cypress, Selenium) by config presence.
5. **Identify CI/CD** by workflow file presence.
6. **Identify infrastructure** by Docker/Testcontainers configs.
7. **Select matching checklist** from Section 4 based on detected technologies.
8. **If multiple test frameworks detected**, run all applicable checklists.

---

## Technology-Specific Checklists

### .NET/C# Checklist

- [ ] **xUnit/NUnit test patterns**:
  - `[Fact]` for deterministic tests, `[Theory]` with `[InlineData]` for parameterized
  - No test methods with void return type in async tests — must return `Task`
  - No `IDisposable` field initialization in test class constructor when `IAsyncLifetime` would be better
  - Proper use of `AssertionHelper` / fluent assertions over raw `Assert.True`
- [ ] **Assertion quality**:
  - `Assert.Equal(expected, actual)` over `Assert.True(expected == actual)` — better failure messages
  - `Assert.NotNull` before accessing members (not null-conditional with no assertion)
  - No `Assert.True(true)` or tautological assertions
  - All thrown exceptions asserted: `Assert.ThrowsAsync<T>(...)` with specific type
- [ ] **Moq/NSubstitute proper usage**:
  - No `Mock.Of<T>()` when setup is needed — use `new Mock<T>()` with explicit setup
  - Verify calls on mocks: `_mock.Verify(x => x.Method(), Times.Once())`
  - No over-mocking: if >5 mocks in a single test, the class may need decomposition
  - Mocks reset between tests — no shared mutable mock state
- [ ] **Integration test gaps**:
  - Testcontainers used for database-dependent tests
  - No direct calls to external APIs (fragile, flaky) — use mock providers or recorded responses
  - Test database seeded with consistent data for each test run
  - Migration tests verify schema applies cleanly
- [ ] **E2E test issues**:
  - No permanently skipped tests with hardcoded skip messages — use prerequisite checks instead
  - Missing `[CollectionDefinition]` for test collections that share expensive resources
  - Shared `IPage` fixture leaking state between tests — each test should get fresh page
  - E2E config should fail loudly for missing config, not fall back to hardcoded defaults
- [ ] **Test naming conventions**:
  - `MethodName_Scenario_ExpectedResult` or descriptive name that reads as a sentence
  - No `Test1`, `Test2`, `TestMethod` generic names
- [ ] **Missing edge case coverage**:
  - Null/empty inputs, boundary values (0, int.MaxValue, DateTime.MinValue)
  - Error paths (exceptions, failed HTTP calls, timeout)
  - Concurrent access patterns for multi-tenant code
- [ ] **Test project structure**:
  - `IsTestProject=true` only on actual test projects, NOT on source projects
  - Test projects reference source projects correctly (not circular dependencies)
  - Shared test fixtures in dedicated test utility project

### Python Checklist

- [ ] **pytest coverage**:
  - Coverage threshold configured in `pyproject.toml` (`[tool.coverage]`)
  - Missing test files for modules (check `tests/` mirrors `src/` structure)
  - Branch coverage enabled (`--cov-branch`), not just line coverage
- [ ] **Fixture reuse**:
  - Common fixtures in `conftest.py`, not duplicated across test files
  - Fixture scope appropriate (function, class, module, session)
  - No expensive fixture operations in function-scoped fixtures without need
- [ ] **Parametrize usage**:
  - `@pytest.mark.parametrize` for testing multiple inputs/outputs
  - No copy-pasted test functions with minor variations — parametrize them
  - `ids` parameter for readable test names in parametrize
- [ ] **Mock hygiene**:
  - `unittest.mock.patch` scope limited — patched in fixture or context manager
  - No patches leaking between tests (use `autospec=True` to catch interface changes)
  - Mock assertions verify call count and arguments
- [ ] **Test isolation**:
  - No order-dependent tests (each test runs independently)
  - Database tests use transactions or test database, not production
  - File system tests use `tmp_path` fixture, not hardcoded paths
- [ ] **Edge case coverage**:
  - Empty lists, None inputs, empty strings
  - Type errors for wrong input types
  - Boundary values for numerical functions

### JavaScript/TypeScript Checklist

- [ ] **Jest/Vitest coverage**:
  - Coverage threshold set in config (`coverageThreshold`)
  - Branch, function, and line coverage all measured
  - Coverage report generated in CI (`--coverage`)
- [ ] **Test isolation**:
  - No shared mutable state between tests
  - `beforeEach`/`afterEach` for cleanup
  - No order-dependent tests (randomize with `--randomize` or `jest --shuffle`)
- [ ] **Snapshot test maintenance**:
  - Snapshots reviewed on update (not blindly accepted with `-u`)
  - Large snapshots (>50 lines) replaced with specific assertions
  - Snapshot files committed to version control
- [ ] **Async test patterns**:
  - All async tests use `async/await` (not `.then()` chains)
  - Proper timeout configuration for long-running tests
  - No floating promises — all promises awaited or explicitly voided
- [ ] **Mock patterns**:
  - `jest.mock()` at file level, `jest.fn()` for inline mocks
  - Mock restoration in `afterEach` (`jest.restoreAllMocks()`)
  - No partial mocking that hides bugs
- [ ] **Edge case coverage**:
  - Null/undefined inputs, empty arrays
  - Error paths (network failures, invalid responses)
  - Concurrent operations and race conditions

### Go Checklist

- [ ] **Table-driven tests**: Functions with multiple input/output cases use table-driven tests with subtests (`t.Run`). No copy-pasted test functions with minor input variations.
- [ ] **Test files naming**: Test files follow `*_test.go` naming convention. Test functions use `TestXxx` naming.
- [ ] **Parallel execution**: `t.Parallel()` used where tests are independent. Missing `t.Parallel()` on slow tests flagged.
- [ ] **Integration tests**: Integration tests tagged with build tags (`//go:build integration`). No direct calls to external APIs without mock/recorded responses.
- [ ] **Mock patterns**: Interface-based mocking with `testify/mock` or generated mocks. No hand-written mock implementations that silently return default values.
- [ ] **Benchmark tests**: `BenchmarkXxx` functions for performance-sensitive code. `b.ReportAllocs()` for allocation tracking.
- [ ] **Edge case coverage**: Empty inputs, nil slices, boundary values, error returns, context cancellation. Table-driven tests include edge cases, not just happy path.
- [ ] **Fixtures**: Test data in `testdata/` directory. No hardcoded file paths. `t.TempDir()` for temporary files.
- [ ] **Error path testing**: All error returns tested. Mocked error returns from dependencies verified. No tests that ignore error returns.

### Java Checklist

- [ ] **JUnit 5 usage**: `@Test` from `org.junit.jupiter.api.Test`, not JUnit 4. `@DisplayName` for readable test names. `@ParameterizedTest` with `@CsvSource`, `@ValueSource`, `@MethodSource`.
- [ ] **Assertion quality**: AssertJ or Hamcrest fluent assertions over raw `assertEquals`. `assertThrows` for exception testing. Chained assertions for multiple fields.
- [ ] **Mockito patterns**: `@Mock` / `@InjectMocks` with `MockitoExtension`. No `MockitoJUnitRunner` (JUnit 4). `verify()` with call counts. No over-mocking (>5 mocks suggests decomposition).
- [ ] **Integration test gaps**: Testcontainers for database-dependent tests. `@SpringBootTest` only when necessary (prefer focused slice tests: `@WebMvcTest`, `@DataJpaTest`).
- [ ] **Test isolation**: `@DirtiesContext` only when necessary. Test database seeded per test class (not shared mutable state). `@BeforeEach` cleanup.
- [ ] **Coverage thresholds**: JaCoCo or Sonar configured in `pom.xml`/`build.gradle`. Branch coverage measured, not just line coverage.
- [ ] **Edge case coverage**: Null inputs, empty collections, boundary values, error paths, concurrent access patterns. `Optional` empty case tested.
- [ ] **Test project structure**: Tests in `src/test/java` mirroring `src/main/java`. No test dependencies in compile scope. Shared fixtures in base class or test utility project.

---

## Anti-Patterns

### Cross-Language Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Permanently skipped tests | high | Tests with hardcoded skip/Skip attribute and no way to re-enable | Dead tests give false confidence in coverage | Use prerequisite checks that skip dynamically when conditions not met |
| 2 | Tautological assertions | high | Assertions that always pass (`Assert.True(true)`, `expect(true).toBe(true)`) | Zero test value, false coverage metrics | Assert actual computed values against expected |
| 3 | Shared mutable test state | high | Tests that modify shared state without cleanup | Order-dependent failures, flaky tests | Each test gets fresh state; cleanup in afterEach/Dispose |
| 4 | Fragile integration tests | medium | Tests calling external APIs directly (network, database without container) | Flaky, slow, fails on network issues | Use Testcontainers, mock providers, recorded responses |
| 5 | Missing error path tests | medium | Only happy path tested, no tests for exceptions/failures | Production errors untested, break silently | Test all error paths: exceptions, null returns, timeouts |

### .NET/C# Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | E2E tests permanently skipped | high | `[Fact(Skip = "E2E not configured")]` hardcoded, never runs | E2E coverage is zero, no one notices | Dynamic prerequisite check: skip only when env vars/URL missing |
| 2 | Integration tests calling external APIs | high | Tests call real external services without mocking | Fragile, fails on network issues, slow, costs money | Use mock providers or recorded HTTP responses |
| 3 | Single IPage reused across E2E tests | high | Shared Playwright page object across test methods | State leaks between tests (cookies, localStorage, navigation) | Each test gets fresh page via fixture |
| 4 | E2E config falling back to defaults silently | medium | Missing env vars silently fall back to localhost:5000 | Tests appear to pass but aren't testing real environment | Fail loudly: throw if required config is missing |
| 5 | src project marked `IsTestProject=true` | medium | Source project has `<IsTestProject>true</IsTestProject>` | Mixes test dependencies into library, breaks packing | Only mark actual test projects with `IsTestProject=true` |
| 6 | Missing `[CollectionDefinition]` | medium | Tests sharing expensive resources without collection definition | Parallel execution conflicts, resource contention | Use `[CollectionDefinition]` and `[Collection]` for shared fixtures |

### Python Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Patches leaking between tests | high | `@patch` at class level without cleanup | Modified state persists, tests become order-dependent | Use `autospec=True`, fixture-scoped patches, or context managers |
| 2 | No branch coverage | medium | Only line coverage measured | Condition branches untested (if/else only one path tested) | Enable `--cov-branch` in pytest config |
| 3 | Copy-pasted test functions | medium | Similar test functions with minor input variations | Maintenance burden, easy to miss cases | Use `@pytest.mark.parametrize` |
| 4 | Hardcoded test file paths | medium | Tests using absolute or hardcoded relative paths | Breaks on different machines/CI | Use `tmp_path` fixture or `pathlib` relative to test location |

### JavaScript/TypeScript Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Floating promises in tests | high | `async` test functions without `await` on inner promises | Tests complete before assertions, false passes | Always await all promises, enable `no-floating-promises` |
| 2 | Snapshot tests for large objects | medium | Snapshots >50 lines comparing entire API responses | Brittle, any field change breaks test, hides real issues | Assert specific fields that matter for the test case |
| 3 | No mock cleanup | medium | `jest.fn()` mocks not restored between tests | Mock state leaks, order-dependent failures | Call `jest.restoreAllMocks()` in `afterEach` |
| 4 | Missing timeout configuration | low | Long-running async tests without timeout | Tests hang indefinitely in CI | Set per-test or global timeout |

### Go Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Missing `t.Parallel()` in independent tests | medium | Tests that could run in parallel but don't | Slower CI pipeline, test suite takes longer | Add `t.Parallel()` at the start of independent tests |
| 2 | No table-driven tests for multiple cases | medium | Separate test functions for each input/output case | Copy-paste maintenance, easy to miss edge cases | Use table-driven tests with subtests `t.Run` |
| 3 | Integration tests without build tag | medium | Tests calling real dependencies not separated with build tag | Tests fail when dependencies not available, mixed with unit tests | Use `//go:build integration` to separate long-running tests |
| 4 | Hardcoded `testdata` paths | low | Test functions referencing absolute or fragile relative paths | Breaks on different machines, in CI | Use `t.TempDir()` for temporary files, `testdata/` for static data with `filepath.Join` |

### Java Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | JUnit 4 mixed with JUnit 5 | medium | `@Test` from `org.junit.Test` alongside Jupiter imports | Conflicting test runners, some tests not picked up | Migrate all to `org.junit.jupiter.api.Test` |
| 2 | No `@ParameterizedTest` for multiple cases | medium | Copy-pasted test methods with minor input variations | Maintenance burden, easy to miss edge cases | Use `@ParameterizedTest` with `@CsvSource` or `@MethodSource` |
| 3 | MockitoJUnitRunner instead of MockitoExtension | low | Using `@RunWith(MockitoJUnitRunner.class)` instead of `@ExtendWith(MockitoExtension.class)` | JUnit 4-specific, missing JUnit 5 integration | Use `@ExtendWith(MockitoExtension.class)` |
| 4 | `@SpringBootTest` when slice test suffices | medium | Loading full application context for focused repository/controller test | Slow test execution, unnecessary dependencies | Use `@WebMvcTest`, `@DataJpaTest`, `@JsonTest` slice annotations |

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
    // Prerequisite check — skip dynamically with clear reason
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

    public PersonRepositoryTests(SharedDatabaseFixture fixture)
    {
        _fixture = fixture; // Shared DB with other test classes — data conflicts
    }

    [Fact]
    public async Task Create_SavesPerson()
    {
        var repo = new PersonRepository(_fixture.DbContext);
        var person = new Person { FirstName = "Test", LastName = "User" };
        await repo.CreateAsync(person);
        // May fail if another test class runs concurrently with same data
    }
}

// ✅ GOOD: Testcontainers — isolated database per test run
public class PersonRepositoryTests : IAsyncLifetime
{
    private PostgreSqlContainer _container = null!;
    private FamilyOfficeDbContext _dbContext = null!;

    public async Task InitializeAsync()
    {
        _container = new PostgreSqlBuilder()
            .WithImage("postgres:16-alpine")
            .Build();
        await _container.StartAsync();

        var options = new DbContextOptionsBuilder<FamilyOfficeDbContext>()
            .UseNpgsql(_container.GetConnectionString())
            .Options;
        _dbContext = new FamilyOfficeDbContext(options);
        await _dbContext.Database.EnsureCreatedAsync();
    }

    public async Task DisposeAsync()
    {
        await _dbContext.DisposeAsync();
        await _container.DisposeAsync();
    }

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
<!-- ❌ BAD: Source project marked as test project — pulls test dependencies into library -->
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
    <IsTestProject>true</IsTestProject>  <!-- WRONG: This is a source project! -->
  </PropertyGroup>
  <ItemGroup>
    <PackageReference Include="xunit" Version="2.9.3" />
    <PackageReference Include="Moq" Version="4.20.72" />
  </ItemGroup>
</Project>

<!-- ✅ GOOD: Source project without test metadata, separate test project -->
<!-- src/FamilyOffice.Modules.People/FamilyOffice.Modules.People.csproj -->
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
    <!-- No IsTestProject — this is a library -->
  </PropertyGroup>
</Project>

<!-- tests/FamilyOffice.Tests.Unit/FamilyOffice.Tests.Unit.csproj -->
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
    <IsTestProject>true</IsTestProject>  <!-- Correct: this IS a test project -->
  </PropertyGroup>
  <ItemGroup>
    <PackageReference Include="xunit" Version="2.9.3" />
    <PackageReference Include="Moq" Version="4.20.72" />
  </ItemGroup>
</Project>
```

### Python Example 1: Parametrize Instead of Copy-Paste

```python
# ❌ BAD: Copy-pasted test functions with minor variations
def test_calculate_tax_zero_rate():
    result = calculate_tax(100.0, 0.0)
    assert result == 100.0

def test_calculate_tax_standard_rate():
    result = calculate_tax(100.0, 0.2)
    assert result == 120.0

def test_calculate_tax_high_rate():
    result = calculate_tax(100.0, 0.5)
    assert result == 150.0

def test_calculate_tax_max_rate():
    result = calculate_tax(100.0, 1.0)
    assert result == 200.0

# ✅ GOOD: Parametrized — one test function, multiple cases
@pytest.mark.parametrize(
    "amount, rate, expected",
    [
        (100.0, 0.0, 100.0),
        (100.0, 0.2, 120.0),
        (100.0, 0.5, 150.0),
        (100.0, 1.0, 200.0),
        (0.0, 0.2, 0.0),       # edge case: zero amount
        (100.0, -0.1, None),    # edge case: negative rate should raise
    ],
    ids=["zero_rate", "standard_rate", "high_rate", "max_rate", "zero_amount", "negative_rate"],
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
// ❌ BAD: Large snapshot — any field change breaks the test
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
    expect(user.active).toBe(true);
    // Only assert fields that matter for this test's purpose
});
```

### Go Example 1: Table-Driven Tests

```go
// ❌ BAD: Copy-pasted test functions for each case
func TestCalculateTaxZeroRate(t *testing.T) {
    result := calculateTax(100.0, 0.0)
    if result != 100.0 {
        t.Errorf("expected 100.0, got %f", result)
    }
}

func TestCalculateTaxStandardRate(t *testing.T) {
    result := calculateTax(100.0, 0.2)
    if result != 120.0 {
        t.Errorf("expected 120.0, got %f", result)
    }
}

// ✅ GOOD: Table-driven test with subtests
func TestCalculateTax(t *testing.T) {
    tests := []struct {
        name     string
        amount   float64
        rate     float64
        expected float64
        wantErr  bool
    }{
        {"zero rate", 100.0, 0.0, 100.0, false},
        {"standard rate", 100.0, 0.2, 120.0, false},
        {"high rate", 100.0, 0.5, 150.0, false},
        {"zero amount", 0.0, 0.2, 0.0, false},
        {"negative rate", 100.0, -0.1, 0, true},
    }
    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            result, err := calculateTax(tt.amount, tt.rate)
            if tt.wantErr && err == nil {
                t.Error("expected error, got nil")
            }
            if !tt.wantErr && result != tt.expected {
                t.Errorf("expected %f, got %f", tt.expected, result)
            }
        })
    }
}
```

### Java Example 1: Parameterized Tests

```java
// ❌ BAD: Copy-pasted test methods for each case
@Test
void calculateTaxZeroRate() {
    double result = TaxCalculator.calculate(100.0, 0.0);
    assertEquals(100.0, result);
}

@Test
void calculateTaxStandardRate() {
    double result = TaxCalculator.calculate(100.0, 0.2);
    assertEquals(120.0, result);
}

// ✅ GOOD: Parameterized test with @CsvSource
@ParameterizedTest
@CsvSource({
    "100.0, 0.0, 100.0",
    "100.0, 0.2, 120.0",
    "100.0, 0.5, 150.0",
    "0.0,   0.2, 0.0"
})
@DisplayName("Calculate tax for valid rates")
void calculateTax(double amount, double rate, double expected) {
    assertEquals(expected, TaxCalculator.calculate(amount, rate));
}
```

---

## Output Format

All audit findings MUST be emitted as structured JSON objects. One finding per object.

### Finding Schema

```json
{
  "severity": "critical|high|medium|low",
  "file": "relative/path/to/test/file",
  "line": 42,
  "endLine": 45,
  "description": "What was found and why it matters. Include the test smell, gap, or anti-pattern detected.",
  "recommendation": "Specific fix with brief code example. Reference testing pattern or framework feature.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|CrossLanguage",
  "category": "testing|maintainability|architecture|performance|security|compliance|observability"
}
```

### Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Severity Guidelines below.
- **file**: Relative path from project root. Use forward slashes. Never absolute paths.
- **line**: 1-based line number where the finding starts.
- **endLine**: Optional. Line where the finding ends (for multi-line issues).
- **description**: Factual, specific. Include the test pattern and why it's problematic.
- **recommendation**: Actionable. Include a code snippet showing the fix when possible.
- **technology**: Primary technology of the finding.
- **category**: Primary category — for test coverage, typically `testing` or `maintainability`.

### Summary Report

After all findings, emit a summary:

```json
{
  "summary": {
    "squad": "audit-test-coverage",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#", "Python"],
    "totalFindings": 25,
    "bySeverity": {
      "critical": 2,
      "high": 5,
      "medium": 12,
      "low": 6
    },
    "byCategory": {
      "testing": 18,
      "maintainability": 4,
      "architecture": 2,
      "performance": 1
    },
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
| **critical** | Critical business logic with zero test coverage, or tests that never run | Financial calculation with no tests, permanently skipped E2E suite |
| **high** | Core functions with <50% branch coverage, test anti-patterns that hide bugs | Tautological assertions, shared mutable state, fragile external API calls |
| **medium** | Missing edge case tests, test structure issues, moderate test smells | Missing error path tests, no parametrize for obvious cases, snapshot bloat |
| **low** | Test organization issues, minor improvements, naming conventions | Test file naming, fixture organization, missing test IDs |

### Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Finding is in financial/billing calculation with no tests | +1 level | All technologies |
| Finding affects authentication/authorization test coverage | +1 level | All technologies |
| Finding is in data validation logic with no error path tests | +1 level | All technologies |
| Skipped test count exceeds 10% of total test count | +1 level | All technologies |
| Finding is in multi-tenant isolation boundary with no tests | +1 level | Multi-tenant systems |

### Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding is in deprecated code scheduled for removal | -1 level (min low) | All technologies |
| Finding is in demo/seed code | -1 level (min low) | All technologies |
| Test file exists but needs improvement (not missing entirely) | -1 level (min low) | All technologies |

### Technology-Specific Severity Adjustments

#### .NET/C#
- Permanently skipped E2E tests → high (zero E2E coverage)
- src project with `IsTestProject=true` → medium (build artifact contamination)
- Shared `IPage` across E2E tests → high (state leaks)
- Missing `[CollectionDefinition]` for shared fixture → medium (parallel conflicts)

#### Python
- Patches leaking between tests → high (order-dependent failures)
- Missing branch coverage config → medium (incomplete coverage picture)
- Copy-pasted test functions → medium (maintainability)
- No `conftest.py` fixture sharing → low (organization)

#### JavaScript/TypeScript
- Floating promises in tests → high (false passes)
- Large snapshot tests → medium (brittle)
- No mock cleanup → medium (state leaks)
- Missing coverage threshold → medium (no enforcement)

#### Go
- Missing `t.Parallel()` in independent tests → medium (CI speed)
- No table-driven tests for multiple cases → medium (maintainability)
- Integration tests without build tag → medium (test isolation)
- Hardcoded `testdata` paths → low (portability)

#### Java
- JUnit 4 mixed with JUnit 5 → medium (test runner conflicts)
- No `@ParameterizedTest` for multi-case → medium (maintainability)
- `@SpringBootTest` when slice test suffices → medium (test speed)

---

## Integration Notes

### Squad Dependency Graph

```
audit-code-quality ──────────────────┐
audit-architecture ──────────────────┤
audit-refactoring ───────────────────┤
audit-test-coverage ◄────────────────┤  (references quality and architecture findings)
audit-consolidated ◄─────────────────┘  (consumes all)
audit-consistency-coherence            (consumes all)
```

### Upstream Dependencies

- **audit-code-quality**: Test coverage references code-quality findings to identify which god classes
  and complex functions most need test coverage.
- **audit-architecture**: Test coverage references architecture findings to verify that architectural
  boundaries have corresponding integration tests.

### Downstream Consumers

- **audit-consolidated**: Aggregates all squad findings into unified report.
- **audit-consistency-coherence**: Validates cross-domain alignment between squad findings.

### Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-test-coverage | audit-code-quality | Test-coverage focuses on test quality; code-quality focuses on production code quality |
| audit-test-coverage | audit-refactoring | Test-coverage identifies test gaps; refactoring may recommend test restructuring |
| audit-test-coverage | audit-cicd | Test-coverage evaluates tests; cicd evaluates test pipeline configuration |

### Recommended Execution Order

For a full audit, this squad should run after:

1. **audit-dependencies** — Establish baseline of third-party risk
2. **audit-security** — Code-level vulnerability scan
3. **audit-architecture** — Structural analysis
4. **audit-code-quality** — Implementation quality
5. **audit-test-coverage** ← This squad (uses quality findings to prioritize coverage gaps)

### Output Integration

When squads run sequentially:
1. Read the previous squad's findings from the shared output location.
2. Deduplicate: skip findings already reported by a previous squad.
3. Cross-reference: escalate findings confirmed by multiple squads (e.g., god class with no tests).
4. Output in the standardized Finding Schema format above for aggregation.
