---
name: audit-architecture
description: >-
  Architectural integrity auditing: layer isolation, coupling analysis, dependency direction,
  module boundary enforcement, DTO leak detection, and structural pattern validation across
  all major programming languages and frameworks.
languages: [".NET/C#", "Python", "JavaScript", "Go", "Java"]
version: "2.0.0"
---

# Architecture Integrity Audit Squad

## USE FOR

- Layer isolation verification — domain layer must not import infrastructure/presentation types
- Coupling analysis — circular dependencies, tight coupling between modules, inappropriate intimacy
- Dependency direction enforcement — dependencies should flow inward (infrastructure → domain ← presentation)
- Module boundary validation — each module has clear responsibility, no god modules, stable interfaces
- DTO leak detection — data transfer objects bleeding into inappropriate layers
- Project structure analysis — verify naming conventions, folder organization, project references
- CQRS pattern compliance — command handlers don't mix with query handlers, proper separation
- Clean Architecture validation — concentric layer boundaries respected, no outward dependencies from domain
- Package dependency graph validation — no cycles, reasonable depth, clear module interfaces
- Interface segregation — interfaces defined by consumers, not producers; small focused contracts
- **.NET/C#**: Clean Architecture layer validation, module csproj reference graph analysis, SharedKernel boundary checks, CQRS handler separation, middleware ordering
- **Python**: Package import graph analysis, circular import detection, layer boundary enforcement in Django/FastAPI/Flask apps
- **JavaScript/TypeScript**: Monorepo package boundaries, barrel export integrity, path alias validation, component architecture review
- **Go**: Internal package boundaries, interface placement verification (consumer-defined), init() function audit
- **Java**: ArchUnit layer rules, Maven/Gradle multi-module coupling, Spring bean scope validation
- **Rust**: Crate dependency direction, module visibility enforcement, feature flag architecture

### DO NOT USE FOR

- Code style and naming conventions (use `audit-code-quality`)
- Refactoring suggestions (use `audit-refactoring`)
- Security vulnerability analysis (use `audit-security`)
- Test coverage gaps (use `audit-test-coverage`)
- Performance optimization (use `audit-code-quality` for N+1 queries)

---

## Section 3: Target Discovery

The agent MUST auto-detect the project structure. Do NOT hardcode repository names.

### 3.1 Language Detection

| Signal | File Pattern | Detected Language |
|--------|-------------|-------------------|
| C# source files | `*.cs`, `*.csproj`, `*.sln`, `*.slnx` | .NET/C# |
| Python source files | `*.py`, `*.pyi`, `pyproject.toml`, `requirements.txt`, `setup.py`, `Pipfile` | Python |
| JavaScript source files | `*.js`, `*.mjs`, `*.cjs` | JavaScript |
| TypeScript source files | `*.ts`, `*.tsx`, `tsconfig.json` | TypeScript |
| Go source files | `*.go`, `go.mod`, `go.sum` | Go |
| Java source files | `*.java`, `pom.xml`, `build.gradle`, `build.gradle.kts` | Java |
| Rust source files | `*.rs`, `Cargo.toml`, `Cargo.lock` | Rust |

### 3.2 Architecture-Relevant File Detection

| Signal | File Pattern | Architecture Concern |
|--------|-------------|----------------------|
| Project files | `*.csproj`, `*.fsproj`, `pyproject.toml`, `package.json`, `go.mod`, `pom.xml`, `build.gradle` | Module structure and dependencies |
| Solution files | `*.sln`, `*.slnx` | Solution-level project references |
| Module folders | `src/*/`, `lib/*/`, `pkg/*/`, `internal/*/`, `modules/*/` | Module boundaries |
| Layer directories | `domain/`, `application/`, `infrastructure/`, `presentation/`, `api/`, `web/` | Layer isolation |
| DI registration | `*ModuleRegistration*`, `*ServiceRegistration*`, `*CompositionRoot*` | Service registration patterns |
| Handlers | `*Handler*`, `*Command*`, `*Query*` | CQRS pattern compliance |
| Entities | `*Entity*`, `*ValueObject*`, `*Aggregate*` | Domain model purity |
| Repositories | `*Repository*`, `*DbContext*`, `*Context*` | Data access patterns |
| Config | `Directory.Build.props`, `Directory.Packages.props`, `tsconfig.json`, `go.mod` | Project-level structure |

