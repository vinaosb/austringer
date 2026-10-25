---
name: docker-deployment
description: "Docker Compose deployment patterns for .NET applications. Overlay-based multi-environment configs, nginx reverse proxy, TLS (self-signed + Let's Encrypt), encrypted backup/restore, health checks, CI/CD integration. USE FOR: Docker Compose setup, multi-environment deployment, nginx config, SSL/TLS, backup scripts, health check endpoints, CI/CD pipeline, Blazor/SignalR proxy configuration, container orchestration. DO NOT USE FOR: Kubernetes, AWS/Azure cloud-native deployment, serverless, non-Docker deployment."
license: MIT
metadata:
  author: community
  version: "1.0.0"
  frameworks: [".NET", "Docker", "Nginx"]
---

# Docker Deployment Skill

Reusable patterns for deploying .NET applications with Docker Compose across multiple environments.

## 1. Docker Compose Overlay Pattern

### Base Compose (`docker-compose.yml`)

Infrastructure services shared across all environments. No application containers.

```yaml
services:
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_DB: ${POSTGRES_DB:-appdb}
      POSTGRES_USER: ${POSTGRES_USER:-appuser}
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:-changeme}
    volumes:
      - postgres-data:/var/lib/postgresql/data
    networks:
      - infra
  dragonfly:
    image: docker.dragonflydb.io/dragonfly/dragonfly:latest
    volumes:
      - dragonfly-data:/data
    networks:
      - infra
volumes:
  postgres-data:
  dragonfly-data:
networks:
  infra:
    driver: bridge
```

### docker-compose.override.yml Auto-Application

`docker-compose.override.yml` is automatically applied by Docker Compose without needing a `-f` flag — no need to explicitly reference it. This is the perfect place for dev-specific resource limits and debug port mappings:

```yaml
# docker-compose.override.yml — auto-applied by docker compose up
services:
  app:
    ports:
      - "5000:8080"      # debug port for attach
    environment:
      ASPNETCORE_ENVIRONMENT: Development
    deploy:
      resources:
        limits:
          cpus: '2'
          memory: 4G
  postgres:
    ports:
      - "5432:5432"      # direct DB access for migrations/tools
```

Run with simply `docker compose up -d` — the override is picked up automatically. Override files are environment-specific and should not be used for production config (use explicit overlay files with `-f` for that).

### Per-Environment Overlays

Each overlay (`docker/docker-compose.{dev,demo,stage,prod}.yml`) adds runtime services and overrides base env vars. They never duplicate base services.

```yaml
# docker/docker-compose.prod.yml
services:
  postgres:
    environment:
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:?Database password is required}
  app:
    image: registry.example.com/app:${IMAGE_TAG}
    environment:
      ASPNETCORE_ENVIRONMENT: Production
      ConnectionStrings__Database: ${DATABASE_URL:?Database URL is required}
    depends_on:
      postgres:
        condition: service_healthy
    networks:
      - infra
  nginx:
    image: nginx:alpine
    ports:
      - "443:443"
    volumes:
      - ./nginx/prod.conf:/etc/nginx/conf.d/default.conf:ro
      - certbot-conf:/etc/letsencrypt
    depends_on:
      - app
    networks:
      - infra
```

**Usage:**

```bash
docker compose -f docker-compose.yml -f docker/docker-compose.prod.yml up -d
```

### Multi-App Compose Pattern

For suites of related applications (e.g. per-module Blazor Server apps) sharing infrastructure. Uses a single Dockerfile with build args to produce N images from one template.

