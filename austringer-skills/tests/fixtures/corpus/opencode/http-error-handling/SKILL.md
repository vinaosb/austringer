---
name: http-error-handling
description: "HTTP error handling patterns for ASP.NET Core minimal APIs. ProblemDetails (RFC 7807), global exception middleware, validation error responses, custom error types, error correlation, structured error logging, rate limit responses, not-found handling, domain exception mapping. USE FOR: API error responses, exception handling middleware, validation error formatting, error correlation across services, structured error logging. DO NOT USE FOR: MVC exception filters (use minimal API middleware), frontend error UI (use Blazor error boundaries), business rule validation (use domain layer)."
license: MIT
metadata:
  author: community
  version: "1.0.0"
  frameworks: [".NET 8+", "ASP.NET Core"]
---

# HTTP Error Handling for ASP.NET Core Minimal APIs

Every endpoint returns `ProblemDetails` (RFC 7807) on failure. No plain text, no stack traces in prod, no silent swallowing.

---

## 1. ProblemDetails (RFC 7807)

.NET 8 built-in. Register once — all 400/404/500 get proper JSON.

```csharp
var builder = WebApplication.CreateBuilder(args);

builder.Services.AddProblemDetails(options =>
{
    options.CustomizeProblemDetails = ctx =>
    {
        ctx.ProblemDetails.Extensions["traceId"] =
            Activity.Current?.Id ?? ctx.HttpContext.TraceIdentifier;
    };
});

var app = builder.Build();
app.UseExceptionHandler();
app.UseStatusCodePages();

app.MapGet("/customers/{id:guid}", async (Guid id, CustomerRepository repo) =>
{
    var customer = await repo.FindAsync(id);
    return customer is null ? Results.NotFound() : Results.Ok(customer);
});

// GET /customers/00000000-0000-0000-0000-000000000000 → 404
// { "type": "https://tools.ietf.org/html/rfc9110#section-15.5.5",
//   "title": "Not Found", "status": 404, "traceId": "00-abc123..." }
```

`Results.NotFound()`, `Results.BadRequest()`, `Results.Problem()` → `ProblemDetails` when `AddProblemDetails()` registered.

---

## 2. Global Exception Middleware

Catch all — nothing escapes as plain text 500.

```csharp
public class GlobalExceptionMiddleware(
    RequestDelegate next, ILogger<GlobalExceptionMiddleware> logger)
{
    public async Task InvokeAsync(HttpContext context)
    {
        try
        {
            await next(context);
        }
        catch (Exception ex)
        {
            logger.LogError(ex, "Unhandled exception on {Method} {Path}",
                context.Request.Method, context.Request.Path);

            await Results.Problem(
                title: "An unexpected error occurred",
                statusCode: StatusCodes.Status500InternalServerError,
                extensions: new Dictionary<string, object?>
                {
                    ["traceId"] = Activity.Current?.Id ?? context.TraceIdentifier
                }).ExecuteAsync(context);
        }
    }
}

// Register before other middleware
app.UseMiddleware<GlobalExceptionMiddleware>();
```

Place first after `UseExceptionHandler()`.

---

## 3. Domain Exception Mapping

Map domain exceptions → HTTP status. Endpoints stay clean.

```csharp
public static class DomainExceptionMappings
{
    public static IResult ToProblemResult(this DomainException ex, HttpContext ctx)
    {
        var traceId = Activity.Current?.Id ?? ctx.TraceIdentifier;

        return ex switch
        {
            NotFoundException nf => Results.Problem(
                title: "Resource not found", detail: nf.Message,
                statusCode: 404,
                extensions: new Dictionary<string, object?> { ["traceId"] = traceId }),

            ConflictException cf => Results.Problem(
                title: "Conflict", detail: cf.Message,
                statusCode: 409,
                extensions: new Dictionary<string, object?> { ["traceId"] = traceId }),

            ValidationFailedException vf => Results.ValidationProblem(
                vf.Errors.ToDictionary(e => e.PropertyName, e => e.Errors.ToArray()),
                title: "Validation failed", detail: vf.Message,
                extensions: new Dictionary<string, object?> { ["traceId"] = traceId }),

            UnauthorizedAccessException => Results.Problem(
                title: "Unauthorized", statusCode: 401,
                extensions: new Dictionary<string, object?> { ["traceId"] = traceId }),

            _ => Results.Problem(
                title: "An unexpected error occurred", statusCode: 500,
                extensions: new Dictionary<string, object?> { ["traceId"] = traceId })
        };
    }
}

public abstract class DomainException(string message) : Exception(message);
public sealed class NotFoundException(string message) : DomainException(message);
public sealed class ConflictException(string message) : DomainException(message);
public sealed class ValidationFailedException(
    string message, List<ValidationError> errors) : DomainException(message)
{
    public List<ValidationError> Errors { get; } = errors;
}
public sealed record ValidationError(string PropertyName, string[] Errors);
```

