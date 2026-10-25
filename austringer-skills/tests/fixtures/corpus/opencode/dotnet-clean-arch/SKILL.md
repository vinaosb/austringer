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

Entity base class: `Guid Id`, `DateTime CreatedAt/UpdatedAt`, `bool IsDeleted`. Value objects as `record` types.

**Entity config file placement**: at module root level (e.g., `DocumentEntityConfigurations.cs`), NOT in subdirectory.

### Domain Entity Business Rules

**Effective deadline fallback**: Use `ExtensionDeadline ?? FilingDeadline` for optional extension deadlines:
```csharp
// WRONG — ignores extension
var isOverdue = DateTime.UtcNow > entity.FilingDeadline;

// CORRECT — use extension as effective deadline
var effectiveDeadline = entity.ExtensionDeadline ?? entity.FilingDeadline;
var isOverdue = DateTime.UtcNow > effectiveDeadline;
```

## CQRS with Wolverine

- Feature folders: `Commands/` and `Queries/` subfolders
- Separate `*CommandHandler` / `*QueryHandler` per command/query
- Handlers are STATIC classes. Use `ILogger<CommandType>` param, not `ILogger<HandlerClass>` (CS0718 error)
- Events in `Events/` subfolder
- FluentValidation via Wolverine middleware: `ValidationMiddleware<TRequest>` runs ALL `IValidator<TRequest>` before handler
- Middleware takes `IEnumerable<IValidator<TRequest>>` via DI, runs `Task.WhenAll`, aggregates failures
- No explicit `AddValidatorsFromAssembly` — DI resolves validators into middleware automatically

### Tenant-Aware Command Pattern

All tenant-aware commands implement marker interface for explicit, auditable tenant filtering:
```csharp
public interface IWolverineTenantCommand
{
    Guid TenantId { get; }
}

public sealed record CreateCustomerCommand(
    Guid TenantId, string Name, string Email) : IWolverineTenantCommand;
```

Wolverine middleware reads `TenantId`, sets multi-tenant context before handler executes.

### Wolverine Middleware Pipeline

**ValidationMiddleware** — runs all FluentValidation validators before handler:
```csharp
public class ValidationMiddleware<TRequest>
{
    private readonly IEnumerable<IValidator<TRequest>> _validators;

    public ValidationMiddleware(IEnumerable<IValidator<TRequest>> validators) => _validators = validators;

    public async Task BeforeHandleAsync(TRequest message, CancellationToken ct)
    {
        if (!_validators.Any()) return;
        var context = new ValidationContext<TRequest>(message);
        var results = await Task.WhenAll(_validators.Select(v => v.ValidateAsync(context, ct)));
        var failures = results.SelectMany(r => r.Errors).Where(f => f is not null).ToList();
        if (failures.Count != 0) throw new ValidationException(failures);
    }
}
```

**TenantMiddleware** — resolves tenant ID into `ITenantAware` commands:
```csharp
public class TenantMiddleware<TRequest>
{
    private readonly IMultiTenantContextAccessor _tenantAccessor;
    public TenantMiddleware(IMultiTenantContextAccessor tenantAccessor) => _tenantAccessor = tenantAccessor;

    public async Task BeforeHandleAsync(TRequest message, CancellationToken ct)
    {
        if (message is ITenantAware tenantAware)
        {
            var tenantId = _tenantAccessor.MultiTenantContext?.TenantInfo?.Id;
            if (tenantId is not null) tenantAware.TenantId = tenantId;
        }
    }
}
```

**AuditingMiddleware** — sets audit user from `ClaimsPrincipal` on `IAuditableCommand`:
```csharp
public class AuditingMiddleware<TRequest>
{
    private readonly IHttpContextAccessor _httpContextAccessor;
    public AuditingMiddleware(IHttpContextAccessor httpContextAccessor) => _httpContextAccessor = httpContextAccessor;

    public async Task BeforeHandleAsync(TRequest message, CancellationToken ct)
    {
        if (message is IAuditableCommand auditable)
        {
            var user = _httpContextAccessor.HttpContext?.User;
            var userId = user?.FindFirst(ClaimTypes.NameIdentifier)?.Value
                ?? user?.FindFirst("sub")?.Value ?? "system";
            auditable.SetAuditUser(userId);
        }
    }
}
```