```yaml
# docker-compose.yml (base: infra + apps)
services:
  # ── Infrastructure ──
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_DB: ${POSTGRES_DB:-appdb}
      POSTGRES_USER: ${POSTGRES_USER:-appuser}
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:-changeme}
    volumes:
      - postgres-data:/var/lib/postgresql/data
      - ./postgres-conf/postgresql.conf:/etc/postgresql/postgresql.conf:ro
    command: postgres -c config_file=/etc/postgresql/postgresql.conf
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ${POSTGRES_USER:-appuser}"]
      interval: 10s
      timeout: 5s
      retries: 5
    networks:
      - infra

  redis:
    image: docker.dragonflydb.io/dragonfly/dragonfly:latest
    volumes:
      - redis-data:/data
    networks:
      - infra

  keycloak:
    image: quay.io/keycloak/keycloak:26.0
    command: start --import-realm
    environment:
      KC_DB: postgres
      KC_DB_URL_HOST: postgres
      KC_DB_USERNAME: ${POSTGRES_USER:-appuser}
      KC_DB_PASSWORD: ${POSTGRES_PASSWORD:-changeme}
      KEYCLOAK_ADMIN: ${KEYCLOAK_ADMIN:-admin}
      KEYCLOAK_ADMIN_PASSWORD: ${KEYCLOAK_ADMIN_PASSWORD:-changeme}
    volumes:
      - ./keycloak/realm-export.json:/opt/keycloak/data/import/realm.json:ro
    depends_on:
      postgres:
        condition: service_healthy
    networks:
      - infra

  # ── Applications (one per module) ──
  # All built from the same Dockerfile via build args
  app-portal:
    build:
      context: .
      dockerfile: Dockerfile
      args:
        APP_PROJECT: src/App.Portal/App.Portal.csproj
        APP_DLL: App.Portal.dll
    env_file: .env.apps
    environment:
      ASPNETCORE_URLS: http://+:8080
    ports:
      - "5101:8080"
    depends_on:
      postgres:
        condition: service_healthy
      redis:
        condition: service_started
      keycloak:
        condition: service_started
    networks:
      - infra

  app-accounting:
    build:
      context: .
      dockerfile: Dockerfile
      args:
        APP_PROJECT: src/App.Accounting/App.Accounting.csproj
        APP_DLL: App.Accounting.dll
    env_file: .env.apps
    environment:
      ASPNETCORE_URLS: http://+:8080
    ports:
      - "5102:8080"
    depends_on:
      postgres:
        condition: service_healthy
      redis:
        condition: service_started
    networks:
      - infra

  # Add more apps following the same pattern...
  # app-billing:
  #   build:
  #     context: .
  #     dockerfile: Dockerfile
  #     args:
  #       APP_PROJECT: src/App.Billing/App.Billing.csproj
  #       APP_DLL: App.Billing.dll
  #   env_file: .env.apps
  #   ...

volumes:
  postgres-data:
  redis-data:

networks:
  infra:
    driver: bridge
```

**Shared env file** (`.env.apps`) avoids repeating connection strings across every service:

```bash
# .env.apps — shared by all app services
ConnectionStrings__Database=Host=postgres;Database=appdb;Username=appuser;Password=changeme
ConnectionStrings__Redis=redis:6379
Keycloak__Authority=http://keycloak:8080/realms/master
Keycloak__ClientId=app-suite
```

**Key patterns:**

- **One Dockerfile, N apps** — `ARG APP_PROJECT` / `ARG APP_DLL` avoids duplicating Dockerfiles.
- **`env_file` sharing** — connection strings, Redis URL, and OIDC config live in one file.
- **Per-app port mapping** — dev overlay exposes `5101:8080`, `5102:8080`, etc.; prod uses reverse proxy instead.
- **`pg_isready` healthcheck** — apps wait for Postgres to accept connections before starting.
- **`--import-realm`** — Keycloak bootstraps its realm from a mounted JSON export on first start.

## 2. Environment Variable Validation

Escalate strictness from dev to prod. Never commit real secrets.

| Environment | Pattern | Behavior |
|---|---|---|
| dev/demo | `${VAR:-default}` | Uses default if missing. |
| stage | Mix of both | Critical vars required, others default. |
| prod | `${VAR:?error message}` | Fails immediately if missing. |

```yaml
# dev overlay
environment:
  ASPNETCORE_ENVIRONMENT: ${ASPNETCORE_ENVIRONMENT:-Development}
  LOG_LEVEL: ${LOG_LEVEL:-Debug}
# prod overlay
environment:
  ASPNETCORE_ENVIRONMENT: ${ASPNETCORE_ENVIRONMENT:?ASPNETCORE_ENVIRONMENT required}
  DATABASE_URL: ${DATABASE_URL:?DATABASE_URL required}
  ENCRYPTION_KEY: ${ENCRYPTION_KEY:?ENCRYPTION_KEY required}
```

