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

Production-ready patterns for .NET applications backed by PostgreSQL and Entity Framework Core via the Npgsql provider.

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

Register the factory in DI. Inject `IDbContextFactory<AppDbContext>` everywhere. Never inject `AppDbContext` directly as scoped.

```csharp
// Program.cs
builder.Services.AddDbContextFactory<AppDbContext>(options =>
    options.UseNpgsql(
        builder.Configuration.GetConnectionString("Default"),
        npgsql => npgsql.MigrationsAssembly(typeof(AppDbContext).Assembly)
    ));

// Usage in a service
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

Register `IDbContextFactory<AppDbContext>` via `AddDbContextFactory` when a concurrent migration coordination service needs to create its own context instances independently of scoped DI:

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

The factory pattern is essential here because the service needs to create short-lived contexts for lock acquisition and migration execution without relying on request-scoped DI.

### Design-Time Factory for dotnet ef CLI

EF Core tools need a factory that reads the connection string without the full host pipeline.

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

Enable pgcrypto in the initial migration so all subsequent migrations can reference its functions.

```csharp
// In your first migration's Up method
protected override void Up(MigrationBuilder migrationBuilder)
{
    migrationBuilder.Sql("CREATE EXTENSION IF NOT EXISTS pgcrypto");
}
```

### AES-256 Encrypt/Decrypt via Value Converter

Store sensitive fields as `bytea`. The encryption happens at the application layer using Npgsql's built-in `pgcrypto` functions.

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
// Query a nested field
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
// In migration
migrationBuilder.CreateIndex(
    name: "ix_customers_metadata",
    table: "customers",
    column: "metadata",
    using: "GIN");
```

## Soft-Delete Interceptor

Convert `DELETE` operations into `UPDATE IsDeleted = true`. Use this for sensitive, financial, or any data where deletion history matters.

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

Register it in DI:

```csharp
builder.Services.AddScoped<SoftDeleteInterceptor>();
```

Then apply in DbContext:

```csharp
protected override void OnConfiguring(DbContextOptionsBuilder optionsBuilder)
{
    optionsBuilder.AddInterceptors(_interceptor);
}
```

Add a global query filter so soft-deleted rows never leak:

```csharp
modelBuilder.Entity<Customer>().HasQueryFilter(p => !p.IsDeleted);
```

### Alternative: Global Query Filter in DbContext

Instead of (or in addition to) an interceptor, you can apply soft-delete filtering centrally in the DbContext via a global query filter. This approach works well when all entities inherit from a common base, because you set the filter once instead of repeating it per entity type.

```csharp
// Shared base entity
public abstract class Entity
{
    public Guid Id { get; set; }
    public bool IsDeleted { get; set; }
    public DateTime CreatedAt { get; set; }
    public DateTime? ModifiedAt { get; set; }
}

// Centralized configuration in DbContext
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

    /// <summary>
    /// Applies a global query filter for soft-delete on all Entity-derived types.
    /// Call this once in OnModelCreating after all entity registrations.
    /// </summary>
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

// Extension method to set the filter generically
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

This pattern is typically combined with an `AuditSaveChangesInterceptor` that converts `EntityState.Deleted` to `EntityState.Modified` and sets `IsDeleted = true`. Together they ensure developers never accidentally hard-delete a row: the interceptor flips the state on write, and the query filter hides deleted rows on read.

## Audit Trail

Capture who changed what, when, and the old and new values.

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

Store `Changes` as a `jsonb` column in the migration for flexible querying.

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

`NpgsqlTsVector` is available transitively via `Npgsql.EntityFrameworkCore.PostgreSQL` — no separate Npgsql package reference needed for the type:

```csharp
modelBuilder.Entity<Article>(entity =>
{
    entity.Property<NpgsqlTsVector>("SearchVector")
        .HasComputedColumnSql(
            "to_tsvector('english', coalesce(\"DisplayName\", '') || ' ' || coalesce(\"Tags\"::text, ''))",
            stored: true);
});
```

Key details:
- `Tags` is `jsonb` so must cast to text first with `::text`.
- `coalesce` handles NULL values to prevent the entire `tsvector` from being NULL.
- Setting `stored: true` materializes the computed column so it persists physically.

### GIN Index on tsvector

GIN index is required for performant full-text search on tsvector columns:

```csharp
modelBuilder.Entity<Article>(entity =>
{
    entity.HasIndex("SearchVector").HasMethod("GIN");
});
```

This creates `CREATE INDEX ix_articles_search_vector ON articles USING GIN ("SearchVector")`.

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
// Find customers who have ALL specified tags
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

PostgreSQL convention requires lowercase enum labels. Map C# enum values to lowercase PostgreSQL labels using `[PgName]` attributes:

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

Without `[PgName]`, the default Npgsql convention lowercases PascalCase names: `DirectorsOfficers` → `"directorsofficers"`. Always verify by checking the generated SQL or using explicit attribute configuration.

### String-Based HasPostgresEnum Overload

When the Persistence project cannot reference module assemblies (to avoid circular dependencies), use the string-based `HasPostgresEnum` overload instead of the typed generic version:

```csharp
// Instead of this (requires compile-time enum type reference):
// modelBuilder.HasPostgresEnum<DocumentCategory>();