```csharp
app.MapPost("/orders", async (CreateOrderCommand cmd, OrderService service, HttpContext ctx) =>
{
    try { return Results.Created($"/orders/{await service.CreateAsync(cmd)}", cmd); }
    catch (DomainException ex) { return ex.ToProblemResult(ctx); }
});
```

---

## 4. Validation Error Responses

FluentValidation → `ProblemDetails` per-field.

```csharp
builder.Services.AddValidatorsFromAssemblyContaining<CreateOrderValidator>();
builder.Services.AddTransient<IValidationInterceptor, ProblemDetailsValidationInterceptor>();

public sealed class ProblemDetailsValidationInterceptor : IValidationInterceptor
{
    public IValidationContext BeforeAspNetValidation(ActionContext _, IValidationContext ctx) => ctx;
    public ValidationResult AfterAspNetValidation(ActionContext _, IValidationContext __, ValidationResult r) => r;
    public Task<IValidationContext> BeforeValidationAsync(HttpContext _, IValidationContext ctx) => Task.FromResult(ctx);

    public async Task<ValidationResult> AfterValidationAsync(
        HttpContext httpContext, IValidationContext _, ValidationResult validationResult)
    {
        if (!validationResult.IsValid)
        {
            await Results.ValidationProblem(
                validationResult.ToDictionary(),
                title: "One or more validation errors occurred",
                extensions: new Dictionary<string, object?>
                {
                    ["traceId"] = Activity.Current?.Id ?? httpContext.TraceIdentifier
                }).ExecuteAsync(httpContext);
        }
        return validationResult;
    }
}
```

Response:

```json
{
  "type": "https://tools.ietf.org/html/rfc4918#section-11.2",
  "title": "One or more validation errors occurred",
  "status": 422,
  "traceId": "00-def456...",
  "errors": {
    "Email": ["'Email' is not a valid email address."],
    "OrderLines": ["At least one order line is required."]
  }
}
```

---

## 5. Custom Error Types

Sealed record hierarchy for consistent shapes:

```csharp
public sealed record ErrorType
{
    public string Code { get; init; } = "";
    public string Message { get; init; } = "";
    public string? Details { get; init; }
    public Dictionary<string, object?> Extensions { get; init; } = new();

    public static ErrorType NotFound(string resource, string id) => new()
    {
        Code = "NOT_FOUND", Message = $"{resource} with ID '{id}' was not found."
    };

    public static ErrorType Conflict(string message, string? details = null) => new()
    {
        Code = "CONFLICT", Message = message, Details = details
    };

    public static ErrorType ValidationFailed(
        Dictionary<string, string[]> fieldErrors, string? details = null) => new()
    {
        Code = "VALIDATION_FAILED",
        Message = "One or more validation errors occurred.",
        Details = details, Extensions = new() { ["fields"] = fieldErrors }
    };

    public static ErrorType RateLimited(int retryAfterSeconds) => new()
    {
        Code = "RATE_LIMITED", Message = "Too many requests. Retry later.",
        Extensions = new() { ["retryAfterSeconds"] = retryAfterSeconds }
    };
}
```

---

## 6. Error Correlation

Correlation ID on every request → logs, metrics, client reports align.

```csharp
public class CorrelationIdMiddleware(RequestDelegate next)
{
    private const string HeaderName = "X-Correlation-ID";

    public async Task InvokeAsync(HttpContext context)
    {
        var correlationId = context.Request.Headers.TryGetValue(HeaderName, out var incoming)
            ? incoming.ToString()
            : Guid.NewGuid().ToString("N")[..16];

        context.Response.Headers.Append(HeaderName, correlationId);

        using (LogContext.PushProperty("CorrelationId", correlationId))
            await next(context);
    }
}

// Serilog enrichment
builder.Services.AddSerilog(cfg => cfg
    .Enrich.FromLogContext()
    .Enrich.WithCorrelationId(headerName: "X-Correlation-ID", propertyName: "CorrelationId")
    .WriteTo.Console(
        "[{Timestamp:HH:mm:ss} {Level:u3}] [{CorrelationId}] {Message:lj}{NewLine}{Exception}"));
```

---

## 7. Not Found Handling

404 for unknown API routes:

```csharp
app.MapFallback("/api/{**catch-all}", (HttpContext ctx) =>
{
    var path = ctx.Request.Path.Value ?? "unknown";
    return Results.Problem(
        title: "Endpoint not found",
        detail: $"No API endpoint matches '{path}'.",
        statusCode: 404,
        type: "https://tools.ietf.org/html/rfc9110#section-15.5.5",
        extensions: new Dictionary<string, object?>
        {
            ["traceId"] = Activity.Current?.Id ?? ctx.TraceIdentifier,
            ["requestedPath"] = path
        });
});
```