Base `appsettings.json` uses empty strings for secrets. Real values arrive at deploy time through env vars or a secrets manager.

## 3. Nginx Reverse Proxy

One config per environment with env-specific upstreams, server_names, and TLS.

```nginx
# nginx/prod.conf
upstream app {
    server app:8080;
}
server {
    listen 443 ssl http2;
    server_name app.example.com;
    ssl_certificate /etc/letsencrypt/live/app.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/app.example.com/privkey.pem;

    add_header Strict-Transport-Security "max-age=63072000; includeSubDomains" always;
    add_header X-Frame-Options DENY always;

    # Blazor Server / SignalR WebSocket support
    location / {
        proxy_pass http://app;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_read_timeout 3600s;
        proxy_buffering off;
    }
    # SPA / WASM fallback
    location /app/ {
        proxy_pass http://app;
        try_files $uri $uri/ /app/index.html;
    }
    # Static assets
    location ~* \.(css|js|png|jpg|svg|woff2)$ {
        proxy_pass http://app;
        expires 1y;
        add_header Cache-Control "public, immutable";
    }
}
```

Blazor Server and SignalR require WebSocket upgrade headers, long read timeouts, and disabled buffering.

### ASP.NET Core Forwarded Headers Configuration

When running behind a reverse proxy, the .NET app must be configured to trust forwarded headers. Two critical rules:

**1. `KnownNetworks` is obsolete in .NET 10 (ASPDEPR005)**
`ForwardedHeadersOptions.KnownNetworks` is deprecated. Use `KnownIPNetworks` instead. If your project has `TreatWarningsAsErrors` enabled, this becomes a build error:
```csharp
// .NET 10+ (correct)
options.KnownIPNetworks.Clear();
options.KnownIPNetworks.Add(new AspNetCore.HttpOverrides.IPNetwork(IPAddress.Loopback, 8));

// .NET 9 and earlier (obsolete in .NET 10)
options.KnownNetworks.Clear();  // ASPDEPR005
```

**2. `UseForwardedHeaders()` ordering in middleware pipeline**
Must be called *before* `UseHsts()`, `UseHttpsRedirection()`, and `UseAuthentication()`:
```csharp
var app = builder.Build();

app.UseForwardedHeaders();     // 1st — process X-Forwarded-* headers
app.UseHsts();                 // 2nd — depends on forwarded scheme
app.UseHttpsRedirection();     // 3rd — depends on forwarded scheme
app.UseAuthentication();       // 4th — depends on forwarded user identity
// ... remaining middleware
```
ASP.NET Core docs require this ordering for correct header processing. Getting it wrong causes redirect loops, broken auth, or incorrect scheme detection.

### Caddy Reverse Proxy

Caddy is an alternative to nginx that provides automatic HTTPS via Let's Encrypt with zero configuration — no certbot sidecar needed. It is particularly well-suited for multi-app deployments behind path-based routing.

```Caddyfile
# Caddyfile
{$DOMAIN:localhost} {
    # Automatic HTTPS — Caddy provisions and renews certs via ACME/Let's Encrypt.
    # No certbot sidecar required. Disable for local dev with: tls internal

    # Identity provider
    handle_path /auth/* {
        reverse_proxy keycloak:8080
    }

    # Per-module Blazor Server apps (WebSocket support is automatic)
    handle_path /portal/* {
        reverse_proxy app-portal:8080
    }

    handle_path /accounting/* {
        reverse_proxy app-accounting:8080
    }

    # Add more app routes as needed:
    # handle_path /billing/* {
    #     reverse_proxy app-billing:8080
    # }

    # Default route — redirect to portal
    handle {
        redir /portal/ permanent
    }

    # Block admin endpoints from external access
    handle /admin* {
        respond "Forbidden" 403
    }
}
```

**Docker Compose integration:**

