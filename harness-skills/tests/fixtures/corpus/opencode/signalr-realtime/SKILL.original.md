---
name: signalr-realtime
description: "SignalR real-time communication patterns for ASP.NET Core. Hub design, authentication, scaling with Redis/Dragonfly backplane, rate limiting, connection lifecycle, WebRTC signaling, group management, client-to-server streaming, server-to-client streaming, automatic reconnection, message ordering. USE FOR: real-time notifications, chat systems, live dashboards, collaborative editing, video call signaling, presence tracking, bidirectional communication, hub design, SignalR scaling. DO NOT USE FOR: REST APIs, server-sent events only, WebSocket-only (without SignalR), non-.NET real-time."
license: MIT
metadata:
  author: community
  version: "1.0.0"
  frameworks: [".NET 8+", "SignalR", "Redis"]
---

# SignalR Real-Time Communication Patterns

Comprehensive patterns for building production-grade real-time features with ASP.NET Core SignalR.

## Hub Design Patterns

### Typed Hubs (preferred)

Typed hubs give you compile-time safety and IntelliSense on the client contract.

```csharp
public interface INotificationClient
{
    Task ReceiveNotification(NotificationDto notification);
    Task ReceiveBulkNotifications(IReadOnlyList<NotificationDto> notifications);
    Task ConnectionRateLimited(TimeSpan retryAfter);
}

public class NotificationHub : Hub<INotificationClient>
{
    private readonly ILogger<NotificationHub> _logger;

    public NotificationHub(ILogger<NotificationHub> logger) => _logger = logger;

    public async Task SubscribeToChannel(string channelName, CancellationToken ct)
    {
        await Groups.AddToGroupAsync(Context.ConnectionId, channelName, ct);
        _logger.LogInformation(
            "Connection {ConnectionId} subscribed to channel {Channel}",
            Context.ConnectionId, channelName);
    }

    public async Task UnsubscribeFromChannel(string channelName, CancellationToken ct)
    {
        await Groups.RemoveFromGroupAsync(Context.ConnectionId, channelName, ct);
    }

    // Return values work fine. The client receives the result of the invocation.
    public async Task<NotificationDto?> GetLatestNotification(CancellationToken ct)
    {
        return await _notificationService.GetLatestAsync(ct);
    }
}
```

### Untyped Hubs (flexible)

Use untyped hubs when the client contract is dynamic or you need to call methods that don't exist in a shared interface.

```csharp
public class LegacyHub : Hub
{
    public async Task SendRaw(string target, string method, object payload)
    {
        await Clients.User(target).SendAsync(method, payload);
    }
}
```

### Hub Context (calling clients from outside a hub)

```csharp
// Inject IHubContext into any service
public class OrderService
{
    private readonly IHubContext<NotificationHub, INotificationClient> _hub;

    public OrderService(IHubContext<NotificationHub, INotificationClient> hub) => _hub = hub;

    public async Task ProcessOrderAsync(Order order)
    {
        // Target specific user across all their connections
        await _hub.Clients.User(order.UserId.ToString())
            .ReceiveNotification(new NotificationDto("Order shipped", order.Id));

        // Target a group (e.g., all users watching a live dashboard)
        await _hub.Clients.Group($"dashboard-{order.TenantId}")
            .ReceiveBulkNotifications(new[] { new NotificationDto("New order", order.Id) });
    }
}
```

### Naming conventions

- Hub methods: PascalCase verbs. `SubscribeToChannel`, `JoinSession`, `SendOffer`.
- Client interface methods: PascalCase event names. `ReceiveNotification`, `PeerJoined`.
- Groups: namespaced with a prefix. `"tenant-{id}"`, `"session-{id}"`, `"channel-{name}"`.

## Authentication

### Basic authorize attribute

```csharp
[Authorize]
public class ChatHub : Hub
{
    public override async Task OnConnectedAsync()
    {
        var userId = Context.UserIdentifier!; // Set during token validation
        await Groups.AddToGroupAsync(Context.ConnectionId, $"user-{userId}");
        await base.OnConnectedAsync();
    }
}
```

### JWT bearer with claim mapping