### 3.3 Framework Detection

| Signal | File Pattern | Detected Framework |
|--------|-------------|--------------------|
| ASP.NET Core | `Program.cs` + `*.csproj` containing `Microsoft.AspNetCore` | ASP.NET Core |
| Blazor | `*.razor`, `*.razor.cs` | Blazor Server/WebAssembly |
| EF Core | `*.csproj` containing `Microsoft.EntityFrameworkCore` | Entity Framework Core |
| Django | `manage.py`, `settings.py` containing `DJANGO_SETTINGS_MODULE` | Django |
| FastAPI | `*.py` importing `fastapi` or `pyproject.toml` listing `fastapi` | FastAPI |
| Flask | `*.py` importing `flask` | Flask |
| Express | `package.json` listing `express` | Express.js |
| Spring Boot | `pom.xml`/`build.gradle` containing `spring-boot` | Spring Boot |
| Monorepo | `workspaces` in `package.json`, `go.work`, `.slnx` | Multi-module structure |

### 3.4 Test Framework Detection

| Signal | Pattern | Detected Framework |
|--------|---------|--------------------|
| xUnit/NUnit/MSTest | `*.csproj` containing `xunit`/`nunit`/`MSTest` | .NET test frameworks |
| pytest | `pytest.ini`, `pyproject.toml` with `[tool.pytest]`, `conftest.py` | pytest |
| Jest | `jest.config.*`, `package.json` with `jest` | Jest |
| Go testing | `*_test.go` files | Go testing |
| JUnit | `*Test.java` with `@Test` annotations | JUnit |

### 3.5 CI/CD Detection

| Signal | File Pattern | Platform |
|--------|-------------|----------|
| GitLab CI | `.gitlab-ci.yml` | GitLab |
| GitHub Actions | `.github/workflows/*.yml` | GitHub |
| Azure Pipelines | `azure-pipelines.yml` | Azure DevOps |

### 3.6 Discovery Procedure

1. **Scan root directory** for project/solution files to understand module structure.
2. **Identify primary language** by source file count.
3. **Map project references** — read `*.csproj` `<ProjectReference>`, `go.mod` imports, `package.json` dependencies, `pom.xml` modules.
4. **Identify layer structure** by directory analysis (domain, application, infrastructure, presentation).
5. **Detect architectural pattern**: Clean Architecture, Hexagonal, CQRS, MVC, layered.
6. **Build dependency graph** — identify circular dependencies and violation edges.
7. **Select the matching checklist** from Section 4 based on detected technologies.
8. **If multiple languages detected** (monorepo, polyglot), run all applicable checklists.

---

## Section 4: Technology-Specific Checklists

### 4.1 .NET/C# Architecture Checklist

- [ ] **Clean Architecture layer isolation**: Domain project must NOT reference infrastructure,
  presentation, or any external framework projects. Verify `*.csproj` has zero framework references
  beyond `SharedKernel`.
- [ ] **CQRS handler boundaries**: Command handlers must not call query methods and vice versa.
  Verify handlers follow single-responsibility (one command = one handler).
- [ ] **Domain entity purity**: Domain entities must NOT import EF Core types (`DbContext`,
  `DbSet`, `IEntityTypeConfiguration`), MediatR, Wolverine, or any infrastructure namespace.
- [ ] **Module coupling via csproj references**: Build reference graph from `*.csproj` files.
  No module should depend on 5+ other domain modules. Cross-module communication via events/interfaces only.
- [ ] **God classes**: Flag classes >300 LOC that handle multiple concerns (e.g., entity + validation +
  persistence). Split into focused types.