**LifecycleMiddleware** — calls lifecycle hooks on `ILifecycleHook` commands:
```csharp
public class LifecycleMiddleware<TRequest>
{
    public async Task BeforeHandleAsync(TRequest message, CancellationToken ct)
    {
        if (message is ILifecycleHook hook) await hook.BeforeValidateAsync(ct);
    }
    public async Task AfterHandleAsync(TRequest message, CancellationToken ct)
    {
        if (message is ILifecycleHook hook) await hook.OnSubmitAsync(ct);
    }
}
```

**Registration**:
```csharp
builder.Host.UseWolverine(opts =>
{
    opts.Policies.ForMessagesOfType<ICommand>().AddMiddleware(typeof(ValidationMiddleware<>));
    opts.Policies.ForMessagesOfType<ITenantAware>().AddMiddleware(typeof(TenantMiddleware<>));
    opts.Policies.ForMessagesOfType<IAuditableCommand>().AddMiddleware(typeof(AuditingMiddleware<>));
});
```

### Wolverine Tenant Resolution Middleware

Resolves tenant from command payload, falls back to envelope header, sets Finbuckle context:
```csharp
public class TenantResolutionMiddleware
{
    public static void Before(IMessageContext context, IWolverineTenantCommand command)
    {
        var tenantId = command.TenantId;
        if (tenantId == Guid.Empty)
        {
            var envelopeTenant = context.Envelope?.TenantId;
            if (!string.IsNullOrEmpty(envelopeTenant))
                tenantId = Guid.Parse(envelopeTenant);
        }
        var accessor = context.ServiceProvider.GetRequiredService<IMultiTenantContextAccessor>();
        var store = accessor.Store ?? throw new InvalidOperationException("Multi-tenant store not configured.");
        var tenantInfo = store.TryGetAsync(tenantId.ToString()).GetAwaiter().GetResult();
        if (tenantInfo is null)
            throw new InvalidOperationException($"Tenant '{tenantId}' not found in store.");
        accessor.Store = store;
    }
}
```

**Registration**:
```csharp
builder.Host.UseWolverine(opts =>
{
    opts.Policies.AddMiddleware<TenantResolutionMiddleware>();
    opts.Policies.AddMiddleware<TenantAuthorizationMiddleware>();
    opts.Discovery.IncludeAssembly(typeof(CustomerModuleRegistration).Assembly);
    opts.Discovery.IncludeAssembly(typeof(OrderModuleRegistration).Assembly);
});
```

### Wolverine Tenant Authorization Middleware

Verifies current user belongs to tenant on command. System admins bypass:
```csharp
public class TenantAuthorizationMiddleware
{
    public static void Before(IMessageContext context, IWolverineTenantCommand command)
    {
        var currentUser = context.ServiceProvider.GetRequiredService<ICurrentUser>();
        if (currentUser.Roles.Contains("system-admin")) return;

        if (currentUser.TenantId != command.TenantId)
            throw new UnauthorizedAccessException(
                $"User '{currentUser.UserId}' cannot access tenant '{command.TenantId}'");
    }
}
```

Defense-in-depth — rejects cross-tenant access before any handler/database call.

## EF Core Patterns

- ALWAYS use `IDbContextFactory<AppDbContext>`. Never scoped DbContext.
- Design-time factory at `Infrastructure/DesignTime/AppDbContextFactory.cs`.
- Audit interceptor: soft-deletes + AuditLog entries.
- Repositories named `*Repository`, auto-registered by suffix.

### Avoid Multi-Source `.Concat()` Queries

EF Core `.Concat()` across 3+ entity types may fail with Npgsql. Collect resource IDs first, single query per type, union in memory:
```csharp
// CORRECT: Collect IDs per type, single query
var patientIds = await dbContext.Patients.Where(p => p.PatientId == patientId).Select(p => p.Id).ToListAsyncAsync();
var prontuarioIds = await dbContext.Prontuarios.Where(p => p.PatientId == patientId).Select(p => p.Id).ToListAsyncAsync();
var allIds = patientIds.Concat(prontuarioIds).ToList();
var entries = await dbContext.Set<AuditLog>()
    .Where(a => allIds.Contains(a.EntityId))
    .OrderByDescending(a => a.Timestamp)
    .ToListAsyncAsync();
```

Keeps each query simple, avoids translation failures, often faster.

### DbContext.GetService&lt;T&gt;() Workaround

Cast to `IServiceProvider`:
```csharp
var service = ((IServiceProvider)context).GetService&lt;T&gt;();
```