```yaml
services:
  caddy:
    image: caddy:2-alpine
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile:ro
      - caddy-data:/data        # persisted certs and keys
      - caddy-config:/config    # auto-generated config
    depends_on:
      - app-portal
      - app-accounting
      - keycloak
    networks:
      - infra

volumes:
  caddy-data:
  caddy-config:
```

**Key advantages over nginx:**

- **Automatic HTTPS** — provisions and renews Let's Encrypt certificates without certbot.
- **WebSocket support** — enabled by default; no manual upgrade headers.
- **Simpler config** — one file, no upstream blocks, no separate SSL directives.
- **`handle_path`** strips the matched prefix before forwarding, so Blazor apps receive clean paths.

**When to choose Caddy over nginx:** Caddy is ideal when you want automatic TLS with minimal config. Choose nginx when you need advanced features like rate limiting, complex rewrite rules, or when your team already has nginx expertise. Do not use both simultaneously — pick one reverse proxy per deployment.

## 4. TLS/SSL

### Dev and Demo (Self-Signed)

Generate at image build time or via entrypoint. For nginx, use a named volume so certs persist across restarts:

```bash
# nginx entrypoint or init script
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
  -keyout /etc/nginx/certs/server.key \
  -out /etc/nginx/certs/server.crt \
  -subj "/CN=localhost"
```

```yaml
# docker-compose.yml
services:
  nginx:
    volumes:
      - nginx-certs:/etc/nginx/certs

volumes:
  nginx-certs:
```

The `nginx-certs` named volume persists across `docker compose down`/`up` cycles, avoiding browser certificate warnings on every restart. For ephemeral dev environments, skip the volume and generate fresh certs each time.

Manual one-shot generation:

```bash
openssl req -x509 -newkey rsa:4096 -nodes \
  -keyout key.pem -out cert.pem -days 365 \
  -subj "/CN=localhost"
```

### Stage and Prod (Let's Encrypt)

Certbot sidecar with shared volumes for ACME challenge and certificates.

```yaml
services:
  nginx:
    volumes:
      - ./nginx/prod.conf:/etc/nginx/conf.d/default.conf:ro
      - certbot-www:/var/www/certbot:ro
      - certbot-conf:/etc/letsencrypt
  certbot:
    image: certbot/certbot
    volumes:
      - certbot-www:/var/www/certbot
      - certbot-conf:/etc/letsencrypt
    entrypoint: >
      sh -c "certbot certonly --webroot --webroot-path /var/www/certbot
      --email ${CERTBOT_EMAIL:?CERTBOT_EMAIL required}
      --agree-tos -d ${DOMAIN:?DOMAIN required}
      && trap exit TERM; while :; do sleep 6h & wait; certbot renew; done"
volumes:
  certbot-www:
  certbot-conf:
```

ACME challenge in nginx:

```nginx
location /.well-known/acme-challenge/ {
    root /var/www/certbot;
}
```

Production hardening: HSTS, OCSP stapling, TLSv1.2+, strong ciphers, gzip, CSP.

```nginx
ssl_protocols TLSv1.2 TLSv1.3;
ssl_ciphers HIGH:!aNULL:!MD5;
ssl_prefer_server_ciphers on;
ssl_stapling on;
ssl_stapling_verify on;
gzip on;
gzip_types text/plain application/json text/css application/javascript;
```

### Content Security Policy (CSP)

Move CSP headers from hardcoded inline nginx strings to `appsettings.json` so they can be overridden per-environment. The .NET app emits the CSP header, not nginx.

```json
// appsettings.json (base)
{
  "ContentSecurityPolicy": {
    "DefaultSrc": "'self'",
    "ScriptSrc": "'self' 'unsafe-eval' https://cdn.example.com",
    "StyleSrc": "'self' 'unsafe-inline' https://fonts.googleapis.com",
    "FontSrc": "'self' https://fonts.gstatic.com",
    "ImgSrc": "'self' data:",
    "ConnectSrc": "'self' https://api.example.com wss://app.example.com",
    "FrameAncestors": "'none'",
    "BaseUri": "'self'"
  }
}
```

```json
// appsettings.Production.json (stricter production overrides)
{
  "ContentSecurityPolicy": {
    "ScriptSrc": "'self'",
    "ConnectSrc": "'self' wss://app.example.com"
  }
}
```