Configure the mapping in `Program.cs` so `UserIdentifier` picks up the right claim:

```csharp
builder.Services.AddAuthentication(JwtBearerDefaults.AuthenticationScheme)
    .AddJwtBearer(options =>
    {
        options.TokenValidationParameters = new TokenValidationParameters
        {
            ValidateIssuer = true,
            ValidateAudience = true,
            ValidateLifetime = true,
            ValidateIssuerSigningKey = true,
            // ...
        };

        options.Events = new JwtBearerEvents
        {
            OnMessageReceived = ctx =>
            {
                // SignalR sends the token as a query string
                var accessToken = ctx.Request.Query["access_token"];
                var path = ctx.HttpContext.Request.Path;
                if (!string.IsNullOrEmpty(accessToken) && path.StartsWithSegments("/hubs"))
                {
                    ctx.Token = accessToken;
                }
                return Task.CompletedTask;
            }
        };
    });

// Map the NameIdentifier claim to UserIdentifier
builder.Services.AddSignalR(options =>
{
    options.MapUser = principal => principal.FindFirstValue(ClaimTypes.NameIdentifier)!;
});
```

### Policy-based authorization

```csharp
[Authorize(Policy = "CanManageCalls")]
public class VideoCallHub : Hub<IVideoCallClient>
{
    // Only users with the "CanManageCalls" policy can connect
}
```

## Connection Lifecycle

### Tracking multiple connections per user

A single user often connects from several devices. Track all of them.

```csharp
public class PresenceHub : Hub
{
    private readonly IConnectionTracker _tracker;

    public PresenceHub(IConnectionTracker tracker) => _tracker = tracker;

    public override async Task OnConnectedAsync()
    {
        var userId = Context.UserIdentifier!;
        var device = Context.GetHttpContext()?.Request.Headers["X-Device-Type"].FirstOrDefault() ?? "unknown";

        await _tracker.AddConnectionAsync(userId, Context.ConnectionId, device);

        // Notify others in the user's groups
        await Clients.Groups(await _tracker.GetUserGroupsAsync(userId))
            .UserOnline(new PresenceInfo(userId, device));

        await base.OnConnectedAsync();
    }

    public override async Task OnDisconnectedAsync(Exception? exception)
    {
        var userId = Context.UserIdentifier!;

        var remaining = await _tracker.RemoveConnectionAsync(userId, Context.ConnectionId);
        if (remaining == 0)
        {
            // Last connection closed. Broadcast offline status.
            await Clients.Groups(await _tracker.GetUserGroupsAsync(userId))
                .UserOffline(new PresenceInfo(userId, "none"));
        }

        await base.OnDisconnectedAsync(exception);
    }
}

// In-memory tracker for single-server setups. Swap for Redis-backed in production.
public interface IConnectionTracker
{
    Task AddConnectionAsync(string userId, string connectionId, string device);
    Task<int> RemoveConnectionAsync(string userId, string connectionId);
    Task<IReadOnlyList<string>> GetUserGroupsAsync(string userId);
}
```

## Group Management

### Dynamic groups with automatic cleanup

```csharp
public class SessionHub : Hub<ISessionClient>
{
    private readonly IConnectionTracker _tracker;

    public SessionHub(IConnectionTracker tracker) => _tracker = tracker;

    public async Task JoinSession(Guid sessionId, CancellationToken ct)
    {
        var groupName = $"session-{sessionId}";
        await Groups.AddToGroupAsync(Context.ConnectionId, groupName, ct);
        await _tracker.AddGroupAsync(Context.UserIdentifier!, groupName);

        // Tell everyone already in the session about the new participant
        await Clients.Group(groupName)
            .ParticipantJoined(new ParticipantInfo(Context.UserIdentifier!, Context.ConnectionId));
    }

    public async Task LeaveSession(Guid sessionId, CancellationToken ct)
    {
        var groupName = $"session-{sessionId}";
        await Groups.RemoveFromGroupAsync(Context.ConnectionId, groupName, ct);
        await _tracker.RemoveGroupAsync(Context.UserIdentifier!, groupName);

        await Clients.Group(groupName)
            .ParticipantLeft(new ParticipantInfo(Context.UserIdentifier!, Context.ConnectionId));
    }

    public override async Task OnDisconnectedAsync(Exception? exception)
    {
        // Remove from all groups this connection joined
        var groups = await _tracker.GetConnectionGroupsAsync(Context.ConnectionId);
        foreach (var group in groups)
        {
            await Groups.RemoveFromGroupAsync(Context.ConnectionId, group);
        }
        await _tracker.RemoveAllConnectionsForUserAsync(Context.UserIdentifier!);
        await base.OnDisconnectedAsync(exception);
    }
}
```

