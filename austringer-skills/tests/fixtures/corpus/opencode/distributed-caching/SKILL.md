---
name: distributed-caching
description: "Distributed caching patterns for ASP.NET Core. IDistributedCache, Redis/Dragonfly cache-aside, cache invalidation strategies, dependency injection, serialization, cache tags, sliding/absolute expiration, concurrent access, stale-while-revalidate, cache stampede prevention, multi-layer caching. USE FOR: session state, output caching, API response caching, rate limiting counters, distributed locks, frequently-read data, reduction of database load. DO NOT USE FOR: in-memory-only caching (use IMemoryCache), CDN caching, browser caching, static file caching."
license: MIT
metadata:
  author: community
  version: "1.0.0"
  frameworks: [".NET 8+", "Redis", "Dragonfly"]
---

# Distributed Caching Patterns for ASP.NET Core
## 1. IDistributedCache Interface

`IDistributedCache` in `Microsoft.Extensions.Caching.Distributed`. `byte[]` operations. String overloads via extensions.

```csharp
public class ProductService(IDistributedCache cache, ProductRepository db)
{
    private static readonly JsonSerializerOptions _json = new() { PropertyNamingPolicy = JsonNamingPolicy.CamelCase };

    public async Task<Product?> GetByIdAsync(Guid id)
    {
        var key = $"product:{id}";
        if (await cache.GetStringAsync(key) is { } json)
            return JsonSerializer.Deserialize<Product>(json, _json);
        var product = await db.GetByIdAsync(id);
        if (product is null) return null;
        await cache.SetStringAsync(key, JsonSerializer.Serialize(product, _json));
        return product;
    }

    public async Task RemoveAsync(Guid id) => await cache.RemoveAsync($"product:{id}");
}
```

Methods: `GetAsync`, `SetAsync`, `RemoveAsync`, `GetStringAsync`, `SetStringAsync`, `RefreshAsync` (resets sliding).
## 2. Redis and Dragonfly Setup
```csharp
// Program.cs
builder.Services.AddStackExchangeRedisCache(options =>
{
    options.Configuration = builder.Configuration.GetConnectionString("Redis");
    options.InstanceName = "myapp:";
});
```
```yaml
# docker-compose.yml
services:
  redis:
    image: redis:7-alpine
    command: redis-server --maxmemory 256mb --maxmemory-policy allkeys-lru
    ports: ["6379:6379"]
  dragonfly:
    image: docker.dragonflydb.io/dragonflydb/dragonfly
    command: --maxmemory 256mb --cache_mode
    ports: ["6379:6379"]
```

Strings: `localhost:6379,abortConnect=false` (plain), `localhost:6379,password=key` (auth). Dragonfly = drop-in. `StackExchange.Redis` multiplexes.

### ConnectionMultiplexerFactory (Async in v10+)

`RedisCacheOptions.ConnectionMultiplexerFactory` v10.0.6 = `Func<Task<IConnectionMultiplexer>>?` (NOT sync). Wrap in `Task.FromResult<IConnectionMultiplexer>()`:

```csharp
builder.Services.AddStackExchangeRedisCache(options =>
{
    options.ConnectionMultiplexerFactory = () => Task.FromResult<IConnectionMultiplexer>(sharedMultiplexer);
    options.InstanceName = "myapp:";
});
```

SignalR `RedisOptions.ConnectionFactory` = `Func<TextWriter, Task<IConnectionMultiplexer>>?`:

```csharp
builder.Services.AddSignalR().AddStackExchangeRedis(options =>
{
    options.ConnectionFactory = async _ => sharedMultiplexer;
});
```

### PostConfigure for Shared Multiplexer

`PostConfigure<RedisOptions>` runs **AFTER** all `Configure` calls, overrides `ConnectionFactory` even if registered before `AddStackExchangeRedis()`. Share one `IConnectionMultiplexer` between cache + SignalR:

```csharp
builder.Services.AddSingleton<IConnectionMultiplexer>(sp =>
    ConnectionMultiplexer.Connect(connectionString));

builder.Services.PostConfigure<RedisOptions>(options =>
{
    options.ConnectionFactory = async _ =>
        sp.GetRequiredService<IConnectionMultiplexer>();
});
```

`ChannelPrefix` on `ConfigurationOptions` affects pub/sub only (SignalR), not `IDistributedCache`. Safe on shared multiplexer.

### Registration Ordering

`AddDistributedCache()` before module registrations so all inject `IDistributedCache`. Also `AddMemoryCache()` for L1.

### DataProtection Key Ring Sharing

Multiple instances generate own keys. Without shared keys: Instance A cookie → Instance B can't decrypt → auth failures.

```csharp
var redisConnection = builder.Configuration.GetConnectionString("Redis") ?? "localhost:6379";
var multiplexer = ConnectionMultiplexer.Connect(redisConnection);

builder.Services.AddDataProtection()
    .PersistKeysToStackExchangeRedis(multiplexer)
    .SetApplicationName("MyAppSuite"); // Must be identical across instances

builder.Services.Configure<CookieAuthenticationOptions>("Cookies", options =>
{
    options.Cookie.Name = "MyAppAuth"; // Same across instances
    options.Cookie.SameSite = SameSiteMode.Lax;
    options.Cookie.HttpOnly = true;
    options.Cookie.SecurePolicy = builder.Environment.IsDevelopment()
        ? CookieSecurePolicy.SameAsRequest
        : CookieSecurePolicy.Always;
});
```

`SetApplicationName` = key discriminator. `PersistKeysToStackExchangeRedis` stores encrypted keys so all instances encrypt/decrypt consistently.

```yaml
# docker-compose.yml — 2 app instances sharing keys via Dragonfly
services:
  dragonfly:
    image: docker.dragonflydb.io/dragonflydb/dragonfly
    command: --maxmemory 256mb --cache_mode
    ports: ["6379:6379"]

  app-a:
    build: .
    environment:
      - ConnectionStrings__Redis=dragonfly:6379,abortConnect=false
      - DataProtection__ApplicationName=MyAppSuite
    depends_on: [dragonfly]

  app-b:
    build: .
    environment:
      - ConnectionStrings__Redis=dragonfly:6379,abortConnect=false
      - DataProtection__ApplicationName=MyAppSuite
    depends_on: [dragonfly]
```

## 3. Cache-Aside Pattern
App owns load + invalidate. Cache never fetches alone.

```csharp
public class CacheAsideService<T>(IDistributedCache cache)
{
    private static readonly JsonSerializerOptions _json = new();

    public async Task<T?> GetOrCreateAsync(string key, Func<Task<T?>> factory,
        DistributedCacheEntryOptions? options = null)
    {
        if (await cache.GetStringAsync(key) is { } cached)
            return JsonSerializer.Deserialize<T>(cached, _json);

        var value = await factory();
        if (value is not null)
            await cache.SetStringAsync(key, JsonSerializer.Serialize(value, _json),
                options ?? new DistributedCacheEntryOptions
                    { AbsoluteExpirationRelativeToNow = TimeSpan.FromMinutes(10) });
        return value;
    }
}
```

Read-heavy + infrequent writes. Pair with write-through for freshness.
## 4. Serialization Strategies
**JSON** (default, readable):
```csharp
await cache.SetStringAsync(key, JsonSerializer.Serialize(data, _jsonOpts), options);
```

**Protobuf** (binary, compact):
```csharp
[ProtoContract]
public class ProductCacheEntry
{
    [ProtoMember(1)] public Guid Id { get; set; }
    [ProtoMember(2)] public string Name { get; set; } = "";
    [ProtoMember(3)] public decimal Price { get; set; }
}
// Serialize: Serializer.Serialize(ms, data); cache.SetAsync(key, ms.ToArray(), options);
// Deserialize: Serializer.Deserialize<T>(new MemoryStream(bytes));
```

