---
name: postgresql-efcore
description: "PostgreSQL-specific Entity Framework Core patterns. pgcrypto encryption, jsonb columns, full-text search, soft-delete interceptors, audit logging, array operations, enum mapping, concurrent migrations, Testcontainers, IDbContextFactory, Npgsql-specific configuration. USE FOR: PostgreSQL database setup, EF Core configuration, encrypted fields, JSON columns, full-text search, audit trails, migration management, integration testing with Testcontainers, performance optimization. DO NOT USE FOR: SQL Server, MySQL, SQLite, non-EF-Core ORMs, pure SQL without EF Core."
license: MIT
metadata:
  author: community
  version: "1.0.0"
  frameworks: [".NET 8+", "EF Core", "PostgreSQL", "Npgsql"]
---

# PostgreSQL + EF Core Patterns

Production-ready patterns for .NET apps with PostgreSQL + EF Core via Npgsql.

## Project Setup

### NuGet Packages

```xml
<ItemGroup>
  <PackageReference Include="Npgsql.EntityFrameworkCore.PostgreSQL" />
  <PackageReference Include="Microsoft.EntityFrameworkCore.Design">
    <PrivateAssets>all</PrivateAssets>
    <IncludeAssets>runtime; build; native; contentfiles; analyzers</IncludeAssets>
  </PackageReference>
  <PackageReference Include="Npgsql" />
</ItemGroup>
```

### Connection String Format

```
Host=localhost;Port=5432;Database=myapp;Username=app_user;Password=secret;SSL Mode=Require;Trust Server Certificate=true;Pooling=true;Maximum Pool Size=100;Timeout=30;
```

### IDbContextFactory (Always, Never Scoped)

Register factory in DI. Inject `IDbContextFactory<AppDbContext>` everywhere. Never inject scoped `AppDbContext`.

```csharp
// Program.cs
builder.Services.AddDbContextFactory<AppDbContext>(options =>
    options.UseNpgsql(
        builder.Configuration.GetConnectionString("Default"),
        npgsql => npgsql.MigrationsAssembly(typeof(AppDbContext).Assembly)
    ));

// Usage in service
public class OrderService
{
    private readonly IDbContextFactory<AppDbContext> _dbFactory;

    public OrderService(IDbContextFactory<AppDbContext> dbFactory) =>
        _dbFactory = dbFactory;

    public async Task<Order?> GetByIdAsync(Guid id)
    {
        await using var db = await _dbFactory.CreateDbContextAsync();
        return await db.Orders.FindAsync(id);
    }
}
```

### IDbContextFactory for Concurrent Migrations

Register via `AddDbContextFactory` when concurrent migration coordination needs independent context instances outside scoped DI:

```csharp
builder.Services.AddDbContextFactory<AppDbContext>(options =>
    options.UseNpgsql(
        builder.Configuration.GetConnectionString("Default"),
        npgsql => npgsql.MigrationsAssembly(typeof(AppDbContext).Assembly)
    ));

public class ConcurrentMigrationService
{
    private readonly IDbContextFactory<AppDbContext> _dbFactory;

    public ConcurrentMigrationService(IDbContextFactory<AppDbContext> dbFactory) =>
        _dbFactory = dbFactory;

    public async Task MigrateWithLockAsync(CancellationToken ct = default)
    {
        await using var db = await _dbFactory.CreateDbContextAsync(ct);
        // Coordinate migration with advisory lock
    }
}
```

Factory pattern essential — service needs short-lived contexts for lock acquisition + migration without request-scoped DI.

### Design-Time Factory for dotnet ef CLI

```csharp
// Infrastructure/Data/DesignTime/AppDbContextFactory.cs
public class AppDbContextFactory : IDesignTimeDbContextFactory<AppDbContext>
{
    public AppDbContext CreateDbContext(string[] args)
    {
        var config = new ConfigurationBuilder()
            .SetBasePath(Directory.GetCurrentDirectory())
            .AddJsonFile("appsettings.json")
            .AddJsonFile("appsettings.Development.json", optional: true)
            .AddEnvironmentVariables()
            .Build();

        var optionsBuilder = new DbContextOptionsBuilder<AppDbContext>();
        optionsBuilder.UseNpgsql(
            config.GetConnectionString("Default"),
            npgsql => npgsql.MigrationsAssembly(typeof(AppDbContext).Assembly));

        return new AppDbContext(optionsBuilder.Options);
    }
}
```

## pgcrypto and Encryption

Enable pgcrypto in initial migration so subsequent migrations reference its functions.

```csharp
// In first migration's Up method
protected override void Up(MigrationBuilder migrationBuilder)
{
    migrationBuilder.Sql("CREATE EXTENSION IF NOT EXISTS pgcrypto");
}
```

### AES-256 Encrypt/Decrypt via Value Converter

Store sensitive fields as `bytea`. Encryption at application layer.

```csharp
public class EncryptedValueConverter<T> : ValueConverter<T, byte[]>
    where T : class
{
    public EncryptedValueConverter(EncryptionService encryption)
        : base(
            v => encryption.Encrypt(JsonSerializer.Serialize(v)),
            v => JsonSerializer.Deserialize<T>(encryption.Decrypt(v))!)
    {
    }
}

// In DbContext OnModelCreating
modelBuilder.Entity<Customer>(entity =>
{
    entity.Property(p => p.SensitiveNotes)
        .HasColumnType("bytea")
        .HasConversion(
            v => _encryption.Encrypt(JsonSerializer.Serialize(v)),
            v => JsonSerializer.Deserialize<string>(_encryption.Decrypt(v))!);
});
```

### Encryption Service