- [ ] **Dead code**: Unused parameters, private methods never called, internal classes never referenced.
  Use Roslyn analyzers (IDE0051, IDE0052) to detect.
- [ ] **DTO leak across layers**: DTOs defined in infrastructure/presentation layer must not appear in
  domain method signatures. Domain should use domain types; mapping happens at the boundary.
- [ ] **Infrastructure referencing domain directly**: Infrastructure projects may reference domain (correct
  dependency direction). Domain must NOT reference infrastructure.
- [ ] **Reporting module coupling**: If a reporting module exists, verify it uses a read model or
  projection pattern rather than directly querying 7+ domain module databases.
- [ ] **Module registration isolation**: Each module's `*ModuleRegistration.cs` should register only its
  own services. Cross-module dependencies resolved via shared interfaces, not direct references.
- [ ] **SharedKernel boundaries**: `SharedKernel` must contain only truly shared primitives (Entity,
  ValueObject, Result). No domain-specific logic or infrastructure types.
- [ ] **Handler directory organization**: Commands, queries, and events organized in separate directories
  with consistent naming (`Commands/`, `Queries/`, `Events/`, `Handlers/`).
- [ ] **No static mutable state**: No `static` mutable fields in services. All state managed via DI.
- [ ] **Middleware ordering**: Authentication → Authorization → Exception Handling → Custom middleware.
  Verify `Program.cs` middleware registration order.

### 4.2 Python Architecture Checklist

- [ ] **Package layer isolation**: `domain/` must not import from `infrastructure/`, `api/`, `presentation/`.
  Use `import` graph analysis to verify dependency direction.
- [ ] **Circular import detection**: Identify circular imports between modules. Python allows them but
  they cause subtle bugs and coupling.
- [ ] **Module coupling**: No module should import from more than 3-4 sibling modules. High fan-in/fan-out
  indicates god module.
- [ ] **God modules**: Flag `*.py` files >500 LOC with functions handling multiple concerns.
- [ ] **Interface segregation**: Abstract base classes should have 3-5 methods max. Large ABCs should be
  split into focused protocols.
- [ ] **Dependency injection**: Services should be injected, not imported and instantiated directly.
  Check for `Service()` constructor calls inside other service classes.
- [ ] **DTO/model leak**: Pydantic models or dataclasses defined for API responses should not appear
  in domain logic. Use domain models internally, convert at API boundary.
- [ ] **Configuration separation**: Config loading in dedicated module, not scattered across service files.
  Use `pydantic-settings` or similar.
- [ ] **Entry point clarity**: Single entry point (`main.py`, `app.py`). No business logic in entry point.
- [ ] **Test mirror structure**: Test directory mirrors source directory structure. `tests/domain/test_entity.py`
  for `src/domain/entity.py`.

### 4.3 JavaScript/TypeScript Architecture Checklist

- [ ] **Module boundary enforcement**: Use TypeScript path aliases or package boundaries. No deep imports
  across module boundaries (`import { X } from '../../../other-module/internal'`).
- [ ] **Dependency direction**: `ui/` → `features/` → `domain/` → `data/`. No reverse dependencies.
- [ ] **Barrel exports**: Each module has `index.ts` barrel export. Internal files not imported outside module.
- [ ] **God components**: React/Vue components >300 LOC handling data fetching, business logic, and UI.
  Extract into hooks, services, and presentational components.
- [ ] **Type sharing**: Shared types in dedicated `types/` or `@shared/` package. No circular type imports.
- [ ] **API layer isolation**: API calls in dedicated service layer, not in components or hooks directly.
  Service layer returns domain types, not raw API responses.
- [ ] **State management architecture**: Global state in store (Redux/Zustand/Context), local state in
  components. No prop drilling beyond 2 levels.
- [ ] **Dead exports**: Functions/types exported but never imported elsewhere. Use `ts-prune` or similar.
- [ ] **Package structure**: Monorepo packages have clear boundaries. Use `workspace:*` for internal deps.

### 4.4 Go Architecture Checklist