```csharp
public static async Task SetCompressedAsync(this IDistributedCache cache,
    string key, string json, DistributedCacheEntryOptions options)
{
    var bytes = Encoding.UTF8.GetBytes(json);
    using var output = new MemoryStream();
    using (var brotli = new BrotliStream(output, CompressionLevel.Optimal, leaveOpen: true))
        await brotli.WriteAsync(bytes);
    await cache.SetAsync(key, output.ToArray(), options);
}

public static async Task<string?> GetCompressedAsync(this IDistributedCache cache, string key)
{
    var bytes = await cache.GetAsync(key);
    if (bytes is null) return null;
    using var ms = new MemoryStream(bytes);
    using var brotli = new BrotliStream(ms, CompressionMode.Decompress);
    return await new StreamReader(brotli).ReadToEndAsync();
}
```

Rule: JSON <1KB, Protobuf for throughput, Brotli >10KB.
## 5. Expiration Policies
```csharp
// Absolute: fixed time regardless of reads
var absolute = new DistributedCacheEntryOptions
    { AbsoluteExpirationRelativeToNow = TimeSpan.FromHours(1) };

// Sliding: resets on access
var sliding = new DistributedCacheEntryOptions
    { SlidingExpiration = TimeSpan.FromMinutes(15) };

// Combined: first limit wins
var combined = new DistributedCacheEntryOptions
{
    AbsoluteExpirationRelativeToNow = TimeSpan.FromHours(2),
    SlidingExpiration = TimeSpan.FromMinutes(20)
};
```

Combined: sliding keeps active entries alive, absolute caps max lifetime.
## 6. Cache Invalidation
```csharp
// Explicit
await cache.RemoveAsync($"product:{id}");
await cache.RemoveAsync($"product:{id}:details");

// Pattern-based (Redis SCAN)
public async Task RemoveByPatternAsync(IConnectionMultiplexer redis, string pattern)
{
    var db = redis.GetDatabase();
    foreach (var key in redis.GetServer(redis.GetEndPoints()[0]).Keys(pattern, 500))
        await db.KeyDeleteAsync(key);
}

// Versioning: append version to key, increment on write, old expires via TTL
// Change tokens: cancel source → invalidate all listeners
var cts = new CancellationTokenSource();
cts.Cancel();
```
## 7. Cache Tags and Dependencies

### Composite Dependencies with IMemoryCache
```csharp
public class TaggedCache(IMemoryCache cache)
{
    private readonly ConcurrentDictionary<string, CancellationTokenSource> _tags = new();

    public void Set<T>(string key, T value, TimeSpan duration, params string[] tags)
    {
        var cts = CancellationTokenSource.CreateLinkedTokenStore(
            tags.Select(t => _tags.GetOrAdd(t, _ => new CancellationTokenSource()).Token));
        cache.Set(key, value, new MemoryCacheEntryOptions
        {
            AbsoluteExpirationRelativeToNow = duration,
            ExpirationTokens = { new CancellationChangeToken(cts.Token) }
        });
    }

    public void InvalidateTag(string tag)
    {
        if (_tags.TryRemove(tag, out var cts)) cts.Cancel();
    }
}
```

### Redis Pub/Sub Cross-Instance Invalidation

```csharp
public class CacheInvalidationHub(IConnectionMultiplexer redis, IDistributedCache cache,
    ILogger<CacheInvalidationHub> logger)
{
    private readonly ISubscriber _sub = redis.GetSubscriber();
    public void Subscribe() => _sub.Subscribe("cache:invalidate", (_, key) =>
    {
        logger.LogInformation("Invalidating: {Key}", key);
        cache.RemoveAsync(key!).GetAwaiter().GetResult();
    });
    public async Task PublishInvalidationAsync(string key)
        => await _sub.PublishAsync("cache:invalidate", key);
}
```

One invalidates → all remove. Essential for multi-layer (§9).

### Domain-Specific Key Namespacing