```csharp
public class EncryptionService
{
    private readonly byte[] _key;
    private readonly byte[] _fallbackKey;

    public EncryptionService(IConfiguration config)
    {
        _key = Convert.FromBase64String(config["Encryption:Key"]!);
        _fallbackKey = Convert.FromBase64String(config["Encryption:FallbackKey"]!);
    }

    public byte[] Encrypt(string plaintext)
    {
        using var aes = Aes.Create();
        aes.Key = _key;
        aes.GenerateIV();
        using var encryptor = aes.CreateEncryptor();
        var ciphertext = encryptor.TransformFinalBlock(
            Encoding.UTF8.GetBytes(plaintext), 0, plaintext.Length);
        var result = new byte[aes.IV.Length + ciphertext.Length];
        aes.IV.CopyTo(result, 0);
        ciphertext.CopyTo(result, aes.IV.Length);
        return result;
    }

    public string Decrypt(byte[] data)
    {
        try { return DecryptWithKey(data, _key); }
        catch (CryptographicException) { return DecryptWithKey(data, _fallbackKey); }
    }

    private string DecryptWithKey(byte[] data, byte[] key)
    {
        using var aes = Aes.Create();
        aes.Key = key;
        var iv = new byte[16];
        Array.Copy(data, iv, 16);
        aes.IV = iv;
        using var decryptor = aes.CreateDecryptor();
        var plaintext = decryptor.TransformFinalBlock(data, 16, data.Length - 16);
        return Encoding.UTF8.GetString(plaintext);
    }
}
```

## jsonb Columns

### Mapping to Owned Types

```csharp
// Domain
public record Address(string Street, string City, string State, string ZipCode);

// DbContext
modelBuilder.Entity<Customer>(entity =>
{
    entity.OwnsOne(p => p.Address, address =>
    {
        address.ToJson("address");
        address.Property(a => a.Street).HasColumnName("street");
    });
});
```

### Mapping to Dictionary

```csharp
modelBuilder.Entity<Product>(entity =>
{
    entity.Property(s => s.Metadata)
        .HasColumnType("jsonb")
        .HasConversion(
            v => JsonSerializer.Serialize(v, JsonSerializerOptions),
            v => JsonSerializer.Deserialize<Dictionary<string, JsonElement>>(v, JsonSerializerOptions)!);
});
```

### Querying jsonb

```csharp
// Query nested field
var results = await db.Customers
    .Where(p => EF.Functions.JsonContains(p.Address, "\"State\": \"SP\""))
    .ToListAsync();

// Raw SQL for complex jsonb queries
var customers = await db.Customers
    .FromSqlInterpolated($"""
        SELECT * FROM customers WHERE metadata->>'category' = {category}
        """)
    .ToListAsync();
```

### GIN Index on jsonb

```csharp
migrationBuilder.CreateIndex(
    name: "ix_customers_metadata",
    table: "customers",
    column: "metadata",
    using: "GIN");
```

## Soft-Delete Interceptor

Convert `DELETE` → `UPDATE IsDeleted = true`. For sensitive/financial data where deletion history matters.

```csharp
public class SoftDeleteInterceptor : SaveChangesInterceptor
{
    public override InterceptionResult<int> SavingChanges(
        DbContextEventData eventData,
        InterceptionResult<int> result)
    {
        ApplySoftDelete(eventData.Context);
        return base.SavingChanges(eventData, result);
    }

    public override ValueTask<InterceptionResult<int>> SavingChangesAsync(
        DbContextEventData eventData,
        InterceptionResult<int> result,
        CancellationToken ct = default)
    {
        ApplySoftDelete(eventData.Context);
        return base.SavingChangesAsync(eventData, result, ct);
    }

    private static void ApplySoftDelete(DbContext? context)
    {
        if (context is null) return;

        foreach (var entry in context.ChangeTracker.Entries())
        {
            if (entry.State != EntityState.Deleted) continue;
            if (entry.Entity is not ISoftDeletable deletable) continue;

            entry.State = EntityState.Modified;
            deletable.IsDeleted = true;
            deletable.DeletedAt = DateTime.UtcNow;
        }
    }
}

public interface ISoftDeletable
{
    bool IsDeleted { get; set; }
    DateTime? DeletedAt { get; set; }
}
```

Register:
```csharp
builder.Services.AddScoped<SoftDeleteInterceptor>();
```

Apply in DbContext:
```csharp
protected override void OnConfiguring(DbContextOptionsBuilder optionsBuilder)
{
    optionsBuilder.AddInterceptors(_interceptor);
}
```

Global query filter so soft-deleted rows never leak:
```csharp
modelBuilder.Entity<Customer>().HasQueryFilter(p => !p.IsDeleted);
```

### Alternative: Global Query Filter in DbContext

Apply centrally when all entities inherit from common base. Set filter once instead of per type.

```csharp
// Shared base entity
public abstract class Entity
{
    public Guid Id { get; set; }
    public bool IsDeleted { get; set; }
    public DateTime CreatedAt { get; set; }
    public DateTime? ModifiedAt { get; set; }
}

// Centralized in DbContext
public class AppDbContext : DbContext
{
    protected void ConfigureBaseEntity<TEntity>(ModelBuilder modelBuilder)
        where TEntity : Entity
    {
        modelBuilder.Entity<TEntity>(entity =>
        {
            entity.HasKey(e => e.Id);
            entity.Property(e => e.IsDeleted).HasDefaultValue(false);
            entity.Property(e => e.CreatedAt).HasDefaultValueSql("now()");
            entity.Ignore(e => e.DomainEvents);
        });
    }

    protected void ConfigureGlobalSoftDeleteFilter(ModelBuilder modelBuilder)
    {
        foreach (var entityType in modelBuilder.Model.GetEntityTypes())
        {
            if (typeof(Entity).IsAssignableFrom(entityType.ClrType))
            {
                modelBuilder.SetSoftDeleteFilter(entityType.ClrType);
            }
        }
    }
}

public static class SoftDeleteFilterExtensions
{
    public static void SetSoftDeleteFilter(this ModelBuilder modelBuilder, Type entityType)
    {
        SetSoftDeleteFilterMethod.MakeGenericMethod(entityType)
            .Invoke(null, [modelBuilder]);
    }

    private static readonly MethodInfo SetSoftDeleteFilterMethod =
        typeof(SoftDeleteFilterExtensions).GetMethod(nameof(SetSoftDeleteFilterImpl),
            BindingFlags.Static | BindingFlags.NonPublic)!;

    private static void SetSoftDeleteFilterImpl<TEntity>(ModelBuilder modelBuilder)
        where TEntity : Entity
    {
        modelBuilder.Entity<TEntity>().HasQueryFilter(e => !e.IsDeleted);
    }
}
```