- [ ] **Package dependency direction**: `cmd/` → `internal/` → `pkg/`. No reverse dependencies.
  `pkg/` packages should not import `internal/`.
- [ ] **Circular import detection**: Go compiler catches these, but verify proposed designs don't
  create near-circular dependencies via interfaces.
- [ ] **Interface placement**: Interfaces defined by consumers, not producers. `consumer/package/interface.go`
  defines what it needs, not `producer/package/interface.go`.
- [ ] **God packages**: Packages with >20 exported types or >1000 LOC. Split into focused packages.
- [ ] **Internal package usage**: Implementation details in `internal/`. Public API in `pkg/` or root packages.
- [ ] **Dependency injection**: Dependencies passed as interfaces in constructors, not global variables
  or `init()` functions.

### 4.5 Java Architecture Checklist

- [ ] **Layer isolation**: `domain/` package must not import `infrastructure/`, `web/`, or framework types.
  Verify with ArchUnit or similar.
- [ ] **Module coupling**: Maven/Gradle multi-module project. No circular dependencies between modules.
  Verify with `mvn detect` or dependency:tree.
- [ ] **God classes**: Flag classes >500 LOC with >10 public methods. Split using extraction patterns.
- [ ] **DTO boundaries**: DTOs in `api/` or `web/` package. Domain layer uses entities/value objects.
  MapStruct or manual mapping at layer boundaries.
- [ ] **Spring bean scoping**: Verify bean scopes are intentional. No request-scoped beans injected into
  singletons without proxy.
- [ ] **Package naming conventions**: Packages follow reverse-domain naming. Feature-based or layer-based
  structure consistent across modules.

---

## Section 5: Anti-Patterns

### 5.1 Cross-Language Architecture Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Circular dependencies | critical | Module A imports Module B which imports Module A | Tight coupling, build order issues, impossible to test in isolation | Extract shared code to Module C; use events/interfaces for cross-module communication |
| 2 | Domain importing framework types | critical | Domain entities reference ORM, DI container, or web framework types | Domain coupled to infrastructure; cannot test or reuse independently | Domain defines interfaces; infrastructure implements them |
| 3 | God class/module | high | Single class/module with >10 responsibilities or >300 LOC multi-concern | Violates SRP, hard to test, hard to maintain, merge conflicts | Split into focused classes/modules with single responsibility |
| 4 | DTO leak across layers | high | API response DTOs used in domain logic or business layer | Domain logic depends on presentation format changes | Domain uses domain types; mapping at layer boundaries |
| 5 | Feature envy | medium | Method in Module A uses more data from Module B than its own | Logic belongs in wrong module, hidden coupling | Move method to Module B or use proper interface |
| 6 | Scattered configuration | medium | Config loading spread across multiple files | Hard to understand configuration surface, inconsistent defaults | Centralized config module with strongly-typed models |

### 5.2 .NET/C# Architecture Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Domain entities importing EF types | critical | `using Microsoft.EntityFrameworkCore` in domain entity files | Domain layer coupled to ORM; cannot switch or test independently | Repository interface in domain; EF implementation in infrastructure |
| 2 | Handlers directly using DbContext | high | Command/query handlers inject `DbContext` directly | Handlers coupled to persistence; no abstraction for testing | Handlers use repository interface; only repositories inject DbContext |
| 3 | Infrastructure module depending on domain module | medium | Infrastructure project references specific domain module | Creates tight coupling between infra and one domain | Infrastructure depends on domain abstractions/interfaces package |
| 4 | Cross-module direct entity access | high | Financial module directly imports Tax module entity types | Modules tightly coupled; changes cascade across modules | Use integration events or shared interface contracts |
| 5 | SharedKernel with domain logic | medium | SharedKernel contains business rules specific to one module | SharedKernel becomes a dumping ground, coupling all modules | SharedKernel contains only primitives (Entity, ValueObject, Result) |
| 6 | Missing module registration | medium | New module added without `*ModuleRegistration.cs` | Module services not registered, DI resolution fails at runtime | Every module has registration class discovered in Program.cs |

