---
name: dotnet-clean-arch
description: ".NET Clean Architecture patterns: CQRS via Wolverine, EF Core, Autofac DI, testing pyramid (unit/integration/E2E), minimal APIs, SignalR, configuration, encryption at rest, multi-tenancy, external service resilience, Keycloak OIDC, migration management. USE FOR: Clean Architecture project setup, CQRS handlers, EF Core patterns, DI registration, test structure, minimal API endpoints, background jobs, encryption, multi-tenancy, Polly resilience, Keycloak auth, database migrations. DO NOT USE FOR: frontend-only work, non-.NET projects, infrastructure deployment."
license: MIT
metadata:
  author: community
  version: "2.0.0"
  frameworks: [".NET 8+", "Wolverine", "EF Core", "Blazor"]
---

# .NET Clean Architecture Skill

## Project Structure

```text
src/
├── {Project}.Domain/          # Entities, ValueObjects, Enums, domain rules
├── {Project}.Application/     # CQRS handlers, service interfaces, DTOs
├── {Project}.Infrastructure/  # EF Core, repos, external integrations, jobs
├── {Project}.Server/          # Blazor Server host OR ASP.NET Core host
├── {Project}.Shared/          # Shared UI components (if Blazor)
└── {Project}.PWA/             # Standalone WASM client (if applicable)
tests/
├── {Project}.Tests.Unit/      # xUnit + mocking
├── {Project}.Tests.Integration/ # Testcontainers
└── {Project}.Tests.E2E/       # Playwright
```

Entity base class: `Guid Id`, `DateTime CreatedAt/UpdatedAt`, `bool IsDeleted`. Value objects as `record` types in `Domain/ValueObjects/`.

**Entity configuration file placement**: Entity configuration files are placed at the module root level (e.g., `DocumentEntityConfigurations.cs` at the module root), NOT in an `EntityConfigurations/` subdirectory. This follows the feature-folder convention where all files related to a module are co-located.

### Domain Entity Business Rules

**Effective deadline fallback pattern**: For entities with optional extension deadlines (e.g., tax obligations), use `ExtensionDeadline ?? FilingDeadline` as the effective due date. Using only `FilingDeadline` causes extended filings to be incorrectly marked overdue:

```csharp
// WRONG — ignores extension
var isOverdue = DateTime.UtcNow > entity.FilingDeadline;

// CORRECT — use extension as effective deadline when present
var effectiveDeadline = entity.ExtensionDeadline ?? entity.FilingDeadline;
var isOverdue = DateTime.UtcNow > effectiveDeadline;
```

## CQRS with Wolverine

- Feature folders with `Commands/` and `Queries/` subfolders
- Separate `*CommandHandler` / `*QueryHandler` file per command/query
- Wolverine handlers are STATIC classes. Use `ILogger<CommandType>` as method parameter, not `ILogger<HandlerClass>` (CS0718 error with static types)
- Events in `Events/` subfolder
- FluentValidation is centralized via Wolverine middleware: `ValidationMiddleware<TRequest>` runs ALL `IValidator<TRequest>` before any handler
- Middleware takes `IEnumerable<IValidator<TRequest>>` via DI, runs them with `Task.WhenAll`, aggregates failures into `ValidationException`
- No explicit `AddValidatorsFromAssembly` needed — DI resolves validators into middleware automatically

### Tenant-Aware Command Pattern

All tenant-aware commands implement a marker interface so Wolverine middleware can resolve the tenant context before the handler executes. This keeps tenant filtering explicit and auditable at the command level.

```csharp
// SharedKernel - Marker interface for tenant-aware commands
public interface IWolverineTenantCommand
{
    Guid TenantId { get; }
}

// Module - Example command implementing the marker
public sealed record CreateCustomerCommand(
    Guid TenantId,
    string Name,
    string Email) : IWolverineTenantCommand;
```

Every command that operates on tenant-scoped data carries `TenantId` as a required field. Wolverine middleware reads this value to set the multi-tenant context before the handler runs, ensuring EF Core queries, repository calls, and downstream services all see the correct tenant.

### Wolverine Middleware Pipeline

Wolverine supports a declarative middleware pipeline that runs before (and after) handlers. Register middleware in the Wolverine configuration — they apply to all matching message types.

**ValidationMiddleware** — runs all FluentValidation validators before the handler executes:

```csharp
public class ValidationMiddleware<TRequest>
{
    private readonly IEnumerable<IValidator<TRequest>> _validators;

    public ValidationMiddleware(IEnumerable<IValidator<TRequest>> validators)
    {
        _validators = validators;
    }

    public async Task BeforeHandleAsync(TRequest message, CancellationToken ct)
    {
        if (!_validators.Any()) return;

        var context = new ValidationContext<TRequest>(message);
        var results = await Task.WhenAll(
            _validators.Select(v => v.ValidateAsync(context, ct)));
        var failures = results.SelectMany(r => r.Errors)
            .Where(f => f is not null).ToList();

        if (failures.Count != 0)
            throw new ValidationException(failures);
    }
}
```

**TenantMiddleware** — resolves tenant ID from multi-tenant context into `ITenantAware` commands:

```csharp
public class TenantMiddleware<TRequest>
{
    private readonly IMultiTenantContextAccessor _tenantAccessor;

    public TenantMiddleware(IMultiTenantContextAccessor tenantAccessor)
    {
        _tenantAccessor = tenantAccessor;
    }

    public async Task BeforeHandleAsync(TRequest message, CancellationToken ct)
    {
        if (message is ITenantAware tenantAware)
        {
            var tenantId = _tenantAccessor.MultiTenantContext?.TenantInfo?.Id;
            if (tenantId is not null)
                tenantAware.TenantId = tenantId;
        }
    }
}
```

**AuditingMiddleware** — sets audit user from `ClaimsPrincipal` on `IAuditableCommand`:

```csharp
public class AuditingMiddleware<TRequest>
{
    private readonly IHttpContextAccessor _httpContextAccessor;

    public AuditingMiddleware(IHttpContextAccessor httpContextAccessor)
    {
        _httpContextAccessor = httpContextAccessor;
    }

    public async Task BeforeHandleAsync(TRequest message, CancellationToken ct)
    {
        if (message is IAuditableCommand auditable)
        {
            var user = _httpContextAccessor.HttpContext?.User;
            var userId = user?.FindFirst(ClaimTypes.NameIdentifier)?.Value
                ?? user?.FindFirst("sub")?.Value
                ?? "system";
            auditable.SetAuditUser(userId);
        }
    }
}
```

**LifecycleMiddleware** — calls lifecycle hooks (`BeforeValidateAsync`, `BeforeSubmitAsync`, `OnSubmitAsync`, `OnCancelAsync`) on commands that implement `ILifecycleHook`:

```csharp
public class LifecycleMiddleware<TRequest>
{
    public async Task BeforeHandleAsync(TRequest message, CancellationToken ct)
    {
        if (message is ILifecycleHook hook)
            await hook.BeforeValidateAsync(ct);
    }

    public async Task AfterHandleAsync(TRequest message, CancellationToken ct)
    {
        if (message is ILifecycleHook hook)
            await hook.OnSubmitAsync(ct);
    }
}
```

**Registration** — add middleware to Wolverine's pipeline in `Program.cs`:

```csharp
builder.Host.UseWolverine(opts =>
{
    opts.Policies.ForMessagesOfType<ICommand>().AddMiddleware(typeof(ValidationMiddleware<>));
    opts.Policies.ForMessagesOfType<ITenantAware>().AddMiddleware(typeof(TenantMiddleware<>));
    opts.Policies.ForMessagesOfType<IAuditableCommand>().AddMiddleware(typeof(AuditingMiddleware<>));
});
```

### Wolverine Tenant Resolution Middleware

This middleware runs before any handler processing a tenant-aware command. It resolves the tenant from the command payload, falls back to the Wolverine envelope header if needed, and sets the Finbuckle multi-tenant context so downstream EF Core queries and services automatically filter by tenant.

```csharp
public class TenantResolutionMiddleware
{
    public static void Before(IMessageContext context, IWolverineTenantCommand command)
    {
        // Resolve tenant from the command, fall back to envelope header
        var tenantId = command.TenantId;
        if (tenantId == Guid.Empty)
        {
            var envelopeTenant = context.Envelope?.TenantId;
            if (!string.IsNullOrEmpty(envelopeTenant))
                tenantId = Guid.Parse(envelopeTenant);
        }

        // Set the multi-tenant context for downstream handlers
        var accessor = context.ServiceProvider.GetRequiredService<IMultiTenantContextAccessor>();
        var store = accessor.Store
            ?? throw new InvalidOperationException("Multi-tenant store is not configured.");

        var tenantInfo = store.TryGetAsync(tenantId.ToString()).GetAwaiter().GetResult();
        if (tenantInfo is null)
            throw new InvalidOperationException($"Tenant '{tenantId}' not found in store.");

        // Finbuckle reads the resolved tenant from the context accessor
        accessor.Store = store;
    }
}
```

**Registration in `Program.cs`**:

```csharp
builder.Host.UseWolverine(opts =>
{
    opts.Policies.AddMiddleware<TenantResolutionMiddleware>();
    opts.Policies.AddMiddleware<TenantAuthorizationMiddleware>();

    // Scan all module assemblies for handlers
    opts.Discovery.IncludeAssembly(typeof(CustomerModuleRegistration).Assembly);
    opts.Discovery.IncludeAssembly(typeof(OrderModuleRegistration).Assembly);
    // ... include all module assemblies
});
```

### Wolverine Tenant Authorization Middleware

Runs after tenant resolution to verify the current user actually belongs to the tenant on the command. System admins bypass this check.

```csharp
public class TenantAuthorizationMiddleware
{
    public static void Before(IMessageContext context, IWolverineTenantCommand command)
    {
        var currentUser = context.ServiceProvider.GetRequiredService<ICurrentUser>();
        if (currentUser.Roles.Contains("system-admin"))
            return; // System admins bypass tenant checks

        // Verify the current user has access to the tenant on the command
        if (currentUser.TenantId != command.TenantId)
            throw new UnauthorizedAccessException(
                $"User '{currentUser.UserId}' cannot access tenant '{command.TenantId}'");
    }
}
```

The authorization middleware rejects cross-tenant access attempts at the Wolverine pipeline level, before any handler or database call runs. This is a defense-in-depth measure that complements the explicit `.Where(e => e.CompanyId == companyId)` filtering in handlers.

## EF Core Patterns

- ALWAYS use `IDbContextFactory<AppDbContext>`. Never scoped DbContext
- Design-time factory at `Infrastructure/DesignTime/AppDbContextFactory.cs` for `dotnet ef` CLI
- Audit interceptor: converts deletes to soft-deletes, creates AuditLog entries
- Repositories named `*Repository`, auto-registered by suffix convention

### Avoid Multi-Source `.Concat()` Queries

EF Core's `.Concat()` chains across 3+ different entity types may fail to translate with the Npgsql PostgreSQL provider. The query planner produces translation errors or runtime exceptions for complex multi-entity Concat operations (e.g., building a unified audit trail from 8 entity types).

**Pattern**: Instead of chaining `.Concat()` from multiple `IQueryable` sources, collect resource IDs from each source first, then execute a single query:

```csharp
// WRONG: Multi-source Concat — may fail with Npgsql
var auditTrail = dbContext.Patients
    .Where(p => p.Id == resourceId)
    .Select(p => new AuditEntry { ... })
    .Concat(dbContext.Prontuarios.Where(p => p.Id == resourceId).Select(p => new AuditEntry { ... }))
    .Concat(dbContext.Sessions.Where(s => s.Id == resourceId).Select(s => new AuditEntry { ... }))
    // ... 5 more Concat calls
    .OrderByDescending(e => e.Timestamp)
    .ToList();

// CORRECT: Collect IDs, single query per type, union in memory
var patientIds = await dbContext.Patients
    .Where(p => p.PatientId == patientId)
    .Select(p => p.Id).ToListAsyncAsync();
var prontuarioIds = await dbContext.Prontuarios
    .Where(p => p.PatientId == patientId)
    .Select(p => p.Id).ToListAsyncAsync();
var sessionIds = await dbContext.Sessions
    .Where(s => s.Appointment.PatientId == patientId)
    .Select(s => s.Id).ToListAsyncAsync();
// ... collect remaining type IDs

var allIds = patientIds.Concat(prontuarioIds).Concat(sessionIds).ToList();

// Single query with IN filter
var entries = await dbContext.Set<AuditLog>()
    .Where(a => allIds.Contains(a.EntityId))
    .OrderByDescending(a => a.Timestamp)
    .ToListAsyncAsync();
```

This approach keeps each query simple (single entity type), avoids Concat translation failures, and is often faster because each sub-query can use its own optimal index.

### DbContext.GetService&lt;T&gt;() Workaround

`context.Database.GetService&lt;T&gt;()` or `context.GetService&lt;T&gt;()` doesn't always resolve services directly from within EF interceptors or save hooks. Cast to `IServiceProvider` first:

```csharp
var service = ((IServiceProvider)context).GetService&lt;T&gt;();
```

## Dependency Injection

- Autofac via `UseServiceProviderFactory(new AutofacServiceProviderFactory())`
- Repos auto-registered by `*Repository` suffix in InfrastructureModule
- Services registered explicitly in InfrastructureModule
- HttpClient-based services: `AddHttpClient<T>()` in Program.cs, register interface in InfrastructureModule
- External services: Refit interface + `*ResiliencePolicy.cs` + `*ServiceExtensions.cs`

## Testing Pyramid