Combined with `AuditSaveChangesInterceptor` (converts `EntityState.Deleted` → `Modified`, sets `IsDeleted = true`). Together ensures devs never accidentally hard-delete.

## Audit Trail

Capture who changed what, when, old/new values.

```csharp
public class AuditInterceptor : SaveChangesInterceptor
{
    private readonly IHttpContextAccessor? _httpContextAccessor;

    public AuditInterceptor(IHttpContextAccessor? httpContextAccessor = null) =>
        _httpContextAccessor = httpContextAccessor;

    public override ValueTask<InterceptionResult<int>> SavingChangesAsync(
        DbContextEventData eventData,
        InterceptionResult<int> result,
        CancellationToken ct = default)
    {
        var context = eventData.Context;
        if (context is null) return base.SavingChangesAsync(eventData, result, ct);

        var actor = _httpContextAccessor?.HttpContext?.User.FindFirst(ClaimTypes.NameIdentifier)?.Value;
        var timestamp = DateTime.UtcNow;

        foreach (var entry in context.ChangeTracker.Entries())
        {
            if (entry.Entity is not IAuditable) continue;
            if (entry.State is EntityState.Detached or EntityState.Unchanged) continue;

            var audit = new AuditLog
            {
                Id = Guid.NewGuid(),
                EntityType = entry.Metadata.ClrType.Name,
                EntityId = entry.Property("Id").CurrentValue?.ToString(),
                Operation = entry.State.ToString(),
                Actor = actor,
                Timestamp = timestamp,
                Changes = entry.Properties
                    .Where(p => p.IsModified)
                    .ToDictionary(
                        p => p.Metadata.Name,
                        p => new ChangeRecord
                        {
                            OldValue = p.OriginalValue?.ToString(),
                            NewValue = p.CurrentValue?.ToString()
                        })
            };

            context.Set<AuditLog>().Add(audit);
        }

        return base.SavingChangesAsync(eventData, result, ct);
    }
}

public interface IAuditable { }

public class AuditLog
{
    public Guid Id { get; set; }
    public string EntityType { get; set; } = "";
    public string? EntityId { get; set; }
    public string Operation { get; set; } = "";
    public string? Actor { get; set; }
    public DateTime Timestamp { get; set; }
    public Dictionary<string, ChangeRecord> Changes { get; set; } = new();
}

public record ChangeRecord(string? OldValue, string? NewValue);
```

Store `Changes` as `jsonb` column for flexible querying.

## Full-Text Search

### tsvector/tsquery via Npgsql

```csharp
// In OnModelCreating
modelBuilder.Entity<Article>(entity =>
{
    entity.HasIndex(a => a.SearchVector).HasMethod("GIN");
});

// In migration
migrationBuilder.AddColumn<tsvector>(
    name: "search_vector",
    table: "articles",
    type: "tsvector",
    nullable: true);

migrationBuilder.Sql("""
    UPDATE articles SET search_vector = to_tsvector('english', coalesce(title, '') || ' ' || coalesce(body, ''))
    """);

migrationBuilder.Sql("""
    CREATE OR REPLACE FUNCTION articles_search_vector_update() RETURNS trigger AS $$
    BEGIN
        NEW.search_vector := to_tsvector('english', coalesce(NEW.title, '') || ' ' || coalesce(NEW.body, ''));
        RETURN NEW;
    END;
    $$ LANGUAGE plpgsql;

    CREATE TRIGGER articles_search_vector_trigger
    BEFORE INSERT OR UPDATE ON articles
    FOR EACH ROW EXECUTE FUNCTION articles_search_vector_update();
    """);
```

### NpgsqlTsVector Shadow Property

Available transitively via `Npgsql.EntityFrameworkCore.PostgreSQL`:

```csharp
modelBuilder.Entity<Article>(entity =>
{
    entity.Property<NpgsqlTsVector>("SearchVector")
        .HasComputedColumnSql(
            "to_tsvector('english', coalesce(\"DisplayName\", '') || ' ' || coalesce(\"Tags\"::text, ''))",
            stored: true);
});
```

Details:
- `Tags` is `jsonb` — cast to text with `::text`.
- `coalesce` handles NULLs preventing entire tsvector from being NULL.
- `stored: true` materializes computed column physically.

### GIN Index on tsvector

```csharp
modelBuilder.Entity<Article>(entity =>
{
    entity.HasIndex("SearchVector").HasMethod("GIN");
});
```

Creates `CREATE INDEX ix_articles_search_vector ON articles USING GIN ("SearchVector")`.

### Searching with Ranking

```csharp
var results = await db.Articles
    .Where(a => EF.Functions.ToTsVector("english", a.Title + " " + a.Body)
        .Matches(EF.Functions.PhraseToTsQuery("english", searchTerm)))
    .OrderByDescending(a => EF.Functions.ToTsVector("english", a.Title + " " + a.Body)
        .Rank(EF.Functions.PhraseToTsQuery("english", searchTerm)))
    .ToListAsync();
```

### Accent-Insensitive Search

```csharp
// In initial migration
migrationBuilder.Sql("CREATE EXTENSION IF NOT EXISTS unaccent");

// Query using unaccent
migrationBuilder.CreateIndex(
    name: "ix_customers_name_unaccent",
    table: "customers",
    column: "name",
    using: "gin",
    opclass: "gin_trgm_ops");

// In queries
var matches = await db.Customers
    .Where(p => EF.Functions.TrigramsAreSimilar(p.Name, searchTerm))
    .ToListAsync();
```