### 5.3 Python Architecture Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Circular imports | critical | Module A imports B at top level, B imports A at top level | Import errors, AttributeError at runtime, tight coupling | Extract shared code to third module; use lazy imports; restructure |
| 2 | God module | high | Single .py file >500 LOC with mixed concerns | Hard to test, understand, and maintain | Split into focused modules following SRP |
| 3 | Framework types in domain | critical | Domain functions import Flask request/response, SQLAlchemy models | Domain coupled to web framework; cannot reuse | Domain uses pure Python types; framework integration at boundaries |
| 4 | Scattered service instantiation | medium | `service = ExternalService()` called throughout codebase | Hard to swap implementations, no centralized config | Dependency injection via function parameters or DI container |
| 5 | Missing __init__.py boundaries | low | Package __init__.py exposes all internal modules | Internal implementation details become public API | Explicit __all__ or minimal exports in __init__.py |

### 5.4 JavaScript/TypeScript Architecture Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Deep relative imports across modules | high | `import { X } from '../../../other-module/src/internal'` | Fragile coupling; breaks on refactoring | Use path aliases or package boundaries; barrel exports |
| 2 | God components | high | React/Vue component >300 LOC with data fetching + logic + UI | Hard to test, reuse, or understand | Extract hooks for logic, services for API, presentational for UI |
| 3 | Global mutable state | high | Module-level variables mutated at runtime | Unpredictable behavior, hard to test, concurrency issues | Use state management library or pass state explicitly |
| 4 | Type coupling | medium | Internal types from one module used directly in another | Changes propagate; no encapsulation | Define interface types for cross-module contracts |
| 5 | Circular dependencies | critical | Module A imports B which imports A | Runtime errors, undefined values, bundle bloat | Extract shared code; use dependency inversion |

---

## Section 6: Code Examples (Bad/Good Pairs)

### 6.1 .NET/C# Examples

#### Example 1: Domain Purity — EF Types in Domain Entity

```csharp
// ❌ BAD: Domain entity importing EF Core types
using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Metadata.Builders;

namespace MyApp.Domain.Entities;

public class Order
{
    public Guid Id { get; set; }
    public List<OrderLine> Lines { get; set; } = [];
    // EF navigation property coupling
    public DateTimeOffset CreatedAt { get; set; }
}

// This configuration should NOT be in the domain project
public class OrderConfiguration : IEntityTypeConfiguration<Order>
{
    public void Configure(EntityTypeBuilder<Order> builder) { ... }
}
```

```csharp
// ✅ GOOD: Domain entity is pure, no framework imports
namespace MyApp.Domain.Entities;

public sealed class Order : Entity
{
    private readonly List<OrderLine> _lines = [];
    public IReadOnlyList<OrderLine> Lines => _lines.AsReadOnly();
    public DateTimeOffset CreatedAt { get; }

    public void AddLine(OrderLine line)
    {
        // Domain logic here — no infrastructure concern
        _lines.Add(line);
    }
}

// Configuration lives in infrastructure project
// MyApp.Infrastructure.Persistence/Configurations/OrderConfiguration.cs
```

#### Example 2: Repository Interface — Handler Using DbContext Directly

```csharp
// ❌ BAD: Handler directly depends on DbContext
public class CreateOrderHandler
{
    private readonly AppDbContext _dbContext;

    public CreateOrderHandler(AppDbContext dbContext)
    {
        _dbContext = dbContext;
    }

    public async Task Handle(CreateOrderCommand command)
    {
        var order = new Order(command.CustomerId);
        _dbContext.Orders.Add(order);
        await _dbContext.SaveChangesAsync();
    }
}
```

```csharp
// ✅ GOOD: Handler depends on repository interface (defined in domain)
public class CreateOrderHandler
{
    private readonly IOrderRepository _orderRepository;

    public CreateOrderHandler(IOrderRepository orderRepository)
    {
        _orderRepository = orderRepository;
    }

    public async Task Handle(CreateOrderCommand command)
    {
        var order = new Order(command.CustomerId);
        await _orderRepository.AddAsync(order);
    }
}

// Interface in domain project
namespace MyApp.Domain.Repositories;
public interface IOrderRepository
{
    Task AddAsync(Order order);
    Task<Order?> GetByIdAsync(Guid id);
}
```