---

## 8. Rate Limit Responses

Custom 429 with `Retry-After` + policy metadata:

```csharp
builder.Services.AddRateLimiter(options =>
{
    options.AddFixedWindowLimiter("orders", opt =>
    {
        opt.Window = TimeSpan.FromMinutes(1);
        opt.PermitLimit = 30;
        opt.QueueProcessingOrder = QueueProcessingOrder.OldestFirst;
        opt.QueueLimit = 5;
    });

    options.OnRejected = async (context, token) =>
    {
        var retryAfter = context.Lease.TryGetMetadata(MetadataName.RetryAfter, out var secs)
            ? (int)secs.TotalSeconds : 60;

        context.HttpContext.Response.Headers.RetryAfter = retryAfter.ToString();

        await Results.Problem(
            title: "Too many requests",
            detail: $"Rate limit exceeded. Retry after {retryAfter} seconds.",
            statusCode: 429,
            extensions: new Dictionary<string, object?>
            {
                ["traceId"] = Activity.Current?.Id ?? context.HttpContext.TraceIdentifier,
                ["retryAfterSeconds"] = retryAfter,
                ["policyName"] = context.Lease.PolicyName
            }).ExecuteAsync(context.HttpContext);
    };
});
```

---

## 9. Structured Error Logging

Enough context to diagnose without raw traces:

```csharp
logger.LogError(exception,
    "Unhandled {ExceptionType} on {Method} {Path} from {User} Accept={Accept} CT={ContentType} Query={Query}",
    exception.GetType().Name,
    context.Request.Method, context.Request.Path,
    context.User.FindFirst(ClaimTypes.NameIdentifier)?.Value ?? "anonymous",
    context.Request.Headers.Accept, context.Request.ContentType,
    context.Request.QueryString);
```

Serilog JSON:

```json
{
  "Level": "Error",
  "Properties": {
    "ExceptionType": "NotFoundException",
    "Method": "GET", "Path": "/api/customers/abc123",
    "User": "usr_7f3a", "CorrelationId": "a1b2c3d4e5f6a7b8",
    "Exception": { "Type": "...", "Message": "...", "StackTrace": "..." }
  }
}
```

---

## 10. Client-Friendly Responses

Developers need traces. Users need plain messages. Switch on environment:

```csharp
public class ProblemDetailsOptionsSetup(IHostEnvironment env) : IConfigureOptions<ProblemDetailsOptions>
{
    public void Configure(ProblemDetailsOptions options)
    {
        options.CustomizeProblemDetails = ctx =>
        {
            ctx.ProblemDetails.Extensions["traceId"] =
                Activity.Current?.Id ?? ctx.HttpContext.TraceIdentifier;

            if (env.IsDevelopment())
            {
                var ex = ctx.HttpContext.Features.Get<IExceptionHandlerFeature>()?.Error;
                if (ex is not null)
                {
                    ctx.ProblemDetails.Extensions["exceptionType"] = ex.GetType().FullName;
                    ctx.ProblemDetails.Extensions["stackTrace"] = ex.ToString();
                    ctx.ProblemDetails.Extensions["exceptionMessage"] = ex.Message;
                }
            }
        };
    }
}

builder.Services.ConfigureOptions<ProblemDetailsOptionsSetup>();
```

Prod: `{"title": "An unexpected error occurred", "status": 500, "traceId": "..."}`. Dev adds `exceptionType`, `exceptionMessage`, `stackTrace`.

---

## 11. Anti-Patterns

**No stack traces in prod.**
```csharp
// BAD: leaks implementation details
catch (Exception ex) { return Results.Problem(detail: ex.ToString()); }
```

**No plain text errors.**
```csharp
// BAD: clients can't parse
catch (Exception ex) { return Results.Text(ex.Message, "text/plain", statusCode: 500); }
```

**No silent swallowing.**
```csharp
// BAD: 200 OK on failure
catch (Exception) { return Results.Ok(); }
```

**No exception filters in minimal APIs.**
```csharp
// BAD: [ExceptionFilter] doesn't work with minimal APIs
app.MapGet("/products/{id}", [ServiceFilter(typeof(CustomExceptionFilter))] () => ...);
```

**No inconsistent shapes.**
```csharp
// BAD: { error: "..." } vs { message: "..." } per endpoint
// Always ProblemDetails as single contract.
```

**No non-standard status codes.**
```csharp
// BAD: what is 460?
return Results.StatusCode(460);
// GOOD: standard code + ProblemDetails
return Results.Problem(title: "Quota exceeded", statusCode: 429);
```