// Use string-based overload:
modelBuilder.HasPostgresEnum("document_category", new[] { "legal", "financial", "insurance" });
```

The string overload accepts the enum name and label values as plain strings. This avoids coupling the Persistence project to domain assemblies while still registering the PostgreSQL enum type.

### Database-Agnostic Enum Value Converter

When you need the same codebase to work with PostgreSQL and SQLite (e.g., in-memory tests), map enums as strings in EF Core but let PostgreSQL use its native type.

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

### EF.Property in Composite Indexes Breaks Design-Time Tooling

Using `EF.Property<string>(e, "TenantId")` inside composite index lambda expressions causes `dotnet ef migrations add` and other design-time tools to fail:

```csharp
// BREAKS design-time tooling:
modelBuilder.Entity<Order>(entity =>
{
    entity.HasIndex(e => new { e.VendorId, TenantId = EF.Property<string>(e, "TenantId") });
});

// FIX: Use separate individual indexes instead:
modelBuilder.Entity<Order>(entity =>
{
    entity.HasIndex(e => e.VendorId);
    entity.HasIndex("TenantId"); // Shadow property index alone
});
```

The lambda expression with `EF.Property` cannot be compiled by design-time EF Core tools. Split into individual indexes — PostgreSQL can still use them together for composite queries.

### Advisory Lock with Deterministic ID

Use a deterministic constant for the advisory lock ID instead of a runtime hash. This ensures all application instances use the same lock value for migration coordination:

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

Use a single integer constant (max 2^31-1) that won't conflict with other pg_advisory_lock usage in the system. Convention: `YYMMDDNN` (year, month, day, sequence).

### Changing Column Types (text → jsonb)

EF Core migrations warn about potential data loss when changing column types. Review warnings but proceed — the change is expected:

```csharp
migrationBuilder.AlterColumn<string>(
    name: "Metadata",
    table: "billing_subscriptions",
    type: "jsonb",
    oldType: "text");
```

This pattern converts a `text` column to `jsonb`. PostgreSQL validates existing data during the cast — invalid JSON causes the migration to fail. Always ensure existing data is valid JSON before applying this migration.

### Complex Data Transformations

For heavy data transformations, don't write raw SQL in migrations. Create a migration service instead.

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

When you have multiple collections on a single entity, use `AsSplitQuery` to avoid the cartesian explosion problem.

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

For queries that execute repeatedly with different parameters, compile them once.

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

Configure Npgsql pooling in the connection string. The defaults work for most cases, but tune for high-throughput scenarios.

```
Pooling=true;Minimum Pool Size=5;Maximum Pool Size=100;Connection Idle Lifetime=300;Connection Pruning Interval=10;
```

### Avoid Multi-Source Concat Queries

Npgsql's EF Core query translator may fail on `.Concat()` chains that span 3+ different entity types. PostgreSQL supports `UNION ALL` natively, but the Npgsql translator struggles when each branch projects from a different table with different column mappings into a shared anonymous type.

**Symptoms**: `InvalidOperationException: The LINQ expression could not be translated` or unexpected runtime exceptions on complex multi-entity `.Concat()` chains.

**Solution — resource-ID collection pattern**: Query each entity type separately to collect IDs, then execute a single query with an `IN` filter:

```csharp
// Instead of this (may fail):
var results = db.Patients.Where(p => p.Id == id).Select(p => new { p.Id, Type = "Patient" })
    .Concat(db.Appointments.Where(a => a.Id == id).Select(a => new { a.Id, Type = "Appointment" }))
    .Concat(db.Documents.Where(d => d.Id == id).Select(d => new { d.Id, Type = "Document" }))
    // ... more branches
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