## Dependency Injection

- Autofac via `UseServiceProviderFactory(new AutofacServiceProviderFactory())`
- Repos auto-registered by `*Repository` suffix
- Services explicitly registered
- HttpClient-based: `AddHttpClient<T>()` in Program.cs
- External services: Refit + `*ResiliencePolicy.cs` + `*ServiceExtensions.cs`

## Testing Pyramid

**Unit** (xUnit + Shouldly + Moq + Bogus):
- `Mock<IMessageBus>` for Wolverine event verification
- EF InMemory via factory with unique DB name per test
- Bogus with locale for realistic data

**Integration** (Testcontainers):
- Shared `IAsyncLifetime` fixture, one PostgreSQL per suite
- `[Collection("Integration")]`
- Deterministic seed data. Never EF InMemory — real PostgreSQL only.

**E2E** (Playwright):
- Standalone Kestrel if Wolverine + Autofac conflict with WebApplicationFactory
- Auth bypass via custom middleware (X-Test-Role / X-Test-UserId headers)
- `[Collection("E2E", DisableParallelization = true)]`. Chromium only unless cross-browser required.

## Minimal APIs

- Extension methods on `RouteGroupBuilder` in `Api/` folder
- No controllers. `AddControllers()` not registered.
- Map in Program.cs after Blazor components.

## Configuration

- `IOptions<T>` with inline options classes
- `Configure<T>(config.GetSection(...))` in `*ServiceExtensions.cs`
- Base `appsettings.json` has NO secrets. Dev in `appsettings.Development.json`.
- Non-prod: `${VAR:-default}`. Prod: `${VAR:?required}`.
- Central package versions in `Directory.Packages.props` with `CentralPackageTransitivePinningEnabled=true`.
- Pin SDK in `global.json`: `{"sdk": "10.0.201", "rollForward": "latestPatch", "allowPrerelease": false}`.

## SignalR

- All hubs `[Authorize]`
- Redis-compatible backplane (Dragonfly/Redis)
- Rate-limit via `HubPolicy`

## Background Jobs (Quartz)

- `services.AddQuartzHostedService(...)` + cron schedules
- Jobs implement `IJob`, auto-discovered or explicit
- Patterns: order reminders, payment processing, daily cleanup, subscription expiry
- Classes in `Infrastructure/Jobs/`

## Health Checks

- `AddHealthChecksUI()` + `/health/live` + `/health/ready`
- Readiness: DB, cache, external services
- CI/CD health gate after deploy: `GET /health/ready` → 200 = verified

## Encryption at Rest

**Interface** (Application):
```csharp
public interface IEncryptionService
{
    string Encrypt(string plaintext);
    string Decrypt(string ciphertext);
}
```

**Implementation** (Infrastructure):
- PostgreSQL `pgcrypto` AES-256
- Key ID prefix with every ciphertext: `{keyId}:{iv}:{ciphertext}`
- Dual-key fallback on decrypt: current key first, previous key fallback
- Encrypted values as `bytea` columns
- Register in Autofac InfrastructureModule

**Key rotation**: `IOptions<EncryptionSettings>` with `CurrentKeyId`/`PreviousKeyId`. Quartz job re-encrypts old rows.

**Data migration**: Batch-convert plaintext to encrypted `bytea`. Verify round-trip before committing.

## Multi-Tenancy Patterns

**Tenant model**: `Company` (or `Tenant`) with `Guid Id`. Tenant-scoped entities carry `CompanyId` FK.

**No global EF query filter**: Intentional. Every handler includes explicit `.Where(e => e.CompanyId == companyId)`. More verbose but auditable.

**Claims-based provisioning**: On first OIDC login, read claims, provision tenant+user if absent. Store `CompanyId` as claim.

**Employee lifecycle**: `Active → InGracePeriod → Suspended → Terminated`. Scheduled job expires overdue grace periods.

**Data isolation**: Every repository method returning tenant-scoped data MUST accept `companyId` param and filter. No exceptions.

### PostgreSQL Schema-per-Tenant Multi-Tenancy

Alternative to `CompanyId` FK: each tenant gets own PostgreSQL schema via `search_path`. Finbuckle for tenant resolution.

**Registration**:
```csharp
services.AddMultiTenant<TenantInfo>().WithEFCoreStore<TenantStoreDbContext, TenantInfo>();
```