**Unit** (xUnit + Shouldly + Moq + Bogus):
- `Mock<IMessageBus>` for Wolverine event publication verification
- EF InMemory via factory helper with unique DB name per test
- Bogus with locale for realistic data
- Feature folders mirror Application layer structure

**Integration** (Testcontainers):
- Shared `IAsyncLifetime` fixture. One PostgreSQL container per suite
- `[Collection("Integration")]` for shared fixture
- Deterministic seed data with static IDs for assertions
- Never use EF InMemory in integration tests. Real PostgreSQL only

**E2E** (Playwright):
- Standalone Kestrel host if Wolverine + Autofac conflict with WebApplicationFactory
- Auth bypass via custom middleware (X-Test-Role / X-Test-UserId headers)
- `[Collection("E2E", DisableParallelization = true)]`. Single server, no parallel
- Chromium only unless cross-browser is explicitly required

## Minimal APIs

- Extension methods on `RouteGroupBuilder` in `Api/` folder
- No controllers. `AddControllers()` is not registered
- Map in `Program.cs` after Blazor components

## Configuration

- `IOptions<T>` with inline options classes
- `Configure<T>(config.GetSection(...))` in `*ServiceExtensions.cs`
- Base `appsettings.json` has NO secrets. Dev values in `appsettings.Development.json`
- Non-prod env vars: `${VAR:-default}`. Prod: `${VAR:?required}` for strict validation
- Central package versions in `Directory.Packages.props` with `CentralPackageTransitivePinningEnabled=true` — prevents transitive version conflicts
- Pin .NET SDK version in `global.json`: `{"sdk": "10.0.201", "rollForward": "latestPatch", "allowPrerelease": false}` — prevents build-breaking SDK drift

## SignalR

- All hubs `[Authorize]`
- Redis-compatible backplane for scale (Dragonfly or Redis)
- Rate-limit via `HubPolicy`

## Background Jobs (Quartz)

- Register in `Program.cs` with `services.AddQuartzHostedService(...)` + cron schedules
- Jobs implement `IJob` interface, auto-discovered or registered explicitly
- Common patterns: order reminders (30min/24h/1h), payment processing, daily cleanup, subscription expiration
- Job classes live in `Infrastructure/Jobs/`, registered in DI

## Health Checks

- `AddHealthChecksUI()` + `/health/live` (liveness) and `/health/ready` (readiness) endpoints
- Readiness checks verify: database connectivity, cache connectivity, external service availability
- Map in `Program.cs` after other middleware, before `app.Run()`
- CI/CD health check after deploy: `GET /health/ready` → success = deploy verified

## Encryption at Rest

Encrypt PII and sensitive fields at the application layer before persisting to PostgreSQL.

**Interface** (Application layer):

```csharp
public interface IEncryptionService
{
    string Encrypt(string plaintext);
    string Decrypt(string ciphertext);
}
```

**Implementation** (Infrastructure layer):

- Use PostgreSQL `pgcrypto` extension with AES-256
- Embed a key ID prefix with every ciphertext string: `{keyId}:{iv}:{ciphertext}`
- Dual-key fallback on decrypt: try the current key first, fall back to the previous key. This lets you rotate keys without downtime
- Store encrypted values as `bytea` columns in PostgreSQL
- Register the interface in your Autofac InfrastructureModule, not in Application

**Key rotation**:

- Configuration via `IOptions<EncryptionSettings>` with `CurrentKeyId` and `PreviousKeyId`
- A scheduled Quartz job iterates rows encrypted with the previous key and re-encrypts them with the current key
- After rotation completes, update `PreviousKeyId` and deploy

**Data migration**:

- Write a one-time migration service that batch-converts plaintext columns to encrypted `bytea`
- Verify round-trip correctness: encrypt, then decrypt, compare to original before committing
- Run as a scoped service invoked from a migration or a startup task

**Adapter pattern**: The `IEncryptionService` interface lives in Application. The concrete implementation lives in Infrastructure and uses `Npgsql` crypto functions or a managed AES library. Application code never references the infrastructure engine directly.

## Multi-Tenancy Patterns

**Tenant model**:

- A `Company` (or `Tenant`) entity with a `Guid Id`
- Tenant-scoped entities carry a `CompanyId` foreign key
- All queries and handlers filter by `CompanyId` explicitly

**No global EF query filter**: This is intentional. Global filters are invisible to reviewers and easy to miss in raw SQL, migrations, and reporting queries. Instead, every handler and query includes the tenant filter as an explicit `.Where(e => e.CompanyId == companyId)` clause. The tradeoff is more verbose code in exchange for auditability. Every query that should be tenant-scoped is obviously tenant-scoped because the filter is right there in the source.

**Claims-based provisioning**:

- On first login via OIDC, read claims from the token (email, roles)
- Provision a tenant and user record if none exists
- Store `CompanyId` as a claim for subsequent requests

**Employee lifecycle**:

```text
Active → InGracePeriod → Suspended → Terminated
```

- A scheduled job expires overdue grace periods daily
- Suspended employees cannot authenticate (enforced at the application layer)

**Billing**: Tenant-scoped billing cycle with usage tracking. Billing logic references `CompanyId` for all aggregation.

**Data isolation rule**: Every repository method that returns tenant-scoped data MUST accept `companyId` as a parameter and apply the filter. No exceptions.

### PostgreSQL Schema-per-Tenant Multi-Tenancy

An alternative to `CompanyId` FK filtering: each tenant gets its own PostgreSQL schema, isolated at the database level via `search_path`. Use Finbuckle for tenant resolution and schema provisioning.

**Registration**:

```csharp
services.AddMultiTenant<TenantInfo>()
    .WithEFCoreStore<TenantStoreDbContext, TenantInfo>();
```

**Schema strategy**: Each tenant's data lives in a schema named `tenant_{sanitized_id}`. The connection string is modified per-request to set the search path:

```csharp
public class SchemaPerTenantStore : ITenantStore<TenantInfo>
{
    private readonly string _baseConnectionString;

    public SchemaPerTenantStore(string baseConnectionString)
    {
        _baseConnectionString = baseConnectionString;
    }

    public async Task<TenantInfo?> TryGetAsync(string identifier)
    {
        await using var conn = new NpgsqlConnection(_baseConnectionString);
        await conn.OpenAsync();

        await using var cmd = new NpgsqlCommand(
            "SELECT id, identifier, name, schema_name FROM public.tenant_info WHERE identifier = @id",
            conn);
        cmd.Parameters.AddWithValue("@id", identifier);

        await using var reader = await cmd.ExecuteReaderAsync();
        if (await reader.ReadAsync())
        {
            return new TenantInfo
            {
                Id = reader.GetString(0),
                Identifier = reader.GetString(1),
                Name = reader.GetString(2),
            };
        }

        return null;
    }
}
```

**Connection string mutation** — append `;Search Path={schemaName}` to the base connection string so all EF Core queries hit the tenant's schema automatically:

```csharp
public static string WithSearchPath(string baseConnectionString, string schema)
{
    var builder = new NpgsqlConnectionStringBuilder(baseConnectionString)
    {
        SearchPath = schema
    };
    return builder.ConnectionString;
}
```

**Tenant provisioning**:

```csharp
public class TenantProvisioningService
{
    private readonly IDbContextFactory<TenantStoreDbContext> _factory;

    public async Task ProvisionAsync(string tenantId, string name)
    {
        var schema = $"tenant_{Sanitize(tenantId)}";
        await using var db = await _factory.CreateDbContextAsync();

        // Create schema
        await db.Database.ExecuteSqlRawAsync(
            $"CREATE SCHEMA IF NOT EXISTS \"{schema}\"");

        // Store metadata in public.tenant_info
        db.Tenants.Add(new TenantInfo
        {
            Id = tenantId,
            Name = name,
            ConnectionString = WithSearchPath(baseConn, schema)
        });
        await db.SaveChangesAsync();

        // Run migrations in new schema
        await MigrateSchemaAsync(schema);
    }
}
```

**Schema migrations** — discover all `tenant_*` schemas and run `MigrateAsync()` per schema:

```csharp
public class MultiTenantMigrationService
{
    public async Task MigrateAllAsync()
    {
        await using var db = await _factory.CreateDbContextAsync();
        var schemas = await db.Database
            .SqlQueryRaw<string>(
                "SELECT schema_name FROM information_schema.schemata WHERE schema_name LIKE 'tenant_%'")
            .ToListAsync();

        foreach (var schema in schemas)
        {
            var connStr = WithSearchPath(_baseConn, schema);
            await using var schemaDb = CreateContext(connStr);
            await schemaDb.Database.MigrateAsync();
        }
    }
}
```

**Tenant resolution**:

- **Blazor Server / MVC**: Cookie named `__tenant__` set at login, read by Finbuckle's `CookieTenantResolver`
- **API**: Header `X-Tenant-Id` resolved by Finbuckle's `HeaderTenantResolver`
- **Resolution chain**: `app.UseMultiTenant()` in the middleware pipeline calls resolvers in order

**Tradeoff vs explicit FK**: Schema-per-tenant provides database-level isolation (no risk of cross-tenant data leakage in queries) at the cost of per-schema migrations and more complex connection management. Choose based on isolation requirements.

## External Service Resilience (Polly)

Each external service follows a consistent trio pattern:

1. **Refit interface**: `I{Service}Api.cs` or `I{Service}Client.cs` — defines the HTTP contract
2. **Resilience policy**: `{Service}ResiliencePolicy.cs` — retry and circuit-breaker rules
3. **Service extensions**: `{Service}ServiceExtensions.cs` — DI registration helper

**Registration pattern**:

```csharp
public static IServiceCollection Add{Service}Services(
    this IServiceCollection services, IConfiguration config)
{
    services.AddRefitClient<I{Service}Api>()
        .ConfigureHttpClient(c =>
            c.BaseAddress = new Uri(config["{Service}:BaseUrl"]))
        .Add{Service}ResiliencePolicy();

    services.AddScoped<I{Service}Provider, {Service}Provider>();
    return services;
}
```

**Resilience policy** using `Microsoft.Extensions.Http.Resilience`:

```csharp
public static IHttpClientBuilder Add{Service}ResiliencePolicy(
    this IHttpClientBuilder builder)
{
    return builder.AddResilienceHandler("policy", pipeline =>
    {
        pipeline.AddRetry(new RetryStrategyOptions<HttpResponseMessage>
        {
            MaxRetryAttempts = 3,
            Delay = TimeSpan.FromMilliseconds(500),
            BackoffType = DelayBackoffType.Exponential,
            UseJitter = true,
            ShouldHandle = new PredicateBuilder<HttpResponseMessage>()
                .HandleResult(r =>
                    !r.IsSuccessStatusCode && IsTransient(r.StatusCode))
        });
    });
}

private static bool IsTransient(HttpStatusCode code) =>
    code is HttpStatusCode.RequestTimeout       // 408
        or HttpStatusCode.TooManyRequests        // 429
        or HttpStatusCode.InternalServerError    // 500
        or HttpStatusCode.BadGateway             // 502
        or HttpStatusCode.ServiceUnavailable     // 503
        or HttpStatusCode.GatewayTimeout;        // 504
```

**Exception handling**: The retry policy should also handle `HttpRequestException`, `TaskCanceledException` (timeouts), and `ApiException` (Refit).

**Rate-limited APIs**: For services with strict rate limits (geocoding, SMS, payment gateways), reduce retry count and increase backoff delays. Consider adding a jitter range of 1-5 seconds to avoid thundering-herd behavior.

## Domain Events

Use Wolverine's built-in event publishing for eventual consistency between bounded contexts.

**Define events** as records in the Domain or Application layer:
```csharp
public record OrderCompleted(Guid OrderId, Guid CustomerId, DateTime CompletedAt);
```

**Publish from handlers** via `IMessageBus`:
```csharp
public class CompleteOrderCommandHandler
{
    public async Task Handle(CompleteOrderCommand cmd, IMessageBus bus)
    {
        // ... domain logic ...
        await bus.PublishAsync(new OrderCompleted(order.Id, ...));
    }
}
```

**Consume events** in separate handlers:
```csharp
public class OrderCompletedHandler
{
    public Task Handle(OrderCompleted evt, IDbContextFactory<AppDbContext> db)
    {
        // Side effect: update read model, send notification, etc.
    }
}
```

**Ordering**: Wolverine dispatches events asynchronously. If ordering matters within a set of events, use Wolverine's message ordering or a dedicated outbox. For cross-bounded-context communication, consider using Wolverine's durable messaging with a persistence backend.

### Domain Event Dispatch via SaveChangesInterceptor

For dispatching domain events accumulated on entities, use `SaveChangesInterceptor` (specifically `SavedChangesAsync`), NOT overriding `SaveChangesAsync` on DbContext. Overriding the DbContext method changes the constructor signature and breaks `DesignTimeDbContextFactory` used by `dotnet ef` CLI tools.

```csharp
public class DomainEventDispatchInterceptor : SaveChangesInterceptor
{
    public override async ValueTask<InterceptionResult<int>> SavingChangesAsync(
        DbContextEventData eventData, InterceptionResult<int> result,
        CancellationToken cancellationToken = default)
    {
        var context = eventData.Context;
        var entities = context?.ChangeTracker.Entries<Entity>()
            .Where(e => e.State != EntityState.Detached)
            .Select(e => e.Entity)
            .Where(e => e.DomainEvents.Any())
            .ToList();
        // Dispatch events after save
        // ...
    }
}
```

Register the interceptor in DI:

```csharp
builder.Services.AddDbContextFactory<AppDbContext>(opts =>
{
    opts.UseNpgsql(connectionString);
    opts.AddInterceptors(new DomainEventDispatchInterceptor());
});
```

### Wolverine Outbox with PostgreSQL and Redis