## Scaling with Backplane

### Redis backplane setup

```csharp
builder.Services.AddSignalR()
    .AddStackExchangeRedis(connectionString, options =>
    {
        options.Configuration.ChannelPrefix = RedisChannel.Literal("MyApp.SignalR");
    });
```

### Dragonfly drop-in

Dragonfly is API-compatible with Redis. Point the same connection string at Dragonfly and it works with no code changes.

```csharp
// Same code. Different connection string.
var redis = builder.Configuration.GetConnectionString("Dragonfly")
    ?? builder.Configuration.GetConnectionString("Redis");
builder.Services.AddSignalR().AddStackExchangeRedis(redis!);
```

### Key considerations

- No sticky sessions needed. Any server can handle any connection.
- The backplane uses Redis pub/sub. Every message goes to every server. That works well up to a few thousand messages per second.
- Group membership is local to each server. When server A adds a connection to a group, server B learns about it via the backplane. Broadcasting to a group fan-out happens on every server.
- For very high throughput (tens of thousands of messages/sec), consider partitioning hubs by tenant or region instead of a single backplane.

## Rate Limiting

### Hub policy with built-in rate limiter

```csharp
builder.Services.AddSignalR(options =>
{
    options.EnableDetailedErrors = builder.Environment.IsDevelopment();
})
.AddHubOptions<ChatHub>(options =>
{
    options.MaximumReceiveMessageSize = 32 * 1024; // 32 KB

    options.AddRateLimiter(rateLimiter =>
    {
        rateLimiter.GlobalLimit = 100;
        rateLimiter.ReconnectionWindowSize = TimeSpan.FromMinutes(1);

        rateLimiter.AddPolicy("PerUser", context =>
        {
            var userId = context.Context.UserIdentifier;
            return userId is not null
                ? RateLimitPartition.GetSlidingWindowLimiter(
                    partitionKey: userId,
                    factory: _ => new SlidingWindowRateLimiterOptions
                    {
                        Window = TimeSpan.FromMinutes(1),
                        SegmentsPerWindow = 6,
                        TokensPerPeriod = 30,
                        QueueProcessingOrder = QueueProcessingOrder.OldestFirst,
                        QueueLimit = 5
                    })
                : RateLimitPartition.GetNoLimiter("anonymous");
        });
    });
});
```

Apply the policy per hub method:

```csharp
[HubMethodName("SendMessage")]
[EnableRateLimiting("PerUser")]
public async Task SendMessage(string message, CancellationToken ct)
{
    // This method is capped at 30 calls per minute per user
    await Clients.Group(_currentGroup).ReceiveMessage(
        new ChatMessage(Context.UserIdentifier!, message, DateTimeOffset.UtcNow));
}
```

## Client Streaming

The client pushes a stream of items to the server.

```csharp
public interface IStreamClient
{
    Task StreamProgress(StreamProgress update);
    Task StreamComplete(StreamResult result);
}

public class UploadHub : Hub<IStreamClient>
{
    public async Task UploadChunkStream(
        IAsyncEnumerable<UploadChunk> chunks,
        CancellationToken ct)
    {
        var totalBytes = 0L;

        await foreach (var chunk in chunks.WithCancellation(ct))
        {
            totalBytes += chunk.Data.Length;
            await _uploadService.AppendChunkAsync(chunk.UploadId, chunk.Data, ct);

            // Push progress back to the calling client only
            await Clients.Caller.StreamProgress(
                new StreamProgress(chunk.UploadId, totalBytes));
        }

        await Clients.Caller.StreamComplete(new StreamResult(totalBytes, success: true));
    }
}

public record UploadChunk(Guid UploadId, byte[] Data, int Sequence);
```