#### Example 3: Module Boundary — Cross-Module Direct Access

```csharp
// ❌ BAD: Financial module directly imports Tax module entity types
using MyApp.Modules.Tax.Entities; // Direct module dependency!

namespace MyApp.Modules.Financial.Services;

public class TransactionService
{
    public Money CalculateNet(Transaction tx, TaxBracket bracket) { ... }
}
```

```csharp
// ✅ GOOD: Module communicates via shared interface or event
namespace MyApp.Modules.Financial.Services;

public class TransactionService
{
    private readonly ITaxCalculator _taxCalculator; // Interface, not concrete type

    public TransactionService(ITaxCalculator taxCalculator)
    {
        _taxCalculator = taxCalculator;
    }

    public Money CalculateNet(Transaction tx)
    {
        var tax = _taxCalculator.CalculateTax(tx.GrossAmount, tx.Category);
        return tx.GrossAmount - tax;
    }
}

// ITaxCalculator defined in SharedKernel or Financial module's own contracts
```

### 6.2 Python Examples

#### Example 1: Layer Isolation — Framework Types in Domain

```python
# ❌ BAD: Domain model imports Flask and SQLAlchemy
from flask import request
from sqlalchemy import Column, String
from sqlalchemy.ext.declarative import declarative_base

Base = declarative_base()

class User(Base):
    __tablename__ = "users"
    id = Column(String, primary_key=True)
    email = Column(String)

    def validate_request(self):
        return request.json.get("email") == self.email  # Framework coupling!
```

```python
# ✅ GOOD: Domain model is pure Python
from dataclasses import dataclass
from typing import Optional

@dataclass(frozen=True)
class Email:
    value: str

    def __post_init__(self):
        if "@" not in self.value:
            raise ValueError(f"Invalid email: {self.value}")

@dataclass
class User:
    id: str
    email: Email

    def matches_email(self, email: str) -> bool:
        return self.email.value == email

# SQLAlchemy mapping lives in infrastructure layer:
# infrastructure/persistence/user_orm.py
```

#### Example 2: Circular Import Resolution

```python
# ❌ BAD: Circular imports between modules
# module_a.py
from module_b import process_b_result

def process_a():
    result = process_b_result()  # Circular dependency
    return f"A({result})"

# module_b.py
from module_a import process_a

def process_b_result():
    return process_a()  # ImportError or AttributeError at runtime
```

```python
# ✅ GOOD: Extract shared contract to separate module
# contracts.py
from typing import Protocol

class ResultProcessor(Protocol):
    def process(self) -> str: ...

# module_a.py
from contracts import ResultProcessor

def process_a(processor: ResultProcessor) -> str:
    result = processor.process()
    return f"A({result})"

# module_b.py
class BResultProcessor:
    def process(self) -> str:
        return "B-result"

# Usage:
# a_result = process_a(BResultProcessor())
```

#### Example 3: DTO Boundary — API Models in Domain

```python
# ❌ BAD: Domain function accepts API response model
from api.schemas import UserResponse  # Presentation type in domain!

def calculate_discount(user: UserResponse) -> float:
    return 0.1 if user.metadata.tier == "premium" else 0.0
```

```python
# ✅ GOOD: Domain uses its own types; mapping at boundary
from dataclasses import dataclass

@dataclass(frozen=True)
class CustomerTier:
    value: str  # "premium", "standard", "basic"

@dataclass(frozen=True)
class Customer:
    id: str
    tier: CustomerTier

def calculate_discount(customer: Customer) -> float:
    return 0.1 if customer.tier.value == "premium" else 0.0

# API layer maps: Customer(id=api_user.id, tier=CustomerTier(api_user.metadata.tier))
```

### 6.3 JavaScript/TypeScript Examples