In the .NET application, build the CSP header string from these settings and emit it via middleware or the ASP.NET Core security header middleware. This allows per-environment relaxation (dev needs `'unsafe-eval'` for Blazor debug, prod drops it). The environment-specific override lives in the appsettings overlay — no nginx config changes needed.

## 5. Encrypted Backup/Restore

### Backup Flow

```bash
pg_dump -Fc "$DATABASE_URL" \
  | openssl enc -aes-256-cbc -salt -pbkdf2 \
    -pass pass:"$BACKUP_KEY" \
  > "/backups/$(date +%Y%m%d_%H%M%S).enc"

# Auto-cleanup: remove backups older than retention period (default: 7 days)
find /backups -name "*.enc" -mtime +7 -type f -delete
```

### Restore Flow

```bash
openssl enc -d -aes-256-cbc -pbkdf2 \
  -pass pass:"$BACKUP_KEY" \
  -in "$BACKUP_FILE" \
  | pg_restore -d "$DATABASE_URL" --no-owner --no-acl
```

### Full Verification and Scheduling

Verification script: encrypt test dump, decrypt, restore to temp DB, compare row counts, clean up. Run on a schedule to catch corruption early.

Backup container uses cron. Entrypoint writes schedule at container start:

```bash
#!/bin/sh
echo "$BACKUP_SCHEDULE /usr/local/bin/backup.sh >> /var/log/cron.log 2>&1" | crontab -
echo "$VERIFY_SCHEDULE /usr/local/bin/verify.sh >> /var/log/cron.log 2>&1" | crontab -
crond -f
```

Backup keys come from env vars or a secrets manager. Never hardcode them in compose files.

## 6. Health Check Endpoints

```csharp
builder.Services.AddHealthChecks()
    .AddNpgSql(connectionString, tags: ["ready"])
    .AddRedis(redisConnectionString, tags: ["ready"])
    .AddUrlGroup(new Uri(keycloakMetadataUrl), "keycloak", tags: ["ready"])
    .AddCheck("live", () => HealthCheckResult.Healthy(), tags: ["live"]);

app.MapHealthChecks("/health/live", new HealthCheckOptions
{
    Predicate = check => check.Tags.Contains("live")
});
app.MapHealthChecks("/health/ready", new HealthCheckOptions
{
    Predicate = check => check.Tags.Contains("ready"),
    ResponseWriter = UIResponseWriter.WriteHealthCheckUIResponse
});
```

- `/health/live` — lightweight, is the process alive?
- `/health/ready` — thorough, checks DB, cache, external deps.

Dockerfile healthcheck: `HEALTHCHECK --interval=30s --timeout=5s --retries=3 CMD curl -f http://localhost:8080/health/live || exit 1`

Use `wget` if curl isn't in the image. CI/CD gates deploy on `/health/ready` returning 200.

### Keycloak Health Check via Management Port

When a Keycloak container is part of the stack, explicitly enable the management port and use an HTTP health check against `/health/ready`. A plain TCP probe is not enough because the HTTP server can accept connections before initialization finishes.

**Production Keycloak**: use `start` (not `start-dev`) with `KC_HOSTNAME` and `KC_HOSTNAME_STRICT=true`. The dev mode disables production hardening and is unsafe for internet-facing identity providers.

```yaml
# Keycloak health check using curl on the management port
# This is portable across both Linux and Windows containers
keycloak:
  image: quay.io/keycloak/keycloak:latest
  command: start        # Production mode (not start-dev)
  environment:
    KC_HEALTH_ENABLED: "true"
    KC_DB: postgres
    KC_DB_URL: jdbc:postgresql://postgres:5432/keycloak
    KC_HOSTNAME: auth.example.com      # Required in production mode
    KC_HOSTNAME_STRICT: "true"         # Reject mismatched Host headers
  ports:
    - "8080:8080"
    - "9000:9000"  # Management port for health checks
  healthcheck:
    test: ["CMD", "curl", "-f", "http://localhost:9000/health/ready"]
    interval: 15s
    timeout: 5s
    start_period: 60s  # Keycloak is slow to start
    retries: 5
```

