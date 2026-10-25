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

- Layer isolation — domain must not import infrastructure/presentation types
- Coupling analysis — circular deps, tight coupling, inappropriate intimacy
- Dependency direction — flow inward (infrastructure → domain ← presentation)
- Module boundary validation — clear responsibility, no god modules, stable interfaces
- DTO leak detection — DTOs bleeding into inappropriate layers
- Project structure — naming conventions, folder organization, project references
- CQRS compliance — command/query handler separation
- Clean Architecture validation — concentric layer boundaries, no outward deps from domain
- Package dependency graph — no cycles, reasonable depth, clear interfaces
- Interface segregation — consumer-defined interfaces, small focused contracts
- **.NET/C#**: Clean Architecture layers, csproj reference graph, SharedKernel boundaries, CQRS separation, middleware ordering
- **Python**: Import graph, circular import detection, layer boundaries in Django/FastAPI/Flask
- **JavaScript/TypeScript**: Monorepo boundaries, barrel export integrity, path aliases, component architecture
- **Go**: Internal package boundaries, consumer-defined interfaces, init() audit
- **Java**: ArchUnit layer rules, Maven/Gradle multi-module coupling, Spring bean scope validation
- **Rust**: Crate dependency direction, module visibility, feature flag architecture

### DO NOT USE FOR

- Code style/naming (use `audit-code-quality`)
- Refactoring suggestions (use `audit-refactoring`)
- Security vulnerabilities (use `audit-security`)
- Test coverage (use `audit-test-coverage`)
- Performance (use `audit-code-quality` for N+1)

---

## Section 3: Target Discovery

Agent MUST auto-detect project structure. Do NOT hardcode repository names.

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

1. **Scan root** for project/solution files → understand module structure.
2. **Identify language** by source file count.
3. **Map references** — `*.csproj` `<ProjectReference>`, `go.mod` imports, `package.json` deps, `pom.xml` modules.
4. **Identify layers** by directory analysis (domain, application, infrastructure, presentation).
5. **Detect pattern**: Clean Architecture, Hexagonal, CQRS, MVC, layered.
6. **Build dep graph** — circular deps and violation edges.
7. **Select checklist** from Section 4.
8. **Multiple languages** → run all applicable checklists.

---

## Section 4: Technology-Specific Checklists

### 4.1 .NET/C# Architecture Checklist

- [ ] **Clean Architecture layer isolation**: Domain must NOT reference infrastructure, presentation, or framework projects. Verify `*.csproj` has zero framework refs beyond `SharedKernel`.
- [ ] **CQRS handler boundaries**: Commands must not call query methods and vice versa. One command = one handler.
- [ ] **Domain entity purity**: Domain entities must NOT import EF Core (`DbContext`, `DbSet`, `IEntityTypeConfiguration`), MediatR, Wolverine, or any infrastructure namespace.
- [ ] **Module coupling via csproj**: Build reference graph. No module depending on 5+ domain modules. Cross-module via events/interfaces only.
- [ ] **God classes**: Flag >300 LOC handling multiple concerns. Split into focused types.
- [ ] **Dead code**: Unused params, private methods, internal classes. Detect via Roslyn (IDE0051, IDE0052).
- [ ] **DTO leak across layers**: DTOs in infrastructure/presentation must not appear in domain signatures. Domain types only; mapping at boundary.
- [ ] **Infrastructure referencing domain**: Infrastructure may reference domain (correct). Domain must NOT reference infrastructure.
- [ ] **Reporting module coupling**: If reporting exists, verify read model or projection — not querying 7+ domain databases.
- [ ] **Module registration isolation**: Each `*ModuleRegistration.cs` registers only its own services. Cross-module via shared interfaces.
- [ ] **SharedKernel boundaries**: Only shared primitives (Entity, ValueObject, Result). No domain logic or infrastructure types.
- [ ] **Handler directory organization**: Commands, queries, events in separate dirs (`Commands/`, `Queries/`, `Events/`, `Handlers/`).
- [ ] **No static mutable state**: No `static` mutable fields in services. All state via DI.
- [ ] **Middleware ordering**: Auth → Authorization → Exception Handling → Custom. Verify `Program.cs` order.