Wolverine's transactional outbox guarantees that domain changes and message publication succeed or fail together. PostgreSQL stores the outbox table in the same database as domain data, so the enqueue happens inside the same DB transaction.

**Configuration**:

```csharp
builder.Host.UseWolverine(opts =>
{
    opts.Discovery.IncludeAssembly(typeof(CreateOrderCommand).Assembly);

    var redis = builder.Configuration.GetConnectionString("redis") ?? "localhost:6379";
    opts.UseRedisTransport(redis).AutoProvision();

    var pg = builder.Configuration.GetConnectionString("postgres")
        ?? throw new InvalidOperationException("Postgres connection string required.");
    opts.PersistMessagesWithPostgresql(pg);

    opts.PublishMessage<OrderCompletedEvent>().ToRedisStream("order-events");
    opts.PublishMessage<PaymentConfirmedEvent>().ToRedisStream("payment-events");

    opts.ListenToRedisStream("order-events", "order-processors").UseDurableInbox();
    opts.ListenToRedisStream("payment-events", "payment-processors").UseDurableInbox();
});
```

**Key concepts**:

- `PersistMessagesWithPostgresql` — transactional outbox: messages stored in the same DB transaction as domain changes, so a handler crash never produces orphaned events
- `UseRedisTransport` — cross-module streaming without a dedicated message broker (Redis Streams under the hood)
- `UseDurableInbox` — prevents duplicate processing on retry or restart
- `IncludeAssembly` — tells Wolverine which assembly to scan for handlers and messages
- `AutoProvision` — creates Redis stream consumer groups automatically on startup

## CORS Configuration

```csharp
builder.Services.AddCors(options =>
{
    options.AddPolicy("AllowBlazorWasm", policy =>
    {
        policy.WithOrigins(builder.Configuration["Cors:AllowedOrigins"]?.Split(',') ?? [])
              .AllowAnyHeader()
              .AllowAnyMethod()
              .AllowCredentials();
    });
});

// In middleware pipeline:
app.UseCors("AllowBlazorWasm");
```

**Never use `AllowAnyOrigin` with `AllowCredentials`** — it's explicitly forbidden by the CORS spec. List specific origins.

**Dev vs prod**: Dev can allow `localhost` with any port. Prod must list exact origins.

**Same-origin**: If both Blazor Server and API are on the same origin, CORS is not needed. It only applies when the browser makes cross-origin requests (e.g., standalone WASM client calling a different API origin).

## Keycloak/OIDC Authentication

**Scheme setup**: Cookie (interactive) + OIDC + JWT Bearer (API calls). Both schemes active simultaneously.

```csharp
builder.Services.AddAuthentication(options =>
{
    options.DefaultScheme = CookieAuthenticationDefaults.AuthenticationScheme;
})
.AddCookie()
.AddOpenIdConnect(...)
.AddJwtBearer(...);
```

**Claims mapping**: Map Keycloak realm roles to `ClaimTypes.Role` in the `OnTokenValidated` event so standard `[Authorize(Roles = "...")]` works.

**Multi-scheme authorization**: Minimal APIs that serve both interactive and API clients use:
```csharp
[Authorize(AuthenticationSchemes = "Bearer,Cookie")]
```

**Admin API provisioning**: Define an `IKeycloakAdminClient` interface in Application. Implement it in Infrastructure using Keycloak's Admin REST API. Use it for user creation, role assignment, and password resets without direct database writes.

**Startup validation**: Fail fast if `Authority` or `ClientId` is missing or empty. Check in a startup filter or `Program.cs` before `app.Run()`.

**Health check**: Probe the OIDC discovery endpoint (`.well-known/openid-configuration`) as part of readiness checks.

*See the `keycloak-oidc` skill for full implementation details.*

## Migration Management

**PostgreSQL specifics**:

- Enable `pgcrypto` extension in the initial migration: `migrationBuilder.Sql("CREATE EXTENSION IF NOT EXISTS pgcrypto");`
- Use `jsonb` for JSON columns, not `json`
- Use `bytea` for encrypted binary fields

**Design-time factory** (required for `dotnet ef` CLI):

```csharp
public class AppDbContextFactory : IDesignTimeDbContextFactory<AppDbContext>
{
    public AppDbContext CreateDbContext(string[] args)
    {
        var optionsBuilder = new DbContextOptionsBuilder<AppDbContext>();
        optionsBuilder.UseNpgsql("Host=localhost;Database=devdb;Username=dev;Password=dev");
        return new AppDbContext(optionsBuilder.Options);
    }
}
```

**Multi-environment strategy**:

- Run `dotnet ef database update` as part of the CI deploy pipeline, not during app startup
- Each environment (dev, stage, prod) applies migrations from the same branch
- The migration connection string comes from environment variables, never from source code

**Idempotent data migrations**:

- Use `migrationBuilder.InsertData` for seed data
- Wrap inserts in SQL with `ON CONFLICT DO NOTHING` to make them safe to re-run
- For complex transformations (encrypting existing columns, reshaping data), write a separate migration service class rather than embedding long SQL in the migration `Up()` method

**Golden rule**: Never modify a migration that has been applied to any environment. If something is wrong, create a new migration that corrects it. Altering applied migrations causes state divergence between environments.

## Anti-Patterns

- DO NOT use scoped DbContext
- DO NOT register `AddValidatorsFromAssembly` if using Wolverine middleware — DI resolves validators into middleware automatically
- DO NOT suppress null warnings (`!`, `null!`)
- DO NOT call ES module exports as global functions. Use `IJSObjectReference` module import
- DO NOT hard-delete data that requires audit trail. Soft-delete only
- DO NOT log sensitive tokens or credentials
- DO NOT use API controllers. Use minimal API endpoints
- DO NOT use `IMessageBus.SendAsync()` for request/response — use `InvokeAsync<TResponse>()` instead
- DO NOT use `IMessageBus.InvokeAsync()` for fire-and-forget events — use `PublishAsync()` instead
- DO NOT call `builder.Entity<T>().IsMultiTenant()` in individual entity configurations when using Finbuckle. The centralized `ConfigureBaseEntity()` method handles multi-tenant filtering globally. Per-entity `IsMultiTenant()` calls conflict with Finbuckle's shadow property management and cause duplicate query filters. Shared or reference entities (e.g., Country, Currency, Category) must NOT call `IsMultiTenant()`.
- DO NOT cache Data Encryption Keys (DEKs) in external caches (Redis, Dragonfly, etc.). DEKs must be cached in-process only. External caching exposes encryption keys to network-level attacks and violates encryption-at-rest security models. Use a singleton in-process cache with TTL-based eviction.
- DO NOT use `builder.Entity<T>()` inside EF `Configure()` methods when using typed `Configure<T>` overloads. In billing and similar configurations, always use the typed `Configure<T>` pattern to avoid ambiguous overloads and potential misconfiguration.
- DO NOT subtract monetary values from different currencies without normalization. Cross-currency gain/loss calculations must convert to a common currency first. E.g., `CurrentValueAmount - CostBasisAmount` is wrong if `CostBasisCurrency` differs from `Currency`. Always normalize: `var normalizedCost = await _fxConverter.ConvertAsync(CostBasisAmount, CostBasisCurrency, Currency); var gainLoss = CurrentValueAmount - normalizedCost;`
- DO NOT accept `tenantId` as a parameter in helper methods but never use it for filtering. This creates a cross-tenant data leak in multi-tenant apps. Always apply `EF.Property<string>(entity, "TenantId") == tenantId.ToString()` or equivalent filtering.
- DO NOT let the Reporting module reference 7+ domain module assemblies directly. This creates broad cross-module coupling. Instead, use report-oriented read models, projections, or contracts owned by the Reporting module.
- DO NOT compute `UnrealizedGainLoss` as a raw subtraction without checking if currencies match. Portfolio allocation sums `CurrentValueAmount` across holdings by security type without currency conversion produces misleading totals in multi-currency portfolios.

## Related Skills

- **keycloak-oidc** — Full OIDC auth setup, realm role mapping, admin API provisioning (see "Keycloak/OIDC Authentication" section above for summary)
- **postgresql-efcore** — PostgreSQL-specific EF Core patterns: pgcrypto, jsonb, full-text search, soft-delete interceptors, Testcontainers
- **signalr-realtime** — Hub design, scaling, group management, rate limiting (see "SignalR" section above for summary)
- **aspnetcore-health** — Health checks, readiness probes, startup validation, OTel integration (see "Health Checks" section above for summary)
- **quartz-scheduling** — Background job scheduling with Quartz.NET: cron, failure handling, distributed locking
- **lgpd-data-privacy** — LGPD compliance: consent, encryption, key rotation, audit trails, breach notification
- **docker-deployment** — Docker Compose overlay pattern, nginx, TLS, backup/restore, CI/CD
- **blazor-mudblazor** — Blazor + MudBlazor UI patterns: dark mode, JS interop, accessibility, WebRTC
- **finbuckle-multitenancy** — Finbuckle multi-tenancy: tenant resolution strategies, EF Core store, schema-per-tenant isolation
- **distributed-caching** — Redis/Dragonfly cache-aside, cache invalidation, stampede prevention, multi-layer caching