## Server Streaming

The server pushes a stream of items to the client.

### IAsyncEnumerable return

```csharp
public class DashboardHub : Hub
{
    public async IAsyncEnumerable<MetricsSnapshot> StreamMetrics(
        [EnumeratorCancellation] CancellationToken ct)
    {
        while (!ct.IsCancellationRequested)
        {
            var snapshot = await _metricsService.GetSnapshotAsync(ct);
            yield return snapshot;
            await Task.Delay(TimeSpan.FromSeconds(2), ct);
        }
    }
}
```

### Channel-based streaming

Use channels when you have an external producer feeding data in.

```csharp
public class NotificationHub : Hub
{
    private readonly INotificationFeed _feed;

    public NotificationHub(INotificationFeed feed) => _feed = feed;

    public ChannelReader<NotificationDto> StreamNotifications(CancellationToken ct)
    {
        // Subscribe returns a channel reader that the hub infrastructure
        // drains and sends to the client automatically.
        return _feed.Subscribe(Context.UserIdentifier!, ct);
    }
}

public interface INotificationFeed
{
    ChannelReader<NotificationDto> Subscribe(string userId, CancellationToken ct);
}
```

## WebRTC Signaling Pattern

SignalR handles the signaling. WebRTC handles the media.

```csharp
public interface IVideoCallClient
{
    Task ReceiveOffer(string callerId, string sdpOffer);
    Task ReceiveAnswer(string callerId, string sdpAnswer);
    Task ReceiveIceCandidate(string senderId, string candidate, string sdpMid, int sdpMLineIndex);
    Task CallEnded(string callerId, CallEndReason reason);
}

public class VideoCallHub : Hub<IVideoCallClient>
{
    private readonly ICallSessionManager _sessions;

    public async Task InitiateCall(string targetUserId, CancellationToken ct)
    {
        var callerId = Context.UserIdentifier!;
        var sessionId = await _sessions.CreateAsync(callerId, targetUserId, ct);

        await Groups.AddToGroupAsync(Context.ConnectionId, $"call-{sessionId}", ct);
    }

    public async Task SendOffer(string sessionId, string sdpOffer, CancellationToken ct)
    {
        var session = await _sessions.GetAsync(sessionId, ct);
        // Send the offer only to the other participant
        await Clients.GroupExcept($"call-{sessionId}", Context.ConnectionId)
            .ReceiveOffer(Context.UserIdentifier!, sdpOffer);
    }

    public async Task SendAnswer(string sessionId, string sdpAnswer, CancellationToken ct)
    {
        var session = await _sessions.GetAsync(sessionId, ct);
        await Clients.GroupExcept($"call-{sessionId}", Context.ConnectionId)
            .ReceiveAnswer(Context.UserIdentifier!, sdpAnswer);
    }

    public async Task SendIceCandidate(
        string sessionId, string candidate, string sdpMid, int sdpMLineIndex, CancellationToken ct)
    {
        await Clients.GroupExcept($"call-{sessionId}", Context.ConnectionId)
            .ReceiveIceCandidate(Context.UserIdentifier!, candidate, sdpMid, sdpMLineIndex);
    }

    public async Task EndCall(string sessionId, CancellationToken ct)
    {
        await _sessions.EndAsync(sessionId, ct);
        await Clients.Group($"call-{sessionId}")
            .CallEnded(Context.UserIdentifier!, CallEndReason.HungUp);
    }

    public override async Task OnDisconnectedAsync(Exception? exception)
    {
        // Clean up any active calls for this connection
        var activeSessions = await _sessions.GetActiveSessionsForUserAsync(Context.UserIdentifier!);
        foreach (var sessionId in activeSessions)
        {
            await EndCall(sessionId, CancellationToken.None);
        }
        await base.OnDisconnectedAsync(exception);
    }
}
```

## Automatic Reconnection

### JavaScript client