#### Example 1: Module Boundary — Deep Cross-Module Import

```typescript
// ❌ BAD: Deep import across module boundary
import { TaxCalculator } from '../../tax/internal/services/calculator';
import { UserSettings } from '../../../settings/domain/models/user-settings';
```

```typescript
// ✅ GOOD: Import from barrel export / public API
import { TaxCalculator } from '@app/tax';
import { UserSettings } from '@app/settings';

// Each module's index.ts explicitly exports public API:
// @app/tax/index.ts
// export { TaxCalculator } from './services/calculator';
// Do NOT export internal utilities
```

#### Example 2: God Component Refactoring

```typescript
// ❌ BAD: God component — 300+ LOC with data fetching, logic, and UI
function UserDashboard() {
  const [users, setUsers] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  useEffect(() => {
    fetch('/api/users')
      .then(res => res.json())
      .then(data => {
        const processed = data.map(u => ({
          ...u,
          displayName: `${u.firstName} ${u.lastName}`,
          isActive: u.lastLogin > Date.now() - 30 * 24 * 60 * 60 * 1000
        }));
        setUsers(processed);
        setLoading(false);
      })
      .catch(err => { setError(err.message); setLoading(false); });
  }, []);

  // ... 200 more lines of JSX
}
```

```typescript
// ✅ GOOD: Separated concerns

// hooks/useUsers.ts — Data fetching logic
function useUsers() {
  const { data, error, isLoading } = useQuery({
    queryKey: ['users'],
    queryFn: () => userService.fetchUsers(),
  });
  return { users: data ?? [], error, isLoading };
}

// services/userService.ts — API layer
const userService = {
  async fetchUsers(): Promise<User[]> {
    const response = await apiClient.get<User[]>('/users');
    return response.data.map(toUser);
  }
};

// components/UserDashboard.tsx — Presentation only
function UserDashboard() {
  const { users, error, isLoading } = useUsers();
  if (isLoading) return <LoadingSpinner />;
  if (error) return <ErrorMessage error={error} />;
  return <UserTable users={users} />;
}
```

#### Example 3: Dependency Direction — Data Layer Importing UI

```typescript
// ❌ BAD: Data layer imports UI component
// data/formatters.ts
import { UserBadge } from '../ui/components/UserBadge';

export function formatUserStatus(user: User) {
  return UserBadge.getStatusColor(user.tier); // Data depends on UI!
}
```

```typescript
// ✅ GOOD: Data layer is pure; UI imports from data
// data/formatters.ts
export function getUserStatusColor(tier: string): string {
  const colors: Record<string, string> = {
    premium: 'gold',
    standard: 'blue',
    basic: 'gray',
  };
  return colors[tier] ?? 'gray';
}

// ui/components/UserBadge.tsx
import { getUserStatusColor } from '@app/data/formatters';

export function UserBadge({ tier }: { tier: string }) {
  const color = getUserStatusColor(tier);
  return <span style={{ color }}>{tier}</span>;
}
```

---

## Section 7: Output Format

All audit findings MUST be emitted as structured JSON objects. One finding per object.

### 7.1 Finding Schema

```json
{
  "severity": "critical|high|medium|low",
  "file": "relative/path/to/file",
  "line": 42,
  "endLine": 45,
  "description": "What was found and why it matters. Be specific — include the violated pattern.",
  "recommendation": "Specific fix with brief code example if applicable. Reference a pattern or doc link.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|CrossLanguage",
  "category": "architecture"
}
```

### 7.2 Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Section 8 for escalation rules.
- **file**: Relative path from project root. Use forward slashes. Never absolute paths.
- **line**: 1-based line number where the finding starts.
- **endLine**: Optional. Line where the finding ends (for multi-line issues).
- **description**: Factual, specific. Include the problematic code pattern. No hedging ("might be a problem").
- **recommendation**: Actionable. Include a code snippet showing the fix when possible.
- **technology**: Primary technology of the finding. Use `CrossLanguage` only for patterns that span multiple files.
- **category**: Always `architecture` for this squad.