**Schema strategy**: `tenant_{sanitized_id}`. Connection string modified per-request:
```csharp
public static string WithSearchPath(string baseConnectionString, string schema)
{
    var builder = new NpgsqlConnectionStringBuilder(baseConnectionString) { SearchPath = schema };
    return builder.ConnectionString;
}
```

**Tenant provisioning**: CREATE SCHEMA + store metadata + EF MigrateAsync() per schema.

**Migrations**: Discover all `tenant_*` schemas, run `MigrateAsync()` per schema.

**Tenant resolution**: Blazor Server: Cookie `__tenant__`. API: Header `X-Tenant-Id`. Chain: `app.UseMultiTenant()`.

**Tradeoff**: Schema-per-tenant provides DB-level isolation at cost of per-schema migrations and complex connection management.

## External Service Resilience (Polly)

Each service: Refit interface + ResiliencePolicy + ServiceExtensions.

**Registration**:
```csharp
public static IServiceCollection Add{Service}Services(this IServiceCollection services, IConfiguration config)
{
    services.AddRefitClient<I{Service}Api>()
        .ConfigureHttpClient(c => c.BaseAddress = new Uri(config["{Service}:BaseUrl"]))
        .Add{Service}ResiliencePolicy();
    services.AddScoped<I{Service}Provider, {Service}Provider>();
    return services;
}
```

**Resilience policy**:
```csharp
public static IHttpClientBuilder Add{Service}ResiliencePolicy(this IHttpClientBuilder builder)
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
                .HandleResult(r => !r.IsSuccessStatusCode && IsTransient(r.StatusCode))
        });
    });
}

private static bool IsTransient(HttpStatusCode code) =>
    code is HttpStatusCode.RequestTimeout or HttpStatusCode.TooManyRequests
        or HttpStatusCode.InternalServerError or HttpStatusCode.BadGateway
        or HttpStatusCode.ServiceUnavailable or HttpStatusCode.GatewayTimeout;
```

Also handle `HttpRequestException`, `TaskCanceledException` (timeouts), `ApiException` (Refit).

## Domain Events

Wolverine built-in event publishing for eventual consistency between bounded contexts.

**Define events** as records:
```csharp
public record OrderCompleted(Guid OrderId, Guid CustomerId, DateTime CompletedAt);
```

**Publish** via `IMessageBus`:
```csharp
await bus.PublishAsync(new OrderCompleted(order.Id, ...));
```

**Consume** in separate handlers:
```csharp
public class OrderCompletedHandler
{
    public Task Handle(OrderCompleted evt, IDbContextFactory<AppDbContext> db) { ... }
}
```

**Ordering**: Wolverine dispatches async. Use durable messaging with persistence backend for cross-bounded-context.

### Domain Event Dispatch via SaveChangesInterceptor

Use `SaveChangesInterceptor` (NOT overriding DbContext `SaveChangesAsync` — breaks DesignTimeDbContextFactory):

```csharp
public class DomainEventDispatchInterceptor : SaveChangesInterceptor
{
    public override async ValueTask<InterceptionResult<int>> SavingChangesAsync(
        DbContextEventData eventData, InterceptionResult<int> result, CancellationToken ct = default)
    {
        var context = eventData.Context;
        var entities = context?.ChangeTracker.Entries<Entity>()
            .Where(e => e.State != EntityState.Detached)
            .Select(e => e.Entity)
            .Where(e => e.DomainEvents.Any()).ToList();
        // Dispatch after save
    }
}
```

Register:
```csharp
opts.AddInterceptors(new DomainEventDispatchInterceptor());
```

### Wolverine Outbox with PostgreSQL and Redis

Transactional outbox: domain changes + message publication succeed or fail together.
```csharp
builder.Host.UseWolverine(opts =>
{
    opts.Discovery.IncludeAssembly(typeof(CreateOrderCommand).Assembly);
    var redis = builder.Configuration.GetConnectionString("redis") ?? "localhost:6379";
    opts.UseRedisTransport(redis).AutoProvision();
    var pg = builder.Configuration.GetConnectionString("postgres") ?? throw new InvalidOperationException("...");
    opts.PersistMessagesWithPostgresql(pg);
    opts.PublishMessage<OrderCompletedEvent>().ToRedisStream("order-events");
    opts.ListenToRedisStream("order-events", "order-processors").UseDurableInbox();
});
```