```javascript
const connection = new signalR.HubConnectionBuilder()
    .withUrl("/hubs/chat", {
        accessTokenFactory: () => localStorage.getItem("access_token")
    })
    .withAutomaticReconnect([0, 2000, 5000, 10000, 30000])
    .build();

connection.onreconnecting(error => {
    console.warn(`Connection lost. Reconnecting in... (${error})`);
    showBanner("Reconnecting...");
});

connection.onreconnected(connectionId => {
    console.log(`Reconnected with ID: ${connectionId}`);
    hideBanner();
    // Re-join groups. The server lost group membership on reconnect.
    connection.send("RejoinGroups", currentGroups);
});

connection.onclose(error => {
    console.error("Connection closed permanently.", error);
    showBanner("Disconnected. Refresh to retry.");
});

await connection.start();
```

### .NET client

```csharp
var connection = new HubConnectionBuilder()
    .WithUrl("https://example.com/hubs/chat", options =>
    {
        options.AccessTokenProvider = () => Task.FromResult(_tokenService.GetToken());
    })
    .WithAutomaticReconnect(TimeSpan.Zero, TimeSpan.FromSeconds(2),
        TimeSpan.FromSeconds(5), TimeSpan.FromSeconds(10), TimeSpan.FromSeconds(30))
    .Build();

connection.Reconnecting += error =>
{
    _logger.LogWarning("Connection lost. Reconnecting... ({Error})", error);
    return Task.CompletedTask;
};

connection.Reconnected += connectionId =>
{
    _logger.LogInformation("Reconnected as {ConnectionId}", connectionId);
    // Re-establish server-side state (groups, presence)
    return RejoinGroupsAsync(connection);
};

connection.Closed += error =>
{
    _logger.LogError("Connection closed permanently. {Error}", error);
    return Task.CompletedTask;
};

await connection.StartAsync();
```

The reconnect intervals array is the delay between each attempt. After all retries exhaust, the connection stays closed. You can also pass a custom `IRetryPolicy` for exponential backoff with jitter.

## Error Handling

### Server-side hub exceptions

Exceptions thrown in hub methods are sent to the client. Don't leak internals.

```csharp
public async Task TransferFunds(TransferRequest request, CancellationToken ct)
{
    try
    {
        await _transferService.TransferAsync(request, ct);
    }
    catch (InsufficientFundsException ex)
    {
        // The client receives this as a HubException with the message.
        throw new HubException("Insufficient funds for this transfer.");
    }
    catch (ValidationException ex)
    {
        throw new HubException(ex.Message);
    }
    // Unhandled exceptions also reach the client, but with a generic
    // "An unexpected error occurred" message. Log them on the server.
}
```

### Client-side error handling (JavaScript)

```javascript
try {
    await connection.invoke("TransferFunds", request);
} catch (err) {
    if (err instanceof signalR.HubException) {
        showError(err.message);
    } else {
        showError("Network error. Check your connection.");
    }
}
```

### Graceful disconnect configuration

```csharp
builder.Services.AddSignalR(options =>
{
    // How long the server waits before declaring a client disconnected
    // after the last keep-alive ping. Default is 30 seconds.
    options.KeepAliveInterval = TimeSpan.FromSeconds(15);

    // How long the client waits before timing out. Must be double
    // the server KeepAliveInterval. Default is 30 seconds.
    options.HandshakeTimeout = TimeSpan.FromSeconds(15);
    options.ClientTimeoutInterval = TimeSpan.FromSeconds(30);

    // Allow detailed errors in development only
    options.EnableDetailedErrors = builder.Environment.IsDevelopment();
});
```

## Testing

### Unit testing with a mock client

