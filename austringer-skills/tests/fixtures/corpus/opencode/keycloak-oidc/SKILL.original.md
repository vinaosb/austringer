---
name: keycloak-oidc
description: "Keycloak OIDC authentication and authorization patterns for ASP.NET Core. Cookie + OIDC + JWT Bearer multi-scheme, realm role mapping, admin REST API provisioning, health checks, startup validation. USE FOR: Keycloak integration, OIDC setup, SSO, realm role claims, user provisioning via Keycloak admin API, multi-scheme auth (cookie + bearer), logout flow, token validation. DO NOT USE FOR: IdentityServer, Auth0, Azure AD, non-Keycloak IdP, frontend-only auth."
license: MIT
metadata:
  author: community
  version: "2.0.0"
  frameworks: [".NET", "Keycloak"]
---

# Keycloak OIDC for ASP.NET Core

Production-ready patterns for integrating Keycloak as your identity provider in .NET applications.

## Architecture Overview

Three authentication schemes work together:

1. **Cookie** (default) — holds the user session after OIDC callback. Used by Blazor Server, MVC, and Razor Pages.
2. **OpenID Connect** (challenge) — redirects unauthenticated users to Keycloak's login page. Authorization-code flow with PKCE.
3. **JWT Bearer** (additional) — validates access tokens for API-only callers hitting minimal API endpoints.

Flow: browser hits a protected page, gets challenged via OIDC, authenticates at Keycloak, returns with an auth code, code is exchanged for tokens, tokens are saved in the cookie, userinfo is fetched and mapped to claims.

> **Note:** JWT Bearer scheme is optional. For Blazor Server applications that only serve browser-based users, Cookie + OIDC is sufficient. JWT Bearer is needed only when you have API clients (mobile apps, SPA with separate backend, third-party integrations) that need to authenticate via access tokens rather than browser cookies.

## ASP.NET Core Configuration

```csharp
builder.Services.AddAuthentication(options =>
{
    options.DefaultScheme = CookieAuthenticationDefaults.AuthenticationScheme;
    options.DefaultChallengeScheme = OpenIdConnectDefaults.AuthenticationScheme;
})
.AddCookie()
.AddOpenIdConnect(options =>
{
    options.Authority = configuration["Keycloak:Authority"];
    options.ClientId = configuration["Keycloak:ClientId"];
    options.ClientSecret = configuration["Keycloak:ClientSecret"];
    options.ResponseType = "code";
    options.SaveTokens = true;
    options.GetClaimsFromUserInfoEndpoint = true;

    options.Scope.Add("openid");
    options.Scope.Add("profile");
    options.Scope.Add("email");
    options.Scope.Add("roles");

    options.TokenValidationParameters = new TokenValidationParameters
    {
        NameClaimType = "preferred_username",
        RoleClaimType = "realm_roles",
        ValidIssuer = configuration["Keycloak:Authority"],
        ValidAudience = configuration["Keycloak:ClientId"],
    };
});
```

`SaveTokens = true` stores access/refresh/id tokens in the cookie. `GetClaimsFromUserInfoEndpoint = true` merges profile claims from Keycloak's userinfo endpoint into the principal.

## Realm Role to ClaimTypes.Role Mapping

Keycloak puts roles in the `realm_roles` claim when the `roles` scope is requested. Standard `[Authorize(Roles = "...")]` expects `ClaimTypes.Role`, so you need to bridge them.

Setting `RoleClaimType = "realm_roles"` on `TokenValidationParameters` is the simplest approach and works for most cases.

For finer control, remap via `OnTokenValidated`:

```csharp
options.Events.OnTokenValidated = ctx =>
{
    var realmRoles = ctx.Principal?.FindAll("realm_roles").ToList();
    if (realmRoles is not null)
    {
        var identity = ctx.Principal!.Identity as ClaimsIdentity;
        foreach (var role in realmRoles)
            identity?.AddClaim(new Claim(ClaimTypes.Role, role.Value));
    }
    return Task.CompletedTask;
};
```

This runs after token validation, before the principal is stored in the cookie. Same pattern works for `resource_access.{clientId}.roles` if you use client-level roles.

### Claims Transformation for Multi-Tenancy

When multiple tenants share a single Keycloak realm, you need a centralized claims transformation that extracts roles, tenant identity, and custom attributes into standard claim types. Register via `IClaimsTransformation` so it applies to every authentication scheme:

```csharp
public sealed class KeycloakClaimsTransformation : IClaimsTransformation
{
    private const string RealmAccessClaimType = "realm_access";
    private const string IssuerPattern = "/realms/";

    public Task<ClaimsPrincipal> TransformAsync(ClaimsPrincipal principal)
    {
        var identity = principal.Identity as ClaimsIdentity;
        if (identity is null || !identity.IsAuthenticated)
            return Task.FromResult(principal);

        // Avoid re-running on already-transformed principals
        if (identity.HasClaim(c => c.Type == "tenant_id"))
            return Task.FromResult(principal);

        // 1. Extract realm roles from the realm_access JSON claim → ClaimTypes.Role
        var realmAccessClaim = identity.FindFirst(RealmAccessClaimType);
        if (realmAccessClaim is not null)
        {
            var realmAccess = JsonSerializer.Deserialize<RealmAccess>(realmAccessClaim.Value);
            if (realmAccess?.Roles is not null)
            {
                foreach (var role in realmAccess.Roles)
                    identity.AddClaim(new Claim(ClaimTypes.Role, role));
            }
        }

        // 2. Extract tenant from issuer URL: https://keycloak.example.com/realms/{realm-name}
        var issuer = identity.FindFirst("iss")?.Value
            ?? identity.FindFirst("aud")?.Value;
        if (issuer is not null)
        {
            var realmName = ExtractRealmFromIssuer(issuer);
            if (realmName is not null)
                identity.AddClaim(new Claim("tenant_id", realmName));
        }

        // 3. Map custom Keycloak attributes (country, locale, etc.)
        var country = identity.FindFirst("country")?.Value;
        if (country is not null)
            identity.AddClaim(new Claim("user_country", country));

        return Task.FromResult(principal);
    }

    private static string? ExtractRealmFromIssuer(string issuer)
    {
        var index = issuer.IndexOf(IssuerPattern, StringComparison.Ordinal);
        if (index < 0) return null;
        var realm = issuer[(index + IssuerPattern.Length)..];
        return string.IsNullOrWhiteSpace(realm) ? null : realm.TrimEnd('/');
    }

    private sealed record RealmAccess(List<string>? Roles);
}
```

Register in DI:

```csharp
builder.Services.AddScoped<IClaimsTransformation, KeycloakClaimsTransformation>();
```

This transformation runs on every request where the principal is materialized. The guard check for an existing `tenant_id` claim prevents duplicate role injection on repeated calls. Adjust the tenant extraction logic if your multi-tenancy model uses a custom claim instead of the issuer URL pattern.

## JWT Bearer for API Endpoints

For minimal APIs or non-browser clients, add JWT Bearer as a second scheme:

```csharp
builder.Services.AddAuthentication()
    .AddJwtBearer(options =>
    {
        options.Authority = configuration["Keycloak:Authority"];
        options.Audience = configuration["Keycloak:ClientId"];
        options.TokenValidationParameters = new TokenValidationParameters
        {
            RoleClaimType = "realm_roles",
            ValidIssuer = configuration["Keycloak:Authority"],
            ValidAudience = configuration["Keycloak:ClientId"],
        };
    });
```

> **Important:** `Microsoft.AspNetCore.Authentication.JwtBearer` is NOT included in the `Microsoft.AspNetCore.App` framework reference. You must add it as an explicit NuGet package. Use your project's central package management (`Directory.Packages.props`) for the correct version — always prefer the version referenced there over hardcoding a version in the project file.

Multi-scheme authorization. Controllers:

```csharp
[Authorize(AuthenticationSchemes = "Bearer,Cookie")]
public class MyApiController : ControllerBase { }
```

Minimal APIs:

```csharp
app.MapGet("/api/data", () => Results.Ok())
   .RequireAuthorization(new[] { "Bearer", "Cookie" });
```

### ApiScheme Policy for Per-Endpoint Opt-In

Register a named "ApiScheme" policy in your auth service registration for per-endpoint opt-in:

```csharp
builder.Services.AddAuthorization(options =>
{
    options.AddPolicy("ApiScheme", policy =>
    {
        policy.AddAuthenticationSchemes("Bearer");
        policy.RequireAuthenticatedUser();
    });
});
```

Apply to individual endpoints with `.RequireAuthorization("ApiScheme")`:

```csharp
app.MapGet("/api/secure-data", () => Results.Ok())
   .RequireAuthorization("ApiScheme");
```