## Array Operations

### Mapping C# Lists to PostgreSQL Arrays

```csharp
modelBuilder.Entity<Customer>(entity =>
{
    entity.Property(p => p.Tags)
        .HasColumnType("text[]")
        .HasConversion(
            v => string.Join(",", v),
            v => string.IsNullOrEmpty(v) ? new List<string>() : v.Split(',', StringSplitOptions.RemoveEmptyEntries).ToList());
});
```

### Containment Queries

```csharp
// Find customers with ALL specified tags
var customers = await db.Customers
    .Where(p => p.Tags.Contains("premium") && p.Tags.Contains("vip"))
    .ToListAsync();

// Raw SQL for ANY operator
var results = await db.Customers
    .FromSqlInterpolated($"""
        SELECT * FROM customers WHERE tags && ARRAY[{tag1}, {tag2}]
        """)
    .ToListAsync();
```

### Indexing Array Columns

```csharp
migrationBuilder.CreateIndex(
    name: "ix_customers_tags",
    table: "customers",
    column: "tags",
    using: "GIN");
```

## Enum Mapping

### PostgreSQL Enum Types

```csharp
// In migration
migrationBuilder.Sql("CREATE TYPE order_status AS ENUM ('pending', 'confirmed', 'completed', 'cancelled')");

migrationBuilder.AlterColumn<OrderStatus>(
    name: "status",
    table: "orders",
    type: "order_status",
    oldType: "text");
```

```csharp
modelBuilder.Entity<Order>(entity =>
{
    entity.Property(e => e.Status)
        .HasColumnType("order_status")
        .HasConversion<string>();
});
```

### Enum Labels Must Be Lowercase

PostgreSQL requires lowercase enum labels. Map C# values using `[PgName]` attributes:

```csharp
public enum DocumentCategory
{
    [PgName("legal")]
    Legal,
    [PgName("financial")]
    Financial,
    [PgName("insurance")]
    Insurance
}

public enum InsuranceType
{
    [PgName("life")]
    Life,
    [PgName("health")]
    Health,
    [PgName("directorsofficers")]
    DirectorsOfficers
}
```

Without `[PgName]`, Npgsql lowercases PascalCase: `DirectorsOfficers` → `"directorsofficers"`. Verify generated SQL or use explicit attributes.

### String-Based HasPostgresEnum Overload

When Persistence project can't reference module assemblies, use string overload instead of typed generic:

```csharp
// Instead of: modelBuilder.HasPostgresEnum<DocumentCategory>();

// Use string-based overload:
modelBuilder.HasPostgresEnum("document_category", new[] { "legal", "financial", "insurance" });
```

Avoids coupling Persistence to domain assemblies while registering PostgreSQL enum type.

### Database-Agnostic Enum Value Converter

For codebase working with PostgreSQL and SQLite (e.g., in-memory tests):

```csharp
public class EnumValueConverter<TEnum> : ValueConverter<TEnum, string>
    where TEnum : struct, Enum
{
    public EnumValueConverter() : base(
        v => v.ToString(),
        v => Enum.Parse<TEnum>(v))
    {
    }
}

// Apply in OnModelCreating
modelBuilder.Entity<Order>(entity =>
{
    entity.Property(e => e.Status).HasConversion(new EnumValueConverter<OrderStatus>());
});
```

## Migration Management

### Idempotent Data Migrations

```csharp
protected override void Up(MigrationBuilder migrationBuilder)
{
    migrationBuilder.Sql("""
        INSERT INTO roles (id, name) VALUES (DEFAULT, 'admin')
        ON CONFLICT (name) DO NOTHING
        """);
}
```

### Seed Data with ON CONFLICT

```csharp
migrationBuilder.Sql("""
    INSERT INTO order_types (id, name, duration_minutes, is_active)
    VALUES
        (gen_random_uuid(), 'Standard', 50, true),
        (gen_random_uuid(), 'Express', 80, true),
        (gen_random_uuid(), 'Bulk', 90, true)
    ON CONFLICT DO NOTHING
    """);
```

### Migration Connection Strings from Env

```csharp
// In DesignTime factory
var connectionString = Environment.GetEnvironmentVariable("MIGRATION_CONNECTION_STRING")
    ?? config.GetConnectionString("Default");
```

### EF.Property in Composite Indexes Breaks Design-Time

Using `EF.Property<string>(e, "TenantId")` in composite index lambdas causes `dotnet ef migrations add` to fail:

```csharp
// BREAKS design-time tooling:
modelBuilder.Entity<Order>(entity =>
{
    entity.HasIndex(e => new { e.VendorId, TenantId = EF.Property<string>(e, "TenantId") });
});

// FIX: Use separate individual indexes:
modelBuilder.Entity<Order>(entity =>
{
    entity.HasIndex(e => e.VendorId);
    entity.HasIndex("TenantId"); // Shadow property index alone
});
```

Lambda with `EF.Property` uncompileable by design-time tools. Split into individual indexes — PostgreSQL can use together for composite queries.

### Advisory Lock with Deterministic ID

Use deterministic constant for advisory lock ID instead of runtime hash. All app instances use same lock value:

```csharp
public async Task MigrateWithAdvisoryLockAsync(CancellationToken ct = default)
{
    await using var db = await _dbFactory.CreateDbContextAsync(ct);

    const int MigrationLockId = 2025042001; // Deterministic constant

    await db.Database.ExecuteSqlRawAsync(
        "SELECT pg_advisory_lock(@p0)", [MigrationLockId], ct);

    try
    {
        await db.Database.MigrateAsync(ct);
    }
    finally
    {
        await db.Database.ExecuteSqlRawAsync(
            "SELECT pg_advisory_unlock(@p0)", [MigrationLockId], ct);
    }
}
```

Use single integer constant (max 2^31-1), convention: `YYMMDDNN`.

### Changing Column Types (text → jsonb)

EF Core warns about potential data loss. Review but proceed — change expected:

```csharp
migrationBuilder.AlterColumn<string>(
    name: "Metadata",
    table: "billing_subscriptions",
    type: "jsonb",
    oldType: "text");
```

PostgreSQL validates existing data during cast. Invalid JSON causes migration failure. Ensure existing data valid JSON before applying.

### Complex Data Transformations

For heavy transformations, create migration service instead of raw SQL:

```csharp
public class DataMigrationService
{
    private readonly IDbContextFactory<AppDbContext> _dbFactory;

    public async Task MigrateCustomerRecordsAsync(CancellationToken ct = default)
    {
        await using var db = await _dbFactory.CreateDbContextAsync(ct);
        var customers = await db.Customers
            .Where(p => p.LegacyData != null)
            .ToListAsync(ct);

        foreach (var customer in customers)
        {
            // Transform and save
        }

        await db.SaveChangesAsync(ct);
    }
}
```

## Testcontainers Integration

### Shared Fixture Pattern

```csharp
public class PostgresTestContainer : IAsyncLifetime
{
    private PostgreSQLContainer _container = null!;
    public string ConnectionString { get; private set; } = "";

    public async Task InitializeAsync()
    {
        _container = new PostgreSQLBuilder()
            .WithImage("postgres:16-alpine")
            .WithDatabase("testdb")
            .WithUsername("test")
            .WithPassword("test")
            .Build();

        await _container.StartAsync();
        ConnectionString = _container.GetConnectionString();
    }

    public async Task DisposeAsync() => await _container.DisposeAsync();
}
```

### Using in xUnit

```csharp
public class CustomerRepositoryTests : IClassFixture<PostgresTestContainer>
{
    private readonly PostgresTestContainer _container;

    public CustomerRepositoryTests(PostgresTestContainer container) =>
        _container = container;

    [Fact]
    public async Task Create_PersistsCustomer()
    {
        var options = new DbContextOptionsBuilder<AppDbContext>()
            .UseNpgsql(_container.ConnectionString)
            .Options;

        await using var db = new AppDbContext(options);
        await db.Database.MigrateAsync();

        var customer = new Customer { Name = "Jane Smith" };
        db.Customers.Add(customer);
        await db.SaveChangesAsync();

        var saved = await db.Customers.FindAsync(customer.Id);
        saved.ShouldNotBeNull();
        saved.Name.ShouldBe("Jane Smith");
    }
}
```

### Deterministic Seed Data

```csharp
public static class SeedData
{
    private static readonly Faker<Customer> CustomerFaker = new Faker<Customer>("en")
        .RuleFor(p => p.Name, f => f.Name.FullName())
        .RuleFor(p => p.Email, f => f.Internet.Email())
        .UseSeed(42);

    public static List<Customer> GenerateCustomers(int count = 10) =>
        CustomerFaker.Generate(count);
}
```

## Performance

### Split Queries

For multiple collections on single entity, use `AsSplitQuery` to avoid cartesian explosion:

```csharp
var store = await db.Stores
    .Include(c => c.Customers)
    .Include(c => c.Orders)
    .Include(c => c.Invoices)
    .AsSplitQuery()
    .FirstOrDefaultAsync(c => c.Id == storeId);
```

### Query Tags for Logging

```csharp
var orders = await db.Orders
    .TagWith("GetActiveOrders:DashboardWidget")
    .Where(o => o.Status == OrderStatus.Active)
    .OrderByDescending(o => o.CreatedAt)
    .Take(50)
    .ToListAsync();
```

### Compiled Queries

For repeatedly executed queries, compile once:

```csharp
private static readonly Func<AppDbContext, Guid, Task<Customer?>> GetCustomerById =
    EF.CompileAsyncQuery((AppDbContext db, Guid id) =>
        db.Customers.FirstOrDefault(p => p.Id == id));

// Usage
var customer = await GetCustomerById(db, customerId);
```

### Composite and Partial Indexes

```csharp
// Composite index
modelBuilder.Entity<Order>(entity =>
{
    entity.HasIndex(a => new { a.VendorId, a.StartTime });
});

// Partial index (only active orders)
migrationBuilder.Sql("""
    CREATE INDEX ix_orders_active
    ON orders (vendor_id, start_time)
    WHERE status = 'confirmed' AND is_deleted = false
    """);
```

### Connection Pooling

Configure Npgsql pooling in connection string. Defaults work for most cases; tune for high-throughput:

```
Pooling=true;Minimum Pool Size=5;Maximum Pool Size=100;Connection Idle Lifetime=300;Connection Pruning Interval=10;
```

### Avoid Multi-Source Concat Queries

Npgsql translator may fail on `.Concat()` chains spanning 3+ entity types. Symptoms: `InvalidOperationException: The LINQ expression could not be translated`.

**Fix — resource-ID collection pattern:** Query each type separately for IDs, then single query with `IN` filter:

```csharp
// Instead of multi-Concat (may fail):
var results = db.Patients.Where(p => p.Id == id).Select(p => new { p.Id, Type = "Patient" })
    .Concat(db.Appointments.Where(a => a.Id == id).Select(a => new { a.Id, Type = "Appointment" }))
    .Concat(db.Documents.Where(d => d.Id == id).Select(d => new { d.Id, Type = "Document" }))
    .ToList();

// Do this:
var patientIds = await db.Patients.Where(p => p.PatientId == targetId).Select(p => p.Id).ToListAsyncAsync();
var appointmentIds = await db.Appointments.Where(a => a.PatientId == targetId).Select(a => a.Id).ToListAsyncAsync();
var documentIds = await db.Documents.Where(d => d.PatientId == targetId).Select(d => d.Id).ToListAsyncAsync();

var allIds = patientIds.Concat(appointmentIds).Concat(documentIds).ToList();

var results = await db.AuditLogs
    .Where(a => allIds.Contains(a.EntityId))
    .OrderByDescending(a => a.Timestamp)
    .ToListAsyncAsync();
```

Each sub-query targets single table (optimal index use); final query is simple `IN (...)` filter. More maintainable — add entity type by adding one ID-collection line.

