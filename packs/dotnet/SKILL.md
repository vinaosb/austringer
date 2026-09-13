---
name: dotnet
description: ".NET pack seeded from the maintainer's real skill library: Clean Architecture layout, EF Core/Npgsql data access, ASP.NET Core health checks and structured logging, distributed caching, ProblemDetails error contracts, SignalR, Quartz scheduling, and Keycloak OIDC."
license: MIT OR Apache-2.0
version: "1.0.0"
languages: ["C#", "F#", "VB.NET", "SQL"]
allowed-tools: ["view_window", "edit_lines", "execute_command", "lsp_query", "tool_search"]
triggers:
  - asp.net core
  - ef core
  - entity framework
  - .net
  - c#
  - csharp
  - blazor
  - signalr
  - minimal api
  - clean architecture
  - keycloak
  - xunit
  - npgsql
  - quartz
  - idistributedcache
  - csproj
  - program.cs
  - wolverine
  - aplicação .net
  - servicio .net
do-not-trigger-on:
  - react component
  - kubernetes manifest
  - electrical panel
  - lease agreement
  - trial balance
  - journal entries
  - financial model
  - sprint retrospective
model-guidance:
  glm-5: Prefer minimal diffs matching the project's existing conventions; run `dotnet build` before claiming an edit is done.
---

# .NET pack

{{shared:pack-conventions}}

Distilled from the maintainer's own .NET skills (clean-arch, health
checks, EF Core/Npgsql, caching, error handling, SignalR, Quartz,
Keycloak). Workflow guidance, not boilerplate generation.

## Workflow

1. **Orient before editing**: read `Program.cs`, the `.csproj` files,
   and one representative handler/entity to learn the project's real
   conventions (DI style, mediator/Wolverine usage, naming).
2. **Layering**: Domain (entities, value objects) ← Application (CQRS
   handlers, interfaces) ← Infrastructure (EF Core, external services) ←
   Host. Dependencies point inward; never reference Infrastructure from
   Application.
3. **Data access (EF Core/Npgsql)**: entity configuration at module
   root; migrations authored and reviewed like code (`dotnet ef
   migrations add`); jsonb columns mapped explicitly; encryption via
   pgcrypto at the column level, never in application string handling.
4. **Health & observability**: health-check endpoints split
   readiness/liveness; Serilog structured logging with stable message
   templates; OpenTelemetry spans across handler boundaries.
5. **Error contracts**: return `ProblemDetails` (RFC 7807) from all
   failure paths — one exception-to-problem mapping, never per-endpoint
   ad-hoc error shapes.
6. **Caching**: cache-aside via `IDistributedCache`; explicit expiration
   (sliding + absolute); stampede guard on hot keys; invalidate by tag
   on write.
7. **Background jobs (Quartz)**: idempotent job handlers, persistent
   job store, misfire policy explicit, distributed lock when more than
   one instance runs.
8. **Auth (Keycloak OIDC)**: cookie + OIDC + JWT bearer multi-scheme;
   realm roles mapped to policies; validate audience/issuer at startup,
   not per-request.
9. **Tests**: xUnit; unit tests next to the unit, integration tests via
   Testcontainers (real PostgreSQL), one E2E smoke per surface.

## Worked example

Request: *"This endpoint returns 500 with a stack trace — fix the error
contract."*

Expected shape of the fix (minimal diff):

```csharp
// Program.cs — one mapping, all failure paths flow through it
app.UseExceptionHandler(pb => pb.Run(ctx => ProblemDetails.Write(ctx)));

// handler failure path
return Problem(title: "Invoice not found", status: 404, detail: invoiceId);
```

```http
HTTP/1.1 404 Not Found
Content-Type: application/problem+json

{"type":"https://tools.ietf.org/html/rfc9110#section-15.5.5",
 "title":"Invoice not found","status":404}
```

No stack trace leaves the service; the log carries the exception with
the correlation id.

## Eval scenario

Prompt: `"Help me set up a .NET clean architecture project with EF Core
and CQRS"`. Expected routing: `dotnet` (triggers: `clean architecture`,
`ef core`, `.net`). First actions: read the existing solution layout if
any, propose the four-project split, then scaffold one vertical slice
(entity + handler + mapping + test) instead of a full boilerplate dump.