> **Note:** Minimal API endpoints under `/api/auth/*` handle their own authorization (AllowAnonymous, RequireAuthorization) individually. You CANNOT blanket-apply the ApiScheme policy to the `MapAuthEndpoints()` return value — that method returns `IEndpointRouteBuilder`, not `IEndpointConventionBuilder`, so `.RequireAuthorization()` is not available on the chain.

## Keycloak Admin REST API: User Provisioning

Use password grant to obtain an admin token. This is only for server-to-server admin operations, never for regular user login.

```csharp
public interface IKeycloakAdminService
{
    Task<string> CreateUserAsync(string email, string firstName, string lastName,
        string temporaryPassword, string realmRole);
    Task DisableUserAsync(string keycloakUserId);
    Task AssignRealmRoleAsync(string keycloakUserId, string roleName);
}
```

Infrastructure implementation uses typed HttpClient. Request an admin token via password grant, then call the admin REST endpoints:

| Action | Method | Path |
|--------|--------|------|
| Create user | POST | `/admin/realms/{realm}/users` |
| Set password | PUT | `/admin/realms/{realm}/users/{id}/reset-password` |
| Assign realm role | POST | `/admin/realms/{realm}/users/{id}/role-mappings/realm` |
| Get user by email | GET | `/admin/realms/{realm}/users?email={email}` |
| Disable user | UPDATE | `/admin/realms/{realm}/users/{id}` (body: `enabled: false`) |

Register in `Program.cs`:

```csharp
builder.Services.AddHttpClient<IKeycloakAdminService, KeycloakAdminService>();
builder.Services.Configure<KeycloakAdminOptions>(
    configuration.GetSection("Keycloak:AdminApi"));
```

### Finbuckle Claim-Based Tenant Resolution

Wire Finbuckle multi-tenancy with Keycloak claims so tenant resolution happens automatically from the OIDC token:

```csharp
// MultiTenancyExtensions.cs - Wire Finbuckle multi-tenancy with Keycloak claims
services.AddMultiTenant<TenantInfo>()
    .WithClaimStrategy("tenant_id")  // Matches Keycloak user attribute
    .WithConfigurationStore();       // Or WithInMemoryStore() for development

// The tenant_id claim is already mapped by KeycloakClaimsTransformation (see section 2).
// Register it so Keycloak realm_roles map to ClaimTypes.Role:
services.AddTransient<IClaimsTransformation, KeycloakClaimsTransformation>();
```

`WithClaimStrategy("tenant_id")` tells Finbuckle to resolve the tenant from the `tenant_id` claim in the OIDC token. This claim must be configured as a user attribute in Keycloak and mapped to the ID token via a protocol mapper.

## Configuration Pattern

`appsettings.json` (committed, empty secrets):

```json
{
  "Keycloak": {
    "Authority": "",
    "ClientId": "",
    "ClientSecret": "",
    "RequireHttpsMetadata": true,
    "AdminApi": {
      "BaseUrl": "",
      "ClientId": "admin-cli",
      "ClientSecret": "",
      "Username": "",
      "Password": ""
    }
  }
}
```

Dev values go in `appsettings.Development.json` (gitignored). Production values come from environment variables or a secrets manager. Never commit admin credentials.

## Startup Validation

Fail fast on missing configuration before the app finishes building:

```csharp
var authority = configuration["Keycloak:Authority"];
var clientId = configuration["Keycloak:ClientId"];

if (string.IsNullOrWhiteSpace(authority))
    throw new InvalidOperationException("Keycloak:Authority is required in configuration.");

if (string.IsNullOrWhiteSpace(clientId))
    throw new InvalidOperationException("Keycloak:ClientId is required in configuration.");
```

Put this in a `StartupValidator` class called from `Program.cs`, or inline after `builder.Build()`. Crash immediately with a clear message rather than starting with a broken auth pipeline.

## Health Check

Probe Keycloak's discovery document to confirm it's reachable:

```csharp
builder.Services.AddHealthChecks()
    .AddUrlGroup(
        new Uri($"{authority}/.well-known/openid-configuration"),
        "keycloak", tags: ["ready"]);
```

Map endpoints:

```csharp
app.MapHealthChecks("/health/live", new HealthCheckOptions { Predicate = _ => false });
app.MapHealthChecks("/health/ready", new HealthCheckOptions
{
    Predicate = check => check.Tags.Contains("ready")
});
```

`/health/live` confirms the process is running. `/health/ready` confirms external dependencies (Keycloak, database) are reachable.

## Logout Flow

Sign out of the local cookie, then redirect to Keycloak's end_session_endpoint:

```csharp
public async Task Logout(HttpContext context, IConfiguration configuration)
{
    await context.SignOutAsync(CookieAuthenticationDefaults.AuthenticationScheme);

    var authority = configuration["Keycloak:Authority"]!;
    var redirectUri = Uri.EscapeDataString("https://yourapp.com");

    var logoutUrl =
        $"{authority}/protocol/openid-connect/logout" +
        $"?client_id={configuration["Keycloak:ClientId"]}" +
        $"&post_logout_redirect_uri={redirectUri}";

    context.Response.Redirect(logoutUrl);
}
```

Without the Keycloak redirect, the Keycloak session stays alive. Next visit silently re-authenticates the user. Always call end_session_endpoint.

> **Note:** The `context.Response.Redirect()` approach above can block the OIDC redirect flow. Instead, use the `OnRedirectToIdentityProviderForSignOut` event handler which properly delegates the redirect to the OIDC middleware:
>
> ```csharp
> .AddOpenIdConnect(options =>
> {
>     // ... other config ...
>     options.Events.OnRedirectToIdentityProviderForSignOut = ctx =>
>     {
>         ctx.ProtocolMessage.IssuerAddress =
>             $"{authority}/protocol/openid-connect/logout";
>         return Task.CompletedTask;
>     };
> });
> ```
>
> This sets the Keycloak end-session URL directly on the protocol message (e.g., `http://keycloak:8080/realms/{realm}/protocol/openid-connect/logout`). Do NOT use `context.Response.Redirect()` — it bypasses the middleware and blocks the redirect flow.

## Token Refresh / Sliding Expiration

The OIDC middleware handles access token refresh automatically when `SaveTokens = true`. The cookie stores the refresh token, and the middleware uses it to obtain new access tokens when the current one expires.

For explicit token refresh (e.g., in API calls):

```csharp
var accessToken = await context.HttpContext.GetTokenAsync("access_token");
var refreshToken = await context.HttpContext.GetTokenAsync("refresh_token");
```

**Sliding expiration**: Set `SlidingExpiration` on the cookie to keep sessions alive during active use:

```csharp
.AddCookie(options =>
{
    options.SlidingExpiration = true;
    options.ExpireTimeSpan = TimeSpan.FromHours(8);
});
```

**Absolute expiration**: Set `AbsoluteExpiration` to force re-authentication periodically, even during active use. This limits the window of a compromised session.

## PKCE (Proof Key for Code Exchange)

PKCE is enabled by default in ASP.NET Core's OIDC handler. It adds a `code_challenge` to the authorization request and verifies it when exchanging the code for tokens. This prevents authorization code interception attacks.

No additional configuration is needed for PKCE in standard ASP.NET Core + Keycloak setups. The middleware generates and validates the `code_verifier` and `code_challenge` automatically.

For native/mobile apps or public clients (no client secret), PKCE is mandatory. In server-side ASP.NET Core, it's enabled but less critical since the code exchange happens server-to-server.

## DI Registration Pattern

Wrap all Keycloak registrations in a service extension:

```csharp
public static class KeycloakServiceExtensions
{
    public static IServiceCollection AddKeycloakServices(
        this IServiceCollection services, IConfiguration configuration)
    {
        var authority = configuration["Keycloak:Authority"]
            ?? throw new InvalidOperationException("Keycloak:Authority is required.");

        services.AddAuthentication(/* cookie + oidc + bearer setup */);
        services.AddHttpClient<IKeycloakAdminService, KeycloakAdminService>();
        services.Configure<KeycloakAdminOptions>(
            configuration.GetSection("Keycloak:AdminApi"));
        services.AddHealthChecks()
            .AddUrlGroup(new Uri($"{authority}/.well-known/openid-configuration"), "keycloak");

        return services;
    }
}
```

Then: `builder.Services.AddKeycloakServices(builder.Configuration);`

## Multi-App SSO with Shared DataProtection

When multiple ASP.NET Core apps (e.g., per-module Blazor Server apps) share a single Keycloak realm for SSO, they must share DataProtection keys and use identical cookie settings. Without shared DataProtection, each app encrypts the auth cookie with its own key ring — a cookie written by App A is unreadable by App B, and SSO breaks.

### Prerequisites

- All apps live under the **same root domain** (e.g., `app1.mycompany.com`, `app2.mycompany.com`).
- All apps connect to the **same Redis instance** for DataProtection key storage.
- All apps reference the **same Keycloak realm** but have **different ClientIds**.