Each sub-query targets a single table (optimal index usage), and the final query is a simple `IN (...)` filter. This is also more maintainable — adding a new entity type means adding one ID-collection line instead of modifying a complex Concat chain.

## Multi-Tenancy (Schema-per-Tenant)

Isolate tenant data using separate PostgreSQL schemas within a single database. Each tenant gets its own schema (e.g., `tenant_acme`) while a shared `public` schema stores tenant metadata. This approach balances strong isolation with operational simplicity—no separate databases to manage, but full query-level separation.

### NuGet Packages

```xml
<ItemGroup>
  <PackageReference Include="Finbuckle.MultiTenant" />
  <PackageReference Include="Finbuckle.MultiTenant.EntityFrameworkCore" />
</ItemGroup>
```

### Schema Naming and Connection String Strategy

A static helper class centralizes schema name generation and connection string manipulation. Every schema name is prefixed and sanitized to prevent SQL injection.

```csharp
public static class TenantSchemaHelper
{
    public const string SchemaPrefix = "tenant_";

    /// <summary>
    /// Builds a fully qualified schema name from a tenant identifier.
    /// Result is always prefixed and sanitized.
    /// </summary>
    public static string GetSchemaName(string identifier)
    {
        var sanitized = SanitizeIdentifier(identifier);
        return $"{SchemaPrefix}{sanitized}";
    }

    /// <summary>
    /// Builds a connection string with the tenant's schema set as the search path.
    /// Removes any existing Search Path parameter to avoid conflicts.
    /// </summary>
    public static string BuildConnectionString(string baseConnStr, string schemaName)
    {
        // Remove existing Search Path parameter if present
        var segments = baseConnStr
            .Split(';', StringSplitOptions.RemoveEmptyEntries)
            .Where(s => !s.StartsWith("Search Path", StringComparison.OrdinalIgnoreCase)
                     && !s.StartsWith("SearchPath", StringComparison.OrdinalIgnoreCase));

        return string.Join(";", segments) + $";Search Path={schemaName}";
    }

    /// <summary>
    /// Strips all characters except letters, digits, and underscores.
    /// Prevents SQL injection through tenant identifiers used in schema names.
    /// </summary>
    public static string SanitizeIdentifier(string input)
    {
        return string.Concat(input.Where(c =>
            char.IsLetterOrDigit(c) || c == '_'));
    }
}
```

### Finbuckle EF Core Store Registration

Register Finbuckle's multi-tenant infrastructure with an EF Core-backed tenant store. The `TenantStoreDbContext` lives in the `public` schema and holds tenant metadata. The application `DbContext` is registered per-request with a schema-specific connection string.

```csharp
public static class MultiTenancyServiceExtensions
{
    public static IServiceCollection AddMultiTenancyInfrastructure(
        this IServiceCollection services,
        IConfiguration configuration)
    {
        var baseConnStr = configuration.GetConnectionString("Default")
            ?? throw new InvalidOperationException("Connection string 'Default' not found.");

        // 1. Register the tenant store DbContext (public schema only)
        services.AddDbContext<TenantStoreDbContext>(options =>
            options.UseNpgsql(baseConnStr, npgsql =>
                npgsql.MigrationsAssembly(typeof(TenantStoreDbContext).Assembly)));

        // 2. Register Finbuckle with EF Core store
        services.AddMultiTenant<TenantInfo>()
            .WithEFCoreStore<TenantStoreDbContext, TenantInfo>();

        // 3. Register scoped tenant data DbContext with schema-specific connection string
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

        // 4. Register migration and provisioning services
        services.AddSingleton<MultiTenantMigrationService>();
        services.AddSingleton<TenantProvisioningService>();

        return services;
    }
}
```

The tenant store DbContext maps tenant metadata in the `public` schema:

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