### 7.3 Summary Report

After all findings, emit a summary:

```json
{
  "summary": {
    "squad": "audit-architecture",
    "timestamp": "2026-01-15T10:30:00Z",
    "project": "detected-project-name",
    "technologies": [".NET/C#", "TypeScript"],
    "totalFindings": 12,
    "bySeverity": {
      "critical": 0,
      "high": 3,
      "medium": 7,
      "low": 2
    },
    "byCategory": {
      "architecture": 12
    },
    "filesScanned": 85,
    "filesWithFindings": 8
  }
}
```

---

## Section 8: Severity Guidelines

### 8.1 Architecture-Specific Severity Definitions

| Level | Definition | Example |
|-------|-----------|---------|
| **critical** | Fundamental structural violation that makes the system unmaintainable or untestable | Circular dependencies between modules, domain layer importing infrastructure framework types |
| **high** | Significant architectural violation affecting multiple components | Layer violations, DTO leaks across >2 layers, god modules, cross-module direct entity access |
| **medium** | Minor coupling issue or suboptimal structure | Missing module registration, scattered configuration, minor import structure issues |
| **low** | Style-level architectural suggestion | Naming inconsistency, suboptimal directory organization, missing barrel exports |

### 8.2 Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Violation is in domain/core layer | +1 level | All technologies |
| Violation affects multiple modules | +1 level | Multi-module systems |
| Violation creates circular dependency | +1 level | All technologies |
| Violation blocks testing | +1 level | All technologies |
| Violation is in shared/common code | +1 level | All technologies |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Finding is in non-production/test code only | -1 level (min low) | All technologies |
| Finding has existing plan to refactor (documented) | -1 level (min low) | All technologies |
| Finding is in deprecated code scheduled for removal | -1 level (min low) | All technologies |
| Finding is in prototype/spike directory | -1 level (min low) | All technologies |

### 8.4 Technology-Specific Severity Adjustments

| Technology | Pattern | Base Severity | Notes |
|-----------|---------|---------------|-------|
| **.NET/C#** | Domain importing EF types | critical | Layer violation, makes domain untestable |
| **.NET/C#** | Handler using DbContext directly | high | Bypasses repository abstraction |
| **.NET/C#** | Cross-module direct entity access | high | Tight coupling between modules |
| **.NET/C#** | Missing module registration | medium | DI resolution fails at runtime |
| **Python** | Domain importing Flask/SQLAlchemy types | critical | Framework coupling in domain layer |
| **Python** | Circular imports between modules | critical | Import errors at runtime |
| **Python** | God module >500 LOC | medium | Violates SRP |
| **JavaScript** | Deep cross-module imports | high | Fragile coupling to internals |
| **JavaScript** | God component >300 LOC | medium | Hard to test and maintain |
| **JavaScript** | Missing barrel exports | low | Internal details exposed |

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
audit-cicd ─────────────────────────┤
audit-license ──────────────────────┤
audit-consolidated ◄────────────────┘
audit-consistency-coherence
```

### 9.2 Upstream Dependencies

- **audit-code-quality**: May reference code quality metrics (complexity, coupling) that inform architectural analysis.
- **audit-dependencies**: Package reference graph informs module coupling analysis.

### 9.3 Downstream Consumers

- **audit-consolidated**: Aggregates all squad findings into unified report
- **audit-consistency-coherence**: Validates cross-domain alignment between squad findings

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-architecture | audit-code-quality | Architecture focuses on layer/module structure; code-quality focuses on implementation smells |
| audit-architecture | audit-refactoring | Architecture identifies structural problems; refactoring recommends modernization approaches |
| audit-architecture | audit-security | Architecture may flag auth middleware placement; security audits the actual vulnerability |

### 9.5 Recommended Execution Order

Architecture squad runs at position 3 in full audit sequence:

1. **audit-dependencies** — Establish baseline of third-party risk
2. **audit-security** — Code-level vulnerability scan
3. **audit-architecture** ← This squad — Structural analysis
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