### Shared DataProtection via Redis

Install the NuGet package:

```xml
<PackageReference Include="Microsoft.AspNetCore.DataProtection.StackExchangeRedis" Version="*" />
```

Register in each app's `Program.cs`:

```csharp
var redis = ConnectionMultiplexer.Connect(
    configuration.GetConnectionString("Redis")!);

builder.Services.AddDataProtection()
    .PersistKeysToStackExchangeRedis(redis, "DataProtection-Keys")
    .SetApplicationName("MySharedAuthApp");
```

Key points:
- `PersistKeysToStackExchangeRedis` stores the key ring in Redis so all apps read/write the same keys.
- `SetApplicationName` must be **identical** across all participating apps. This scopes the key ring so apps sharing the same name can decrypt each other's cookies.
- The Redis key name (`"DataProtection-Keys"`) must be the same in every app.

### Cookie Configuration

All apps must use the **same cookie name** so the browser sends it to every subdomain:

```csharp
builder.Services.AddAuthentication(options =>
{
    options.DefaultScheme = CookieAuthenticationDefaults.AuthenticationScheme;
    options.DefaultChallengeScheme = OpenIdConnectDefaults.AuthenticationScheme;
})
.AddCookie(options =>
{
    options.Cookie.Name = ".MySharedAuth";
    options.Cookie.Domain = ".mycompany.com"; // shared root domain
    options.Cookie.SameSite = SameSiteMode.Lax;
    options.Cookie.HttpOnly = true;
    options.Cookie.SecurePolicy = CookieSecurePolicy.SameAsRequest;
    options.SlidingExpiration = true;
    options.ExpireTimeSpan = TimeSpan.FromHours(8);
})
.AddOpenIdConnect(options =>
{
    options.Authority = configuration["Keycloak:Authority"];
    options.ClientId = configuration["Keycloak:ClientId"]; // per-app, from appsettings
    options.ClientSecret = configuration["Keycloak:ClientSecret"];
    options.ResponseType = "code";
    options.SaveTokens = true;
    options.GetClaimsFromUserInfoEndpoint = true;

    options.Scope.Add("openid");
    options.Scope.Add("profile");
    options.Scope.Add("email");
    options.Scope.Add("roles");

    options.TokenValidationParameters = new TokenValidationParameters
    {
        NameClaimType = "preferred_username",
        RoleClaimType = "realm_roles",
        ValidIssuer = configuration["Keycloak:Authority"],
        ValidAudience = configuration["Keycloak:ClientId"],
    };
});
```

### Per-App OIDC ClientId

Each app reads its own `ClientId` from `appsettings.json`:

**App A — `appsettings.json`:**
```json
{
  "Keycloak": {
    "Authority": "https://keycloak.mycompany.com/realms/myrealm",
    "ClientId": "myapp-launchpad",
    "ClientSecret": ""
  }
}
```

**App B — `appsettings.json`:**
```json
{
  "Keycloak": {
    "Authority": "https://keycloak.mycompany.com/realms/myrealm",
    "ClientId": "myapp-accounting",
    "ClientSecret": ""
  }
}
```

Both point to the same realm (`myrealm`) but register as different clients. When a user authenticates on App A, the cookie is shared. When App B reads the cookie, it recognizes the session because the DataProtection keys and cookie name are identical.

### Why This Works

1. User visits App A → no cookie → OIDC challenge → Keycloak login → callback → encrypted cookie set on `.mycompany.com`.
2. User navigates to App B → browser sends the same cookie (shared domain) → App B decrypts it (shared DataProtection keys) → user is already authenticated.
3. App B can still challenge for its own client-specific tokens if needed, but the session is already established.

### Per-App Client Configuration

In Keycloak, create one **public client** per app within the same realm. Realm roles are shared across all clients by default.

**Realm export JSON format** (partial, for importing via Docker):