### 4.2 Python Architecture Checklist

- [ ] **Package layer isolation**: `domain/` must not import `infrastructure/`, `api/`, `presentation/`. Verify via `import` graph.
- [ ] **Circular import detection**: Identify circular imports → subtle bugs and coupling.
- [ ] **Module coupling**: No module importing from >3-4 siblings. High fan-in/fan-out = god module.
- [ ] **God modules**: Flag `*.py` >500 LOC with mixed concerns.
- [ ] **Interface segregation**: ABCs with 3-5 methods max. Split large ones into focused protocols.
- [ ] **Dependency injection**: Services injected, not imported/instantiated directly. Check for `Service()` calls inside other services.
- [ ] **DTO/model leak**: Pydantic/dataclasses for API responses must not appear in domain logic. Domain models internally, convert at boundary.
- [ ] **Configuration separation**: Config in dedicated module. Use `pydantic-settings` or similar.
- [ ] **Entry point clarity**: Single entry (`main.py`, `app.py`). No business logic there.
- [ ] **Test mirror structure**: Test dir mirrors source. `tests/domain/test_entity.py` for `src/domain/entity.py`.

### 4.3 JavaScript/TypeScript Architecture Checklist

- [ ] **Module boundary enforcement**: Path aliases or package boundaries. No deep cross-module imports (`../../../other-module/internal`).
- [ ] **Dependency direction**: `ui/` → `features/` → `domain/` → `data/`. No reverse.
- [ ] **Barrel exports**: Each module has `index.ts`. Internal files not imported outside.
- [ ] **God components**: React/Vue >300 LOC with data+logic+UI. Extract into hooks, services, presentational components.
- [ ] **Type sharing**: Shared types in `types/` or `@shared/`. No circular type imports.
- [ ] **API layer isolation**: API calls in service layer, not components/hooks. Returns domain types, not raw responses.
- [ ] **State management**: Global in store (Redux/Zustand/Context), local in components. No prop drilling beyond 2 levels.
- [ ] **Dead exports**: Exported but never imported. Use `ts-prune`.
- [ ] **Package structure**: Monorepo with clear boundaries. `workspace:*` for internal deps.

### 4.4 Go Architecture Checklist

- [ ] **Package dep direction**: `cmd/` → `internal/` → `pkg/`. No reverse. `pkg/` must not import `internal/`.
- [ ] **Circular import detection**: Compiler catches these, but verify no near-circular deps via interfaces.
- [ ] **Interface placement**: Consumer-defined. `consumer/pkg/interface.go` defines what it needs.
- [ ] **God packages**: >20 exported types or >1000 LOC → split.
- [ ] **Internal package usage**: Implementation in `internal/`. Public API in `pkg/` or root.
- [ ] **Dependency injection**: Deps as interfaces in constructors, not globals or `init()`.

### 4.5 Java Architecture Checklist

- [ ] **Layer isolation**: `domain/` must not import `infrastructure/`, `web/`, framework types. Verify with ArchUnit.
- [ ] **Module coupling**: Maven/Gradle multi-module. No circular deps. `mvn detect` or `dependency:tree`.
- [ ] **God classes**: >500 LOC with >10 public methods → split via extraction.
- [ ] **DTO boundaries**: DTOs in `api/` or `web/`. Domain uses entities/VOs. MapStruct at boundaries.
- [ ] **Spring bean scoping**: Verify scopes intentional. No request-scoped in singletons without proxy.
- [ ] **Package naming**: Reverse-domain. Feature-based or layer-based consistently across modules.

---

## Section 5: Anti-Patterns