```csharp
public class NotificationHubTests
{
    private readonly Mock<ISender> _sender = new();
    private readonly Mock<ILogger<NotificationHub>> _logger = new();

    private NotificationHub CreateHub(
        Mock<IClientProxy>? callerMock = null,
        Mock<IGroupManager>? groupsMock = null)
    {
        var hub = new NotificationHub(_sender.Object, _logger.Object);
        var clients = new Mock<IHubCallerClients<INotificationClient>>();
        clients.Setup(c => c.Caller).Returns(callerMock?.Object ?? Mock.Of<IClientProxy>());
        clients.Setup(c => c.Group(It.IsAny<string>())).Returns(Mock.Of<IClientProxy>());
        clients.Setup(c => c.User(It.IsAny<string>())).Returns(Mock.Of<IClientProxy>());

        hub.Clients = clients.Object;
        hub.Groups = groupsMock?.Object ?? Mock.Of<IGroupManager>();

        // Set up a mock context
        var context = new Mock<HubCallerContext>();
        context.Setup(c => c.ConnectionId).Returns("test-conn-1");
        context.Setup(c => c.UserIdentifier).Returns("user-42");
        hub.Context = context.Object;

        return hub;
    }

    [Fact]
    public async Task SubscribeToChannel_AddsConnectionToGroup()
    {
        var groupMock = new Mock<IGroupManager>();
        var hub = CreateHub(groupsMock: groupMock);

        await hub.SubscribeToChannel("orders", CancellationToken.None);

        groupMock.Verify(g => g.AddToGroupAsync(
            "test-conn-1", "orders", It.IsAny<CancellationToken>()), Times.Once);
    }
}
```

### Integration testing with WebApplicationFactory

```csharp
public class ChatHubIntegrationTests : IClassFixture<WebApplicationFactory<Program>>
{
    private readonly WebApplicationFactory<Program> _factory;

    public ChatHubIntegrationTests(WebApplicationFactory<Program> factory)
    {
        _factory = factory.WithWebHostBuilder(builder =>
        {
            builder.ConfigureServices(services =>
            {
                // Replace real services with test doubles
                services.Replace(ServiceDescriptor.Singleton(
                    typeof(IConnectionTracker), new InMemoryConnectionTracker()));
            });
        });
    }

    [Fact]
    public async Task AuthenticatedUser_CanConnect()
    {
        var client = _factory.CreateClient();
        // Negotiate expects a real auth cookie or token
        var response = await client.GetAsync("/hubs/chat/negotiate");

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        var negotiate = await response.Content.ReadFromJsonAsync<NegotiationResponse>();
        Assert.NotNull(negotiate?.ConnectionId);
    }
}
```

## Anti-Patterns

**Blocking hub methods.** Hub methods execute on the thread pool. Never call `.Result`, `.Wait()`, or `Thread.Sleep`. Always `await`. Blocking starves the thread pool and kills throughput.

```csharp
// Bad
public Task<string> GetStatus()
{
    var result = _service.GetStatusSync().Result; // Deadlock risk
    return Task.FromResult(result);
}

// Good
public async Task<string> GetStatus(CancellationToken ct)
{
    return await _service.GetStatusAsync(ct);
}
```

**Stateful hub instances.** A hub is transient. Don't store per-connection state in hub fields. Use `Context`, groups, or an external store (Redis, database).

```csharp
// Bad: _sessions is lost on the next invocation
public class BadHub : Hub
{
    private readonly List<string> _sessions = new();
}

// Good: use groups or an external store
```

**Broadcasting to all when a group suffices.** `Clients.All()` sends to every connection on every server. Use groups, `Clients.User()`, or `Clients.Caller()`.

**No authentication.** Every hub that handles user data or actions needs `[Authorize]`. Unauthenticated hubs are fine for public feeds, but most apps need auth.

**No rate limiting.** Without it, a single client can spam thousands of messages per second. Always configure `AddRateLimiter` for user-facing hubs.

**Ignoring disconnection.** Always override `OnDisconnectedAsync` to clean up groups, presence state, and external resources. Leaked connections accumulate and cause stale data.

**Catching all exceptions silently.** Hub methods should throw `HubException` for expected failures. Catching everything and returning silently hides bugs from both client and server logs.

**Storing tokens in hub state.** Don't read tokens from `Context` and store them in fields or statics. Tokens expire. Always validate on use, or let the auth middleware handle it.

## Related Skills

- **blazor-mudblazor** — WebRTC video session pattern via JS interop + SignalR signaling hub (see Section 8 of that skill)
- **aspnetcore-health** — Health checks including SignalR dependency monitoring
- **dotnet-clean-arch** — Project structure, DI registration, and CQRS patterns that integrate with SignalR hubs
- **keycloak-oidc** — Authentication setup for securing hubs with `[Authorize]`
- **docker-deployment** — nginx WebSocket proxy configuration required for SignalR behind reverse proxy