## Multi-Tenancy (Schema-per-Tenant)

Isolate tenant data using separate PostgreSQL schemas. Each tenant gets own schema (e.g., `tenant_acme`); shared `public` schema stores metadata.

### NuGet Packages

```xml
<ItemGroup>
  <PackageReference Include="Finbuckle.MultiTenant" />
  <PackageReference Include="Finbuckle.MultiTenant.EntityFrameworkCore" />
</ItemGroup>
```

### Schema Naming and Connection String Strategy

```csharp
public static class TenantSchemaHelper
{
    public const string SchemaPrefix = "tenant_";

    public static string GetSchemaName(string identifier)
    {
        var sanitized = SanitizeIdentifier(identifier);
        return $"{SchemaPrefix}{sanitized}";
    }

    public static string BuildConnectionString(string baseConnStr, string schemaName)
    {
        var segments = baseConnStr
            .Split(';', StringSplitOptions.RemoveEmptyEntries)
            .Where(s => !s.StartsWith("Search Path", StringComparison.OrdinalIgnoreCase)
                     && !s.StartsWith("SearchPath", StringComparison.OrdinalIgnoreCase));

        return string.Join(";", segments) + $";Search Path={schemaName}";
    }

    public static string SanitizeIdentifier(string input)
    {
        return string.Concat(input.Where(c =>
            char.IsLetterOrDigit(c) || c == '_'));
    }
}
```

### Finbuckle EF Core Store Registration

```csharp
public static class MultiTenancyServiceExtensions
{
    public static IServiceCollection AddMultiTenancyInfrastructure(
        this IServiceCollection services,
        IConfiguration configuration)
    {
        var baseConnStr = configuration.GetConnectionString("Default")
            ?? throw new InvalidOperationException("Connection string 'Default' not found.");

        // 1. Tenant store DbContext (public schema only)
        services.AddDbContext<TenantStoreDbContext>(options =>
            options.UseNpgsql(baseConnStr, npgsql =>
                npgsql.MigrationsAssembly(typeof(TenantStoreDbContext).Assembly)));

        // 2. Finbuckle with EF Core store
        services.AddMultiTenant<TenantInfo>()
            .WithEFCoreStore<TenantStoreDbContext, TenantInfo>();

        // 3. Scoped tenant data DbContext with schema-specific connection string
        services.AddScoped<AppDbContext>(sp =>
        {
            var tenantAccessor = sp.GetRequiredService<IMultiTenantContextAccessor<TenantInfo>>();
            var tenant = tenantAccessor.MultiTenantContext?.TenantInfo
                ?? throw new InvalidOperationException("Tenant not resolved.");

            var schema = TenantSchemaHelper.GetSchemaName(tenant.Identifier);
            var connStr = TenantSchemaHelper.BuildConnectionString(baseConnStr, schema);

            var options = new DbContextOptionsBuilder<AppDbContext>()
                .UseNpgsql(connStr, npgsql =>
                    npgsql.MigrationsAssembly(typeof(AppDbContext).Assembly))
                .Options;

            return new AppDbContext(options);
        });

        // 4. Migration and provisioning services
        services.AddSingleton<MultiTenantMigrationService>();
        services.AddSingleton<TenantProvisioningService>();

        return services;
    }
}
```

Tenant store DbContext maps metadata in `public` schema:

```csharp
public class TenantStoreDbContext : DbContext, IMultiTenantDbContext
{
    private readonly TenantInfo? _tenantInfo;

    public TenantStoreDbContext(DbContextOptions<TenantStoreDbContext> options)
        : base(options) { }

    public TenantStoreDbContext(
        DbContextOptions<TenantStoreDbContext> options,
        TenantInfo? tenantInfo)
        : base(options) => _tenantInfo = tenantInfo;

    public DbSet<TenantInfo> Tenants => Set<TenantInfo>();
    public TenantInfo? TenantInfo => _tenantInfo;

    protected override void OnModelCreating(ModelBuilder modelBuilder)
    {
        modelBuilder.Entity<TenantInfo>(entity =>
        {
            entity.ToTable("tenant_info", "public");
            entity.HasKey(e => e.Id);
            entity.Property(e => e.Identifier).IsRequired().HasMaxLength(100);
            entity.Property(e => e.Name).IsRequired().HasMaxLength(200);
            entity.HasIndex(e => e.Identifier).IsUnique();
        });
    }
}
```

### Per-Tenant Migration Service

Discovers tenant schemas, applies pending migrations plus `public` schema:

```csharp
public class MultiTenantMigrationService
{
    private readonly string _baseConnStr;
    private readonly ILogger<MultiTenantMigrationService> _logger;

    public MultiTenantMigrationService(
        IConfiguration configuration,
        ILogger<MultiTenantMigrationService> logger)
    {
        _baseConnStr = configuration.GetConnectionString("Default")
            ?? throw new InvalidOperationException("Connection string 'Default' not found.");
        _logger = logger;
    }

    public async Task<IReadOnlyList<string>> DiscoverTenantSchemasAsync(
        CancellationToken ct = default)
    {
        await using var connection = new NpgsqlConnection(_baseConnStr);
        await connection.OpenAsync(ct);

        await using var cmd = connection.CreateCommand();
        cmd.CommandText = $"""
            SELECT schema_name
            FROM information_schema.schemata
            WHERE schema_name LIKE '{TenantSchemaHelper.SchemaPrefix}%'
            ORDER BY schema_name
            """;

        var schemas = new List<string>();
        await using var reader = await cmd.ExecuteReaderAsync(ct);
        while (await reader.ReadAsync(ct))
        {
            schemas.Add(reader.GetString(0));
        }

        return schemas.AsReadOnly();
    }

    public async Task MigrateAllAsync(CancellationToken ct = default)
    {
        _logger.LogInformation("Migrating public schema for tenant metadata");
        await using var publicDb = CreateContextForSchema("public");
        await publicDb.Database.MigrateAsync(ct);

        var schemas = await DiscoverTenantSchemasAsync(ct);
        foreach (var schema in schemas)
        {
            _logger.LogInformation("Migrating tenant schema: {Schema}", schema);
            await MigrateSchemaAsync(schema, ct);
        }

        _logger.LogInformation("All {Count} tenant schemas migrated", schemas.Count);
    }

    public async Task MigrateSchemaAsync(string schema, CancellationToken ct = default)
    {
        await using var db = CreateContextForSchema(schema);
        await db.Database.MigrateAsync(ct);
    }

    public AppDbContext CreateContextForSchema(string schema)
    {
        var connStr = TenantSchemaHelper.BuildConnectionString(_baseConnStr, schema);
        var options = new DbContextOptionsBuilder<AppDbContext>()
            .UseNpgsql(connStr, npgsql =>
                npgsql.MigrationsAssembly(typeof(AppDbContext).Assembly))
            .Options;

        return new AppDbContext(options);
    }

    public async Task RollbackSchemaAsync(
        string schema, string targetMigration, CancellationToken ct = default)
    {
        _logger.LogWarning("Rolling back schema {Schema} to migration {Target}", schema, targetMigration);
        await using var db = CreateContextForSchema(schema);
        await db.Database.MigrateAsync(targetMigration, ct);
    }

    public async Task CreateSchemaAsync(string schema, CancellationToken ct = default)
    {
        var sanitized = TenantSchemaHelper.SanitizeIdentifier(
            schema.StartsWith(TenantSchemaHelper.SchemaPrefix)
                ? schema[TenantSchemaHelper.SchemaPrefix.Length..]
                : schema);
        var fullSchemaName = TenantSchemaHelper.GetSchemaName(sanitized);

        await using var connection = new NpgsqlConnection(_baseConnStr);
        await connection.OpenAsync(ct);

        await using var cmd = connection.CreateCommand();
        cmd.CommandText = $"CREATE SCHEMA IF NOT EXISTS \"{fullSchemaName}\"";
        await cmd.ExecuteNonQueryAsync(ct);

        _logger.LogInformation("Created schema: {Schema}", fullSchemaName);
    }

    public async Task DropSchemaAsync(string schema, CancellationToken ct = default)
    {
        var fullSchemaName = schema.StartsWith(TenantSchemaHelper.SchemaPrefix)
            ? schema
            : TenantSchemaHelper.GetSchemaName(schema);

        await using var connection = new NpgsqlConnection(_baseConnStr);
        await connection.OpenAsync(ct);

        await using var cmd = connection.CreateCommand();
        cmd.CommandText = $"DROP SCHEMA IF EXISTS \"{fullSchemaName}\" CASCADE";
        await cmd.ExecuteNonQueryAsync(ct);

        _logger.LogWarning("Dropped schema: {Schema}", fullSchemaName);
    }
}
```

Provisioning service wires schema creation with tenant metadata:

```csharp
public class TenantProvisioningService
{
    private readonly MultiTenantMigrationService _migrationService;
    private readonly IDbContextFactory<TenantStoreDbContext> _storeFactory;
    private readonly ILogger<TenantProvisioningService> _logger;

    public TenantProvisioningService(
        MultiTenantMigrationService migrationService,
        IDbContextFactory<TenantStoreDbContext> storeFactory,
        ILogger<TenantProvisioningService> logger)
    {
        _migrationService = migrationService;
        _storeFactory = storeFactory;
        _logger = logger;
    }

    public async Task ProvisionAsync(
        string identifier, string name, CancellationToken ct = default)
    {
        var schema = TenantSchemaHelper.GetSchemaName(identifier);

        await _migrationService.CreateSchemaAsync(schema, ct);
        await _migrationService.MigrateSchemaAsync(schema, ct);

        await using var db = await _storeFactory.CreateDbContextAsync(ct);
        var tenant = new TenantInfo
        {
            Id = Guid.NewGuid().ToString(),
            Identifier = identifier,
            Name = name
        };

        db.Tenants.Add(tenant);
        await db.SaveChangesAsync(ct);

        _logger.LogInformation("Provisioned tenant {Name} with schema {Schema}", name, schema);
    }

    public async Task DeprovisionAsync(string identifier, CancellationToken ct = default)
    {
        var schema = TenantSchemaHelper.GetSchemaName(identifier);

        await _migrationService.DropSchemaAsync(schema, ct);

        await using var db = await _storeFactory.CreateDbContextAsync(ct);
        var tenant = await db.Tenants
            .FirstOrDefaultAsync(t => t.Identifier == identifier, ct);

        if (tenant is not null)
        {
            db.Tenants.Remove(tenant);
            await db.SaveChangesAsync(ct);
        }

        _logger.LogWarning("Deprovisioned tenant {Identifier}", identifier);
    }
}
```

### Tenant Resolution Strategies

#### Cookie Strategy (Blazor Server)

```csharp
services.AddMultiTenant<TenantInfo>()
    .WithEFCoreStore<TenantStoreDbContext, TenantInfo>()
    .WithCookieStrategy(options =>
    {
        options.CookieName = "__tenant__";
    });

app.UseMultiTenant();
```

#### Header Strategy (API)

```csharp
services.AddMultiTenant<TenantInfo>()
    .WithEFCoreStore<TenantStoreDbContext, TenantInfo>()
    .WithHeaderStrategy(options =>
    {
        options.TenantKey = "X-Tenant-Id";
    });

app.UseMultiTenant();
```

Both look up tenant identifier in EF Core store to resolve full `TenantInfo`. If not found, `MultiTenantContext.TenantInfo` set to `null`.

### Tenant-Aware Request Propagation

```csharp
public interface ITenantAware
{
    string? TenantId { get; set; }
}
```