### 5.1 Cross-Language Architecture Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Circular dependencies | critical | A imports B imports A | Tight coupling, build order issues, untestable in isolation | Extract to C; use events/interfaces |
| 2 | Domain importing framework types | critical | Domain refs ORM, DI, web framework | Domain coupled to infra; untestable/unreusable | Domain defines interfaces; infra implements |
| 3 | God class/module | high | >10 responsibilities or >300 LOC multi-concern | SRP violation, hard to test/maintain, merge conflicts | Split into focused classes/modules |
| 4 | DTO leak across layers | high | API DTOs in domain/business layer | Domain depends on presentation format changes | Domain types; mapping at boundaries |
| 5 | Feature envy | medium | Method in A uses more data from B than its own | Logic in wrong module, hidden coupling | Move to B or use proper interface |
| 6 | Scattered configuration | medium | Config loading spread across files | Hard to understand, inconsistent defaults | Centralized config with typed models |

### 5.2 .NET/C# Architecture Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Domain importing EF types | critical | `using Microsoft.EntityFrameworkCore` in domain | Domain coupled to ORM; untestable | Repo interface in domain; EF in infra |
| 2 | Handlers using DbContext | high | Handlers inject `DbContext` directly | Coupled to persistence; no test abstraction | Handlers use repo interface |
| 3 | Infra depending on domain module | medium | Infra references specific domain module | Tight infra↔domain coupling | Infra depends on domain abstractions |
| 4 | Cross-module direct entity access | high | Financial imports Tax entity types | Tight coupling; changes cascade | Integration events or shared interfaces |
| 5 | SharedKernel with domain logic | medium | SharedKernel has module-specific business rules | Becomes dumping ground | Only primitives (Entity, ValueObject, Result) |
| 6 | Missing module registration | medium | New module without `*ModuleRegistration.cs` | DI fails at runtime | Every module has registration in Program.cs |

### 5.3 Python Architecture Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Circular imports | critical | A imports B, B imports A at top level | Import errors, AttributeError, tight coupling | Extract to third module; lazy imports |
| 2 | God module | high | .py >500 LOC mixed concerns | Hard to test/understand/maintain | Split into focused modules (SRP) |
| 3 | Framework types in domain | critical | Domain imports Flask/SQLAlchemy | Domain coupled to framework | Pure Python types; framework at boundaries |
| 4 | Scattered service instantiation | medium | `service = ExternalService()` everywhere | Hard to swap, no centralized config | DI via params or container |
| 5 | Missing __init__.py boundaries | low | __init__.py exposes all internals | Internal = public API | Explicit __all__ or minimal exports |

### 5.4 JavaScript/TypeScript Architecture Anti-Patterns

| # | Anti-Pattern | Severity | Description | Why Harmful | Correct Pattern |
|---|-------------|----------|-------------|-------------|-----------------|
| 1 | Deep relative imports | high | `../../../other-module/src/internal` | Fragile; breaks on refactor | Path aliases; barrel exports |
| 2 | God components | high | React/Vue >300 LOC: data+logic+UI | Hard to test/reuse/understand | Hooks for logic, services for API, presentational for UI |
| 3 | Global mutable state | high | Module-level vars mutated at runtime | Unpredictable, hard to test, concurrency issues | State library or explicit passing |
| 4 | Type coupling | medium | Internal types from one module in another | Changes propagate; no encapsulation | Interface types for cross-module contracts |
| 5 | Circular dependencies | critical | A imports B imports A | Runtime errors, undefined, bundle bloat | Extract shared; dependency inversion |

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

All findings as structured JSON. One per object.

### 7.1 Finding Schema

```json
{
  "severity": "critical|high|medium|low",
  "file": "relative/path/to/file",
  "line": 42,
  "endLine": 45,
  "description": "What was found and why. Be specific — include the violated pattern.",
  "recommendation": "Specific fix with code example. Reference pattern or doc link.",
  "technology": ".NET|Python|JavaScript|TypeScript|Go|Java|CrossLanguage",
  "category": "architecture"
}
```

### 7.2 Field Rules

- **severity**: Exactly one of `critical`, `high`, `medium`, `low`. See Section 8.
- **file**: Relative path from root. Forward slashes. Never absolute.
- **line**: 1-based start line.
- **endLine**: Optional end line for multi-line issues.
- **description**: Factual, specific. Include problematic pattern. No hedging.
- **recommendation**: Actionable. Code snippet for fix when possible.
- **technology**: Primary technology. `CrossLanguage` only for multi-file patterns.
- **category**: Always `architecture`.