Migrations must run against each tenant schema individually. This service discovers all tenant schemas and applies pending migrations to each one, plus the `public` schema for tenant metadata.

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

    /// <summary>
    /// Discovers all tenant schemas matching the configured prefix pattern.
    /// </summary>
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

    /// <summary>
    /// Applies pending migrations to all tenant schemas and the public schema.
    /// </summary>
    public async Task MigrateAllAsync(CancellationToken ct = default)
    {
        // Migrate public schema (tenant metadata)
        _logger.LogInformation("Migrating public schema for tenant metadata");
        await using var publicDb = CreateContextForSchema("public");
        await publicDb.Database.MigrateAsync(ct);

        // Migrate each tenant schema
        var schemas = await DiscoverTenantSchemasAsync(ct);
        foreach (var schema in schemas)
        {
            _logger.LogInformation("Migrating tenant schema: {Schema}", schema);
            await MigrateSchemaAsync(schema, ct);
        }

        _logger.LogInformation("All {Count} tenant schemas migrated", schemas.Count);
    }

    /// <summary>
    /// Applies pending migrations to a single tenant schema.
    /// </summary>
    public async Task MigrateSchemaAsync(string schema, CancellationToken ct = default)
    {
        await using var db = CreateContextForSchema(schema);
        await db.Database.MigrateAsync(ct);
    }

    /// <summary>
    /// Creates a DbContext instance targeting the specified schema via search_path.
    /// </summary>
    public AppDbContext CreateContextForSchema(string schema)
    {
        var connStr = TenantSchemaHelper.BuildConnectionString(_baseConnStr, schema);
        var options = new DbContextOptionsBuilder<AppDbContext>()
            .UseNpgsql(connStr, npgsql =>
                npgsql.MigrationsAssembly(typeof(AppDbContext).Assembly))
            .Options;

        return new AppDbContext(options);
    }

    /// <summary>
    /// Rolls back all migrations on a specific tenant schema to a target migration.
    /// </summary>
    public async Task RollbackSchemaAsync(
        string schema,
        string targetMigration,
        CancellationToken ct = default)
    {
        _logger.LogWarning("Rolling back schema {Schema} to migration {Target}", schema, targetMigration);
        await using var db = CreateContextForSchema(schema);
        await db.Database.MigrateAsync(targetMigration, ct);
    }

    /// <summary>
    /// Creates a new schema for a tenant. Does NOT run migrations—
    /// call MigrateSchemaAsync after provisioning.
    /// </summary>
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

    /// <summary>
    /// Drops a tenant schema and all its data. Irreversible.
    /// </summary>
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