Structured keys = pattern invalidation + easier debugging.

```text
{domain}:{entity}:{id}        — "product:detail:abc123"
{domain}:dedup:{reference}     — "payment:dedup:tx-456"
{domain}:track:{identifier}    — "shipment:track:pkg-789"
```

```csharp
public static class CacheKeys
{
    public static string ForEntity(string domain, string id) => $"{domain}:detail:{id}";
    public static string ForDedup(string domain, string reference) => $"{domain}:dedup:{reference}";
    public static string ForTracking(string domain, string id) => $"{domain}:track:{id}";
    public static string ForList(string domain, int page) => $"{domain}:list:p{page}";
}
```

`CacheKeys.ForEntity(...)` > string interpolation at call sites.

### Cache Key Architecture with IDistributedCache

Key prefixing via `RedisCacheOptions.InstanceName` (e.g. `"fo:"`), independent of `ChannelPrefix`.

**Tenant-scoped:**
```text
{prefix}:{tenantId}:entity:{type}:{id}    — "fo:tenant-123:product:detail:abc"
```

**Global:**
```text
{prefix}:{from}:{to}:{date}               — "fo:USD:EUR:2026-01-01"
```

**WARNING:** Double-prefixing harmless but redundant.
## 8. Cache Stampede Prevention

Hot key expires → concurrent requests hammer DB.

### Singleflight with SemaphoreSlim

```csharp
public class StampedeSafeCache(IDistributedCache cache, ILogger<StampedeSafeCache> logger)
{
    private static readonly ConcurrentDictionary<string, SemaphoreSlim> _locks = new();

    public async Task<T?> GetOrCreateAsync<T>(string key, Func<Task<T?>> factory,
        DistributedCacheEntryOptions options)
    {
        if (await cache.GetStringAsync(key) is { } cached) return JsonSerializer.Deserialize<T>(cached);
        var semaphore = _locks.GetOrAdd(key, _ => new SemaphoreSlim(1, 1));
        await semaphore.WaitAsync();
        try
        {
            if (await cache.GetStringAsync(key) is { } c2) return JsonSerializer.Deserialize<T>(c2);
            logger.LogDebug("Cache miss, populating: {Key}", key);
            var value = await factory();
            if (value is not null)
                await cache.SetStringAsync(key, JsonSerializer.Serialize(value), options);
            return value;
        }
        finally { semaphore.Release(); _locks.TryRemove(key, out _); }
    }
}
```

> `_locks` MUST be `static` in scoped services. Non-static = per-instance dictionary = no protection. `TryRemove` in `finally` prevents leaks.

### Stale-While-Revalidate

Serve stale, refresh in background:

```csharp
public async Task<T?> GetStaleWhileRevalidateAsync<T>(string key,
    Func<Task<T?>> factory, DistributedCacheEntryOptions options)
{
    if (await cache.GetStringAsync(key) is { } json)
    {
        _ = Task.Run(async () => { try { await cache.RefreshAsync(key); } catch { } });
        return JsonSerializer.Deserialize<T>(json);
    }
    return await factory();
}
```
## 9. Multi-Layer Caching

L1 `IMemoryCache` (speed) + L2 Redis `IDistributedCache` (cross-instance).

```csharp
public class TwoLevelCache(IMemoryCache l1, IDistributedCache l2)
{
    public async Task<T?> GetAsync<T>(string key) where T : class
    {
        if (l1.TryGetValue(key, out T? l1Value)) return l1Value;
        if (await l2.GetStringAsync(key) is { } json)
        {
            var value = JsonSerializer.Deserialize<T>(json);
            l1.Set(key, value, TimeSpan.FromMinutes(5));
            return value;
        }
        return null;
    }

    public async Task SetAsync<T>(string key, T value, TimeSpan l2Expiration) where T : class
    {
        l1.Set(key, value, TimeSpan.FromMinutes(5));
        await l2.SetStringAsync(key, JsonSerializer.Serialize(value),
            new DistributedCacheEntryOptions { AbsoluteExpirationRelativeToNow = l2Expiration });
    }

    public async Task RemoveAsync(string key)
    {
        l1.Remove(key);
        await l2.RemoveAsync(key);
    }
}
```