### 7.3 Summary Report

After all findings, emit:

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
| **critical** | Structural violation — system unmaintainable/untestable | Circular deps, domain importing infra framework types |
| **high** | Significant violation affecting multiple components | Layer violations, DTO leaks across >2 layers, god modules, cross-module direct access |
| **medium** | Minor coupling or suboptimal structure | Missing module registration, scattered config, minor import issues |
| **low** | Style-level architectural suggestion | Naming inconsistency, suboptimal dir org, missing barrel exports |

### 8.2 Severity Escalation Triggers

| Trigger | Escalation | Applies To |
|---------|-----------|------------|
| Domain/core layer | +1 level | All technologies |
| Multiple modules affected | +1 level | Multi-module systems |
| Creates circular dependency | +1 level | All technologies |
| Blocks testing | +1 level | All technologies |
| Shared/common code | +1 level | All technologies |

### 8.3 Severity De-escalation Triggers

| Trigger | De-escalation | Applies To |
|---------|---------------|------------|
| Non-production/test only | -1 level (min low) | All technologies |
| Existing refactor plan (documented) | -1 level (min low) | All technologies |
| Deprecated code for removal | -1 level (min low) | All technologies |
| Prototype/spike directory | -1 level (min low) | All technologies |

### 8.4 Technology-Specific Severity Adjustments

| Technology | Pattern | Base Severity | Notes |
|-----------|---------|---------------|-------|
| **.NET/C#** | Domain importing EF types | critical | Layer violation, untestable |
| **.NET/C#** | Handler using DbContext directly | high | Bypasses repo abstraction |
| **.NET/C#** | Cross-module direct entity access | high | Tight coupling |
| **.NET/C#** | Missing module registration | medium | DI fails at runtime |
| **Python** | Domain importing Flask/SQLAlchemy | critical | Framework in domain |
| **Python** | Circular imports | critical | Runtime import errors |
| **Python** | God module >500 LOC | medium | SRP violation |
| **JavaScript** | Deep cross-module imports | high | Fragile coupling |
| **JavaScript** | God component >300 LOC | medium | Hard to test/maintain |
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

- **audit-code-quality**: Quality metrics (complexity, coupling) inform architectural analysis.
- **audit-dependencies**: Package reference graph informs module coupling.

### 9.3 Downstream Consumers

- **audit-consolidated**: Aggregates all findings into unified report
- **audit-consistency-coherence**: Validates cross-domain alignment

### 9.4 Overlapping Squads

| This Squad | Overlaps With | Boundary |
|-----------|---------------|----------|
| audit-architecture | audit-code-quality | Architecture: layer/module structure; code-quality: implementation smells |
| audit-architecture | audit-refactoring | Architecture: structural problems; refactoring: modernization approaches |
| audit-architecture | audit-security | Architecture: middleware placement; security: actual vulnerability |

### 9.5 Recommended Execution Order

Architecture runs at position 3:

1. **audit-dependencies** — Third-party risk baseline
2. **audit-security** — Vulnerability scan
3. **audit-architecture** ← This squad
4. **audit-code-quality** — Implementation quality
5. **audit-test-coverage** — Testing assessment
6. **audit-fault-tolerance** — Resilience review
7. **audit-observability** — Telemetry validation
8. **audit-privacy-data** — PII/data handling
9. **audit-cicd** — Pipeline security
10. **audit-license** — License compliance
11. **Domain-specific squads** (financial-model, tax-fiscal, regulatory-*, etc.)
12. **audit-consolidated** — Aggregate findings
13. **audit-consistency-coherence** — Cross-domain validation

### 9.6 Output Integration

When running sequentially, each squad SHOULD:
1. Read previous findings from shared output location.
2. Deduplicate: skip already-reported findings.
3. Cross-reference: escalate confirmed by multiple squads.
4. Output in Section 7 format for aggregation.