```json
{
  "realm": "myrealm",
  "enabled": true,
  "clients": [
    {
      "clientId": "myapp-launchpad",
      "enabled": true,
      "publicClient": true,
      "redirectUris": ["https://launchpad.mycompany.com/signin-oidc"],
      "webOrigins": ["https://launchpad.mycompany.com"],
      "standardFlowEnabled": true,
      "directAccessGrantsEnabled": false
    },
    {
      "clientId": "myapp-accounting",
      "enabled": true,
      "publicClient": true,
      "redirectUris": ["https://accounting.mycompany.com/signin-oidc"],
      "webOrigins": ["https://accounting.mycompany.com"],
      "standardFlowEnabled": true,
      "directAccessGrantsEnabled": false
    },
    {
      "clientId": "myapp-payments",
      "enabled": true,
      "publicClient": true,
      "redirectUris": ["https://payments.mycompany.com/signin-oidc"],
      "webOrigins": ["https://payments.mycompany.com"],
      "standardFlowEnabled": true,
      "directAccessGrantsEnabled": false
    }
  ],
  "roles": {
    "realm": [
      { "name": "admin", "description": "Full access" },
      { "name": "accountant", "description": "Accounting module access" },
      { "name": "viewer", "description": "Read-only access" }
    ]
  }
}
```

**Docker import** — mount the realm export and use `--import-realm`:

```yaml
services:
  keycloak:
    image: quay.io/keycloak/keycloak:26.0
    command: start-dev --import-realm
    environment:
      KC_DB: postgres
      KC_DB_URL: jdbc:postgresql://db:5432/keycloak
      KC_DB_USERNAME: keycloak
      KC_DB_PASSWORD: keycloak_password
      KEYCLOAK_ADMIN: admin
      KEYCLOAK_ADMIN_PASSWORD: admin
    volumes:
      - ./keycloak/realm-export.json:/opt/keycloak/data/import/realm-export.json
    ports:
      - "8080:8080"
```

Key points:
- All clients in one realm share the same user store and realm roles.
- Each client has its own `redirectUris` pointing to its specific domain.
- `publicClient: true` avoids managing client secrets for server-side Blazor apps (PKCE handles the flow).
- `--import-realm` is required for Docker-based startup to load the JSON realm file.
- In production, replace `start-dev` with `start` and configure TLS properly.

## Anti-Patterns

- **NEVER** use Resource Owner Password grant for regular user login. Only for server-to-server admin API access. Users authenticate via authorization-code flow.
- **NEVER** store admin credentials, client secrets, or tokens in committed `appsettings.json`. Use environment variables, user secrets (dev), or a vault (prod).
- **NEVER** set `SkipUnrecognizedRequests = true` or disable token validation to work around auth issues. That bypasses security.
- **NEVER** log OIDC tokens, refresh tokens, or admin credentials.
- **NEVER** run Keycloak with `start-dev` in production. It disables TLS, caching, and uses an in-memory database.
- **NEVER** hardcode realm URLs, client IDs, or Keycloak addresses in source code. Always use configuration.
- **DO NOT** use `[AllowAnonymous]` on sensitive endpoints as a dev shortcut. It will ship to production.
- **DO NOT** rely solely on client-side role checks. Always validate roles server-side.
- **DO NOT** use different `SetApplicationName` values across SSO apps. Shared DataProtection requires the same application name or cookies are undecryptable across apps.
- **DO NOT** hardcode client IDs in `Program.cs`. Each app should read its `ClientId` from `appsettings.json` so the same DI extension code is shared across all apps.
- **DO NOT** forget `--import-realm` when running Keycloak in Docker with a realm export file. Without it, Keycloak starts with an empty `master` realm and your clients are missing.
- **DO NOT** use different cookie names or domains across SSO apps. The browser will not send the cookie to sibling apps, defeating SSO entirely.
- **NEVER** forget to register KeycloakClaimsTransformation as IClaimsTransformation. Without it, Keycloak `realm_roles` won't map to `ClaimTypes.Role` and `[Authorize(Roles = "...")]` checks silently fail because ASP.NET Core doesn't recognize Keycloak's custom claim format.
- **NEVER** hardcode tenant IDs in claim strategies. Use configuration: `WithClaimStrategy(configuration["MultiTenancy:ClaimKey"] ?? "tenant_id")`. The claim name might differ between Keycloak realms.

## Related Skills

- **dotnet-clean-arch** — Project structure where Keycloak is configured in Program.cs and auth services registered in InfrastructureModule
- **docker-deployment** — Docker Compose patterns for running Keycloak alongside multi-app architectures with shared infrastructure
- **blazor-mudblazor** — Authentication state revalidation in Blazor components after OIDC callback, per-app NavMenu patterns
- **lgpd-data-privacy** — User identity tracking for audit trails, keyed by Keycloak user ID, consent lifecycle integration
- **aspnetcore-health** — Keycloak health check via OIDC discovery endpoint probe
- **signalr-realtime** — Hub authentication using the same cookie+OIDC+bearer schemes