Register: `AddMemoryCache()` + `AddStackExchangeRedisCache(...)` + `AddSingleton<TwoLevelCache>()`.
## 10. Rate Limiting with Cache

Redis atomic counters ideal for distributed rate limiting.

```csharp
public class RedisRateLimiter(IDistributedCache cache)
{
    public async Task<bool> IsAllowedAsync(string key, int maxRequests, TimeSpan window)
    {
        var now = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
        var countJson = await cache.GetStringAsync($"rl:{key}");
        var windowJson = await cache.GetStringAsync($"rl:{key}:w");
        var count = countJson is null ? 0 : long.Parse(countJson);
        var start = windowJson is null ? 0 : long.Parse(windowJson);
        if (now - start >= window.TotalSeconds) { count = 0; start = now; }
        if (count >= maxRequests) return false;
        count++;
        var ttl = TimeSpan.FromSeconds(now - start + window.TotalSeconds);
        var opts = new DistributedCacheEntryOptions { AbsoluteExpirationRelativeToNow = ttl };
        await cache.SetStringAsync($"rl:{key}", count.ToString(), opts);
        await cache.SetStringAsync($"rl:{key}:w", start.ToString(), opts);
        return true;
    }
}
```

> Prod: prefer `StringIncrementAsync` or Lua scripts for true atomicity.

```csharp
if (!await rateLimiter.IsAllowedAsync($"user:{userId}:api", 100, TimeSpan.FromMinutes(1)))
    return StatusCode(429);
```
## 11. Distributed Locking

```csharp
public class DistributedLockService(IConnectionMultiplexer redis)
{
    public async Task<IDisposable?> AcquireLockAsync(string resource, TimeSpan expiry,
        TimeSpan? waitTimeout = null)
    {
        var db = redis.GetDatabase();
        var value = Guid.NewGuid().ToString();
        var end = DateTimeOffset.UtcNow + (waitTimeout ?? TimeSpan.Zero);
        while (DateTimeOffset.UtcNow < end)
        {
            if (await db.StringSetAsync($"lock:{resource}", value, expiry, When.NotExists))
                return new LockReleaser(db, $"lock:{resource}", value);
            await Task.Delay(50);
        }
        return null;
    }

    private record LockReleaser(IDatabase Db, string Key, string Value) : IDisposable
    {
        public void Dispose()
        {
            var s = @"if redis.call('get', KEYS[1]) == ARGV[1]
                then return redis.call('del', KEYS[1]) else return 0 end";
            Db.ScriptEvaluate(s, new RedisKey[] { Key }, new RedisValue[] { Value });
        }
    }
}
```

`await using var @lock = await lockService.AcquireLockAsync("order:batch", TimeSpan.FromSeconds(30));`
## 12. Testing

### Unit Tests

```csharp
public class ProductServiceTests
{
    private readonly Mock<IDistributedCache> _cache = new();
    private readonly Mock<IProductRepository> _repo = new();
    private readonly ProductService _sut;
    public ProductServiceTests() { _sut = new ProductService(_cache.Object, _repo.Object); }

    [Fact]
    public async Task GetById_ReturnsCached_WhenPresent()
    {
        var product = new Product { Id = Guid.NewGuid(), Name = "Widget" };
        _cache.Setup(c => c.GetStringAsync("product:" + product.Id))
              .ReturnsAsync(JsonSerializer.Serialize(product));
        var result = await _sut.GetByIdAsync(product.Id);
        result.ShouldNotBeNull();
        result.Name.ShouldBe("Widget");
        _repo.Verify(r => r.GetByIdAsync(It.IsAny<Guid>()), Times.Never);
    }

    [Fact]
    public async Task GetById_CachesAndReturns_WhenNotCached()
    {
        var product = new Product { Id = Guid.NewGuid(), Name = "Gadget" };
        _cache.Setup(c => c.GetStringAsync(It.IsAny<string>())).ReturnsAsync((string?)null);
        _repo.Setup(r => r.GetByIdAsync(product.Id)).ReturnsAsync(product);
        var result = await _sut.GetByIdAsync(product.Id);
        result.ShouldNotBeNull();
        _cache.Verify(c => c.SetStringAsync("product:" + product.Id,
            It.IsAny<string>(), It.IsAny<DistributedCacheEntryOptions>()), Times.Once);
    }
}
```