The management port (default `9000`) must be configured explicitly. Use `curl`-based HTTP checks rather than TCP checks so readiness only passes when the service is actually serving requests. In development, `command: start-dev` is acceptable for local testing but must be replaced with `command: start` + `KC_HOSTNAME` for any environment exposed to real users.

## 7. Multi-Stage Dockerfile for .NET

```dockerfile
FROM mcr.microsoft.com/dotnet/sdk:10.0 AS build
WORKDIR /src
COPY *.csproj .
RUN dotnet restore
COPY . .
RUN dotnet publish -c Release -o /app

FROM mcr.microsoft.com/dotnet/aspnet:10.0 AS runtime
WORKDIR /app
COPY --from=build /app .
RUN adduser --disabled-password --gecos "" appuser
USER appuser
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
  CMD curl -f http://localhost:8080/health/live || exit 1
ENTRYPOINT ["dotnet", "App.dll"]
```

Separate build and runtime stages. Non-root user. Healthcheck baked in. Backup container uses its own Dockerfile based on `postgres:16-alpine`.

### Per-App Dockerfile with Build Args

When multiple Blazor Server apps share common project references, use a single Dockerfile parameterized with build arguments. This avoids N nearly-identical Dockerfiles while maintaining optimal layer caching.

```dockerfile
# Dockerfile — shared by all app services
# Usage: docker build --build-arg APP_PROJECT=src/App.Accounting/App.Accounting.csproj
#                      --build-arg APP_DLL=App.Accounting.dll -t app-accounting .

ARG NET_VERSION=10.0
FROM mcr.microsoft.com/dotnet/sdk:${NET_VERSION} AS build
ARG APP_PROJECT
ARG APP_DLL

WORKDIR /src

# Layer 1: Copy shared project files first for Docker layer caching.
# When shared code changes, only this layer and below rebuild.
# When only the app project changes, the shared layers are cached.
COPY ["src/Domain/Domain.csproj", "src/Domain/"]
COPY ["src/Infrastructure/Infrastructure.csproj", "src/Infrastructure/"]
COPY ["src/Shared/Shared.csproj", "src/Shared/"]
COPY ["src/Contracts/Contracts.csproj", "src/Contracts/"]
COPY ["src/Hosting/Hosting.csproj", "src/Hosting/"]

# Layer 2: Copy the specific app project
COPY ["${APP_PROJECT}", "src/App/"]

# Layer 3: Restore — hits cache unless .csproj files changed
WORKDIR /src/src/App
RUN dotnet restore

# Layer 4: Copy all remaining source and publish
COPY . .
RUN dotnet publish -c Release -o /app --no-restore

# Runtime stage
FROM mcr.microsoft.com/dotnet/aspnet:${NET_VERSION} AS runtime
ARG APP_DLL
WORKDIR /app
COPY --from=build /app .

RUN adduser --disabled-password --gecos "" appuser
USER appuser

EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
  CMD curl -f http://localhost:8080/health/live || exit 1

ENTRYPOINT ["dotnet", "${APP_DLL}"]
```

**Docker Compose build configuration:**

```yaml
services:
  app-accounting:
    build:
      context: .
      dockerfile: Dockerfile
      args:
        APP_PROJECT: src/App.Accounting/App.Accounting.csproj
        APP_DLL: App.Accounting.dll
```

**Layer caching strategy:**

1. **Shared .csproj files** — copied and restored first. Changes here invalidate the cache for all downstream apps.
2. **App-specific .csproj** — copied next. Only app-level dependency changes invalidate this layer.
3. **All source code** — copied last. Routine code changes only invalidate the `publish` layer, not the restore layer.

This ordering minimizes rebuild time: shared project dependency changes trigger full rebuilds across all apps, but a change in a single app's code only rebuilds that app's publish step.

## 8. CI/CD Integration

Pipeline: `build → test → publish image → migrate DB → deploy → health check → rollback`

Tag images with commit SHA and environment: `registry.example.com/app:abc1234-prod`

Deploy via SSH:

```bash
ssh deploy@vps "docker compose -p prod \
  -f /opt/app/docker-compose.yml \
  -f /opt/app/docker/docker-compose.prod.yml \
  up -d --no-build"
```

Health gate after deploy:

```bash
for i in $(seq 1 30); do
  if curl -sf https://app.example.com/health/ready; then exit 0; fi
  sleep 10
done
exit 1
```

Rollback: keep the previous image tag, redeploy with it on failure.

Branch mapping: `develop`→dev, `demo`→demo, `main`→stage, prod=manual gate on main.

## 9. PostgreSQL Configuration for Multi-Tenant Apps

Schema-per-tenant multi-tenancy requires careful PostgreSQL tuning: each tenant gets its own schema, and the application sets `search_path` per connection to isolate data.

### Custom postgresql.conf

Mount a tuned config file instead of relying on defaults:

```ini
# postgres-conf/postgresql.conf

# Connection handling
max_connections = 200
superuser_reserved_connections = 3

# Memory (tune for your server RAM)
shared_buffers = 256MB
effective_cache_size = 768MB
work_mem = 4MB
maintenance_work_mem = 64MB

# Write-Ahead Logging
wal_level = replica
max_wal_size = 1GB
min_wal_size = 80MB
checkpoint_completion_target = 0.9

# Query performance
random_page_cost = 1.1          # SSD-optimized; use 4.0 for HDD
effective_io_concurrency = 200  # SSD-optimized
default_statistics_target = 100

# Logging
log_min_duration_statement = 500  # log queries slower than 500ms
log_checkpoints = on
log_connections = on
log_disconnections = on
log_lock_waits = on

# Query statistics via extension
shared_preload_libraries = 'pg_stat_statements'
```

### pg_stat_statements

Enable the extension for query performance monitoring. Add to your initialization SQL:

```sql
-- Run once per database
CREATE EXTENSION IF NOT EXISTS pg_stat_statements;
```

Top slow queries:

```sql
SELECT query, calls, mean_exec_time, total_exec_time
FROM pg_stat_statements
ORDER BY total_exec_time DESC
LIMIT 20;
```

### Docker Compose Mount

```yaml
services:
  postgres:
    image: postgres:16-alpine
    volumes:
      - postgres-data:/var/lib/postgresql/data
      - ./postgres-conf/postgresql.conf:/etc/postgresql/postgresql.conf:ro
      - ./postgres-conf/init.sql:/docker-entrypoint-initdb.d/01-init.sql:ro
    command: postgres -c config_file=/etc/postgresql/postgresql.conf
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ${POSTGRES_USER:-appuser}"]
      interval: 10s
      timeout: 5s
      retries: 5
```

### Schema-Per-Tenant Connection Pattern

The application sets `search_path` on each connection to route queries to the correct tenant schema. EF Core example:

```csharp
// Called after opening/leasing each connection
await context.Database.ExecuteSqlRawAsync(
    "SET search_path TO {0}, public", tenantSchema);
```

Tenant metadata lives in `public.tenant_info`; each tenant's tables live in `tenant_{id}`. Migrations run per-tenant schema at provisioning time.

### Connection Pooling

For multi-app deployments with many Blazor Server instances hitting one Postgres, connection pooling is essential. Use Npgsql pooling (built-in) with PgBouncer for external pooling:

```yaml
services:
  pgbouncer:
    image: edoburu/pgbouncer:latest
    environment:
      DATABASE_URL: postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@postgres:5432/${POSTGRES_DB}
      POOL_MODE: transaction
      MAX_CLIENT_CONN: 500
      DEFAULT_POOL_SIZE: 25
      RESERVE_POOL_SIZE: 10
    depends_on:
      postgres:
        condition: service_healthy
    networks:
      - infra
```

Apps connect to PgBouncer instead of Postgres directly. `POOL_MODE: transaction` releases connections between transactions, critical for Blazor Server's long-lived circuits.

## 10. Observability (Optional)

OTEL collector, Prometheus, Grafana, Loki, and Tempo live in base compose (shared across environments).