Wolverine middleware:
```csharp
public class TenantMiddleware<TRequest>
{
    private readonly IMultiTenantContextAccessor<TenantInfo> _tenantAccessor;

    public TenantMiddleware(IMultiTenantContextAccessor<TenantInfo> tenantAccessor)
    {
        _tenantAccessor = tenantAccessor;
    }

    public void Handle(TRequest request)
    {
        if (request is not ITenantAware tenantAware)
            return;

        var tenantInfo = _tenantAccessor.MultiTenantContext?.TenantInfo;
        if (tenantInfo is null)
            throw new InvalidOperationException(
                $"Tenant not resolved for request {typeof(TRequest).Name}. " +
                "Ensure tenant resolution middleware runs before this handler.");

        tenantAware.TenantId = tenantInfo.Identifier;
    }
}
```

Register in Wolverine pipeline:
```csharp
builder.Host.UseWolverine(options =>
{
    options.Policies.ForMessagesThatImplement<ITenantAware>()
        .AddMiddleware<TenantMiddleware<IMessage>>();
});
```

Or MediatR pipeline behavior:
```csharp
builder.Services.AddScoped(
    typeof(IPipelineBehavior<,>),
    typeof(TenantPropagationBehavior<,>));

public class TenantPropagationBehavior<TRequest, TResponse>
    : IPipelineBehavior<TRequest, TResponse>
    where TRequest : notnull
{
    private readonly IMultiTenantContextAccessor<TenantInfo> _tenantAccessor;

    public TenantPropagationBehavior(
        IMultiTenantContextAccessor<TenantInfo> tenantAccessor)
    {
        _tenantAccessor = tenantAccessor;
    }

    public async Task<TResponse> Handle(
        TRequest request,
        RequestHandlerDelegate<TResponse> next,
        CancellationToken ct)
    {
        if (request is ITenantAware tenantAware)
        {
            var tenantInfo = _tenantAccessor.MultiTenantContext?.TenantInfo
                ?? throw new InvalidOperationException("Tenant not resolved.");

            tenantAware.TenantId = tenantInfo.Identifier;
        }

        return await next();
    }
}
```

### Centralized Base Entity Configuration

```csharp
protected void ConfigureBaseEntity<TEntity>(ModelBuilder modelBuilder)
    where TEntity : Entity
{
    modelBuilder.Entity<TEntity>(entity =>
    {
        entity.HasKey(e => e.Id);
        entity.IsMultiTenant(); // Single centralized call for all entities
        entity.Property(e => e.IsDeleted).HasDefaultValue(false);
        entity.Property(e => e.CreatedAt).HasDefaultValueSql("now()");
        entity.Property(e => e.ModifiedAt);
        entity.Ignore(e => e.DomainEvents);
    });
}

// In OnModelCreating:
ConfigureBaseEntity<Customer>(modelBuilder);
ConfigureBaseEntity<Order>(modelBuilder);
ConfigureBaseEntity<Product>(modelBuilder);
// NOT for shared/reference entities like Country or Currency
```

Critical: only call `ConfigureBaseEntity<T>()` for tenant-owned entities. Shared/reference data (countries, currencies, lookups) configured separately without `IsMultiTenant()`.

## Anti-Patterns

**Never use scoped DbContext.** Always inject `IDbContextFactory<AppDbContext>`, call `CreateDbContextAsync()`. Scoped DbContext in background services, SignalR hubs, or parallel code causes threading issues + connection exhaustion.

**Never use `SET search_path` via raw SQL.** Use connection string `Search Path=` parameter. Raw `SET` commands leak across pooled connections — another tenant's query on recycled connection may see wrong schema.

**Never trust tenant identifiers without sanitization.** Unsanitized identifiers allow SQL injection in schema names. Strip non-alphanumeric chars via `TenantSchemaHelper.SanitizeIdentifier`.

**Never share single DbContext across tenants.** Each tenant needs own DbContext with schema-specific connection string. Scoped registration ensures one per request.

**Never run migrations against public schema with schema-per-tenant.** Public schema holds only tenant metadata. Business tables belong in tenant schemas.

**Never use InMemory provider for integration tests.** InMemory doesn't enforce relational constraints, data types, or PostgreSQL-specific behavior. Use Testcontainers with real PostgreSQL.

**Never modify applied migrations.** If migration applied to any environment, create new migration to fix. Modifying breaks `__EFMigrationsHistory` and deployments.

**Never skip indexes on foreign keys.** EF Core doesn't create FK indexes by default. Every FK participating in queries needs explicit index.

```csharp
// Wrong: no index on FK
modelBuilder.Entity<Order>()
    .HasOne(a => a.Vendor)
    .WithMany(p => p.Orders);

// Right: explicit index
modelBuilder.Entity<Order>(entity =>
{
    entity.HasOne(a => a.Vendor)
        .WithMany(p => p.Orders);
    entity.HasIndex(a => a.VendorId);
});
```

**Never use SELECT * in production.** Always project needed columns via `Select()`. Reduces payload size and memory allocation.

```csharp
// Avoid: returns all columns
var customers = await db.Customers.ToListAsync();

// Prefer: project only what you need
var summaries = await db.Customers
    .Select(p => new CustomerSummary { Id = p.Id, Name = p.Name, Email = p.Email })
    .ToListAsync();
```

**NEVER call `IsMultiTenant()` per-entity with centralized base config.** When `ConfigureBaseEntity<T>()` calls `entity.IsMultiTenant()` once, adding it per-entity creates duplicate query filters + runtime errors. Shared entities (reference data) spanning tenants must NOT call `IsMultiTenant()`.

## Related Skills

- **dotnet-clean-arch** — Project structure, CQRS handlers, DI registration with `IDbContextFactory`
- **lgpd-data-privacy** — Encryption at rest, key rotation, audit trail patterns depending on EF Core interceptors
- **aspnetcore-health** — Health checks verifying DB connectivity
- **quartz-scheduling** — Background jobs using `IDbContextFactory` for batch processing
- **keycloak-oidc** — OIDC tenant resolution and multi-scheme auth for multi-tenant apps
- **distributed-caching** — Cache key namespacing for multi-tenant isolation