### Integration with Testcontainers

```csharp
public class RedisCacheTest : IAsyncLifetime
{
    private RedisContainer _redis = null!;
    private IDistributedCache _cache = null!;

    public async Task InitializeAsync()
    {
        _redis = new RedisBuilder().WithImage("redis:7-alpine").Build();
        await _redis.StartAsync();
        _cache = new ServiceCollection()
            .AddStackExchangeRedisCache(o =>
            {
                o.Configuration = _redis.GetConnectionString();
                o.InstanceName = "test:";
            }).BuildServiceProvider().GetRequiredService<IDistributedCache>();
    }

    public async Task DisposeAsync() => await _redis.DisposeAsync();

    [Fact]
    public async Task SetAndGet_WorksEndToEnd()
    {
        await _cache.SetStringAsync("key", "value");
        (await _cache.GetStringAsync("key")).ShouldBe("value");
    }
}
```

## 13. Anti-Patterns

**No unscoped user data.** Without user/tenant ID: User A sees User B's data. `$"user:{userId}:profile"`.

**No TTL-only invalidation.** Invalidate on writes. TTL = safety net.

**No skipped serialization.** `byte[]` API — skip serialize = garbage or crash.

**No cache as truth.** Empty/stale → fall back to DB. Cache loss must be non-catastrophic.

**No staleness-unsafe data.** Cache product list, fetch live price.

**No missing expiration.** Keys without TTL = forever in Redis. Memory leak.

**No missing `SetApplicationName`.** Different discriminator per instance → can't decrypt cross-instance cookies.

**No mismatched cookie names across SSO.** Different names → silent SSO breakage. Same name everywhere.

**No DEKs in distributed caches.** Envelope encryption DEKs = in-process only. Redis/Dragonfly = network exposure. Use process-local `ConcurrentDictionary` + TTL eviction:

````markdown
```csharp
// CORRECT: In-process DEK cache only
public class EncryptionKeyCache
{
    private readonly ConcurrentDictionary<string, CachedDek> _cache = new();
    private readonly TimeSpan _ttl = TimeSpan.FromMinutes(15);

    public byte[]? Get(string key)
    {
        if (_cache.TryGetValue(key, out var cached) && cached.IsValid)
            return cached.Key;
        _cache.TryRemove(key, out _);
        return null;
    }

    public void Set(string key, byte[] dek)
    {
        _cache[key] = new CachedDek(dek, DateTime.UtcNow + _ttl);
    }

    public void EvictFamily(string familyPrefix)
    {
        foreach (var key in _cache.Keys.Where(k => k.StartsWith(familyPrefix)))
            _cache.TryRemove(key, out _);
    }

    private record CachedDek(byte[] Key, DateTime ExpiresAt)
    {
        public bool IsValid => DateTime.UtcNow < ExpiresAt;
    }
}
```

```csharp
// WRONG: Never do this
await distributedCache.SetStringAsync($"dek:{keyId}", Convert.ToBase64String(dek));
```
````

## Related Skills

- **dotnet-clean-arch** — DI registration with IDistributedCache
- **aspnetcore-health** — Redis/Dragonfly health checks
- **signalr-realtime** — Redis backplane + pub/sub
- **keycloak-oidc** — Shared DataProtection keys
- **docker-deployment** — Docker Compose with Redis/Dragonfly