A provisioning service wires schema creation with tenant metadata:

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
        string identifier,
        string name,
        CancellationToken ct = default)
    {
        var schema = TenantSchemaHelper.GetSchemaName(identifier);

        // Create the schema
        await _migrationService.CreateSchemaAsync(schema, ct);

        // Run migrations against the new schema
        await _migrationService.MigrateSchemaAsync(schema, ct);

        // Register the tenant in the store
        await using var db = await _storeFactory.CreateDbContextAsync(ct);
        var tenant = new TenantInfo
        {
            Id = Guid.NewGuid().ToString(),
            Identifier = identifier,
            Name = name
        };

        db.Tenants.Add(tenant);
        await db.SaveChangesAsync(ct);

        _logger.LogInformation(
            "Provisioned tenant {Name} with schema {Schema}", name, schema);
    }

    public async Task DeprovisionAsync(
        string identifier,
        CancellationToken ct = default)
    {
        var schema = TenantSchemaHelper.GetSchemaName(identifier);

        // Drop the schema and all data
        await _migrationService.DropSchemaAsync(schema, ct);

        // Remove from tenant store
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

Finbuckle supports multiple resolution strategies. The two most common patterns for web applications:

#### Cookie Strategy (Blazor Server)

Reads the `__tenant__` cookie to determine the current tenant. Ideal for Blazor Server apps where the browser persists the selection across requests.

```csharp
// Program.cs — after AddMultiTenant<TenantInfo>()
services.AddMultiTenant<TenantInfo>()
    .WithEFCoreStore<TenantStoreDbContext, TenantInfo>()
    .WithCookieStrategy(options =>
    {
        options.CookieName = "__tenant__";
    });

// In middleware pipeline, AFTER UseAuthentication and UseAuthorization
app.UseMultiTenant();
```

#### Header Strategy (API)

Reads the `X-Tenant-Id` header. Suitable for API-to-API calls where a cookie is not available.

```csharp
// Program.cs
services.AddMultiTenant<TenantInfo>()
    .WithEFCoreStore<TenantStoreDbContext, TenantInfo>()
    .WithHeaderStrategy(options =>
    {
        options.TenantKey = "X-Tenant-Id";
    });

app.UseMultiTenant();
```

Both strategies look up the tenant identifier in the EF Core store to resolve the full `TenantInfo` record. If the identifier is not found, Finbuckle sets `MultiTenantContext.TenantInfo` to `null`.

### Tenant-Aware Request Propagation

When using CQRS or message-based patterns, propagate the resolved tenant identifier into request objects automatically via middleware.

```csharp
/// <summary>
/// Marker interface for requests that carry tenant context.
/// Middleware sets TenantId before the handler executes.
/// </summary>
public interface ITenantAware
{
    string? TenantId { get; set; }
}
```

```csharp
/// <summary>
/// Wolverine or MediatR middleware that copies the resolved tenant
/// identifier into any request implementing ITenantAware.
/// </summary>
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

Register the middleware in the Wolverine pipeline:

```csharp
builder.Host.UseWolverine(options =>
{
    options.Policies.ForMessagesThatImplement<ITenantAware>()
        .AddMiddleware<TenantMiddleware<IMessage>>();
});
```

Or with MediatR pipeline behaviors:

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

When all entities inherit from a common base, you can call `IsMultiTenant()` once inside a shared configuration method instead of repeating it in every entity's `OnModelCreating` setup. This keeps tenant filtering DRY and makes it impossible to forget the call on a new entity.

```csharp
// Centralized base entity pattern - avoids per-entity IsMultiTenant() calls
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
// ... but NOT for shared/reference entities like Country or Currency
```

The critical detail: only call `ConfigureBaseEntity<T>()` for entities that belong to tenants. Shared or reference data (countries, currencies, lookup tables, system-wide categories) must be configured separately without the `IsMultiTenant()` call, so they remain visible across all tenants.

## Anti-Patterns

**Never use scoped DbContext.** Always inject `IDbContextFactory<AppDbContext>` and call `CreateDbContextAsync()`. Scoped DbContext in background services, SignalR hubs, or parallel code leads to threading issues and connection exhaustion.

**Never use `SET search_path` via raw SQL commands.** Use the connection string `Search Path=` parameter instead. Raw `SET` commands leak across pooled connections—another tenant's query on a recycled connection may see the wrong schema.

**Never trust tenant identifiers without sanitization.** Unsanitized identifiers allow SQL injection in schema names. Always strip non-alphanumeric characters using a helper like `TenantSchemaHelper.SanitizeIdentifier`.

**Never share a single DbContext across tenants.** Each tenant needs its own DbContext instance with a schema-specific connection string. Scoped registration ensures one context per request. A shared context would write data to whichever schema happened to be in the search path.

**Never run migrations against the public schema when using schema-per-tenant.** The public schema holds only tenant metadata. Business tables belong in tenant schemas. Running business migrations against public mixes tenant data into a shared space.

**Never use InMemory provider for integration tests.** InMemory does not enforce relational constraints, data types, or PostgreSQL-specific behavior. Use Testcontainers with a real PostgreSQL instance.

**Never modify applied migrations.** If a migration has been applied to any environment, leave it alone. Create a new migration to fix the problem. Modifying applied migrations breaks the `__EFMigrationsHistory` table and makes deployments unreliable.

**Never skip indexes on foreign keys.** EF Core does not create indexes on FK columns by default. Every FK that participates in queries needs an explicit index.

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

**Never use SELECT * in production queries.** Always project to the columns you need. EF Core's `Select()` maps only the requested fields and reduces payload size and memory allocation.

```csharp
// Avoid: returns all columns
var customers = await db.Customers.ToListAsync();

// Prefer: project only what you need
var summaries = await db.Customers
    .Select(p => new CustomerSummary { Id = p.Id, Name = p.Name, Email = p.Email })
    .ToListAsync();
```

**NEVER call `IsMultiTenant()` per-entity when using a centralized base entity configuration.** When all entities inherit from a common base and `ConfigureBaseEntity<T>()` calls `entity.IsMultiTenant()` once, adding `IsMultiTenant()` in individual entity configurations creates duplicate query filters and runtime errors. Shared entities (e.g., reference data like Country or Currency) that intentionally span tenants must NOT call `IsMultiTenant()` at all.

## Related Skills

- **dotnet-clean-arch** — Project structure, CQRS handlers, and DI registration patterns that use `IDbContextFactory`
- **lgpd-data-privacy** — Encryption at rest, key rotation, and audit trail patterns that depend on EF Core interceptors
- **aspnetcore-health** — Health checks that verify database connectivity (see `AddNpgSql` check)
- **quartz-scheduling** — Background jobs that use `IDbContextFactory` for batch data processing
- **keycloak-oidc** — OIDC tenant resolution and multi-scheme authentication for multi-tenant applications
- **distributed-caching** — Cache key namespacing strategies for multi-tenant isolation