```yaml
services:
  otel-collector:
    image: otel/opentelemetry-collector-contrib:latest
    volumes:
      - ./otel/otel-collector-config.yaml:/etc/otelcol/config.yaml
    networks:
      - infra
  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    volumes:
      - ./grafana/provisioning:/etc/grafana/provisioning
      - ./grafana/dashboards:/var/lib/grafana/dashboards
    environment:
      GF_DASHBOARDS_DEFAULT_HOME_DASHBOARD_PATH: /var/lib/grafana/dashboards/overview.json
    networks:
      - infra
```

The .NET app ships traces, metrics, and logs to the collector via OTLP. Grafana queries Loki (logs), Tempo (traces), and Prometheus/Mimir (metrics).

### Docker Healthchecks for Observability Services

Include health checks for Tempo and Mimir so `depends_on` conditions work reliably in the observability stack:

```yaml
services:
  tempo:
    image: grafana/tempo:latest
    healthcheck:
      test: ["CMD", "wget", "-qO-", "http://localhost:3200/ready"]
      interval: 15s
      timeout: 5s
      retries: 5
  mimir:
    image: grafana/mimir:latest
    healthcheck:
      test: ["CMD", "wget", "-qO-", "http://localhost:9009/ready"]
      interval: 15s
      timeout: 5s
      retries: 5
```

Use `wget` over `curl` since `wget` is available in the base Grafana images. These endpoints return HTTP 200 when the services have completed their initialization (ingesters joined the ring, blocks loaded, etc.).

```yaml
# grafana/provisioning/dashboards/dashboard.yaml
apiVersion: 1
providers:
  - name: "default"
    orgId: 1
    folder: ""
    type: file
    disableDeletion: true
    editable: false  # Prevent manual edits — enforce config-as-code
    updateIntervalSeconds: 30
    options:
      path: /var/lib/grafana/dashboards
      foldersFromFilesStructure: true
```

The `editable: false` setting prevents manual dashboard changes that are not tracked in version control. All dashboard updates must be made in the JSON files in the repository.

## 11. Anti-Patterns

- **Never use `latest` for production images.** Pin to a specific SHA or version.
- **Never run Keycloak in `start-dev` mode in production.** Use production-grade settings.
- **Never commit secrets to compose files.** Use env vars, secrets managers, or Docker secrets.
- **Never skip health check gates in CI/CD.** A deploy without verification isn't a deploy.
- **Never use `docker-compose` v1 syntax.** Target the Compose Specification.
- **Never expose database ports to the internet.** Bind to `127.0.0.1` or use an internal network.
- **Never skip backup verification.** An unverified backup is not a backup.
- **Never run containers as root in production.** Always add a non-root user.
- **Never hardcode env-specific values in the base compose.** That's what overlays are for.
- **Never run nginx and Caddy simultaneously.** Pick one reverse proxy per deployment. Mixing them adds complexity with no benefit.
- **Never expose per-app localhost ports in production.** Use a reverse proxy (nginx or Caddy) with path-based routing instead of publishing dozens of host ports.
- **Never copy the entire source tree before restore.** Copy `.csproj` files first, restore, then copy source. This preserves Docker layer caching for the expensive restore step.
- **Never use TCP checks for application health endpoints.** An HTTP server can accept TCP connections before it finishes initializing. Always use HTTP-based health checks (`curl -f http://localhost:8080/health/ready`) that verify the application is actually serving requests, not just that the port is open.
- **Never make Grafana dashboards editable in production when using config-as-code.** Manual edits in the Grafana UI will be lost on container restart if dashboards are provisioned from files. Set `editable: false` in the provisioning config to prevent this.

## Related Skills

- **aspnetcore-health** — Health check endpoints that Docker HEALTHCHECK probes hit, and CI/CD health gates
- **keycloak-oidc** — Keycloak service in docker-compose, health check, and production hardening
- **dotnet-clean-arch** — Multi-stage Dockerfile for .NET apps, and migration management in CI/CD
- **quartz-scheduling** — Running Quartz jobs in containers alongside the app
- **blazor-mudblazor** — Blazor Server app patterns, SignalR WebSocket proxy configuration, shared component libraries
- **lgpd-data-privacy** — Schema-per-tenant data isolation and field-level encryption patterns for multi-tenant deployments