- `PersistMessagesWithPostgresql` — transactional outbox
- `UseRedisTransport` — cross-module streaming (Redis Streams)
- `UseDurableInbox` — prevents duplicate processing
- `AutoProvision` — creates Redis consumer groups on startup

## CORS Configuration

```csharp
builder.Services.AddCors(options =>
{
    options.AddPolicy("AllowBlazorWasm", policy =>
    {
        policy.WithOrigins(builder.Configuration["Cors:AllowedOrigins"]?.Split(',') ?? [])
              .AllowAnyHeader().AllowAnyMethod().AllowCredentials();
    });
});
app.UseCors("AllowBlazorWasm");
```

**Never `AllowAnyOrigin` with `AllowCredentials`** — forbidden by CORS spec.
**Same-origin**: Blazor Server + API on same origin needs no CORS.

## Keycloak/OIDC Authentication

**Scheme setup**: Cookie (interactive) + OIDC + JWT Bearer (API):
```csharp
builder.Services.AddAuthentication(options =>
{
    options.DefaultScheme = CookieAuthenticationDefaults.AuthenticationScheme;
})
.AddCookie().AddOpenIdConnect(...).AddJwtBearer(...);
```

**Claims mapping**: Map Keycloak realm roles to `ClaimTypes.Role` in `OnTokenValidated`.
**Multi-scheme**: Minimal APIs use `[Authorize(AuthenticationSchemes = "Bearer,Cookie")]`.
**Admin API**: `IKeycloakAdminClient` in Application, implementation in Infrastructure.
**Startup validation**: Fail fast if `Authority` or `ClientId` missing.
**Health check**: Probe `.well-known/openid-configuration`.

## Migration Management

- Enable `pgcrypto`: `migrationBuilder.Sql("CREATE EXTENSION IF NOT EXISTS pgcrypto");`
- `jsonb` for JSON columns, `bytea` for encrypted binary

**Design-time factory**:
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

**Multi-environment**: Run `dotnet ef database update` in CI deploy, not app startup.
**Idempotent data migrations**: Use `ON CONFLICT DO NOTHING`. Complex transformations in separate migration service class.
**Golden rule**: Never modify applied migrations. Create new migration to correct.

## Anti-Patterns

- DO NOT use scoped DbContext
- DO NOT register `AddValidatorsFromAssembly` with Wolverine middleware — DI resolves automatically
- DO NOT suppress null warnings (`!`, `null!`)
- DO NOT call ES module exports as global functions — use `IJSObjectReference`
- DO NOT hard-delete auditable data — soft-delete only
- DO NOT log sensitive tokens/credentials
- DO NOT use API controllers — use minimal API endpoints
- DO NOT use `IMessageBus.SendAsync()` for request/response — use `InvokeAsync<TResponse>()`
- DO NOT use `IMessageBus.InvokeAsync()` for fire-and-forget — use `PublishAsync()`
- DO NOT call `builder.Entity<T>().IsMultiTenant()` in per-entity configs — centralized `ConfigureBaseEntity()` handles it
- DO NOT cache DEKs in external caches (Redis/Dragonfly) — in-process only
- DO NOT use `builder.Entity<T>()` inside EF `Configure()` when typed `Configure<T>` overloads exist
- DO NOT subtract monetary values from different currencies without normalizing first
- DO NOT accept `tenantId` param but never use for filtering — creates cross-tenant leak
- DO NOT let Reporting module reference 7+ domain modules — use report-oriented read models
- DO NOT compute `UnrealizedGainLoss` as raw subtraction without checking currency match

## Related Skills

- **keycloak-oidc** — Full OIDC auth, realm role mapping, admin API provisioning
- **postgresql-efcore** — pgcrypto, jsonb, full-text search, soft-delete, Testcontainers
- **signalr-realtime** — Hub design, scaling, group management, rate limiting
- **aspnetcore-health** — Health checks, readiness probes, startup validation, OTel
- **quartz-scheduling** — Background job scheduling: cron, failure handling, distributed locking
- **lgpd-data-privacy** — LGPD compliance: consent, encryption, key rotation, audit trails
- **docker-deployment** — Docker Compose overlay, nginx, TLS, backup/restore, CI/CD
- **blazor-mudblazor** — Blazor + MudBlazor UI: dark mode, JS interop, accessibility, WebRTC
- **finbuckle-multitenancy** — Tenant resolution strategies, EF Core store, schema-per-tenant
- **distributed-caching** — Redis/Dragonfly cache-aside, invalidation, stampede prevention