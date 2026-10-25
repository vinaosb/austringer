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

`docker-compose.override.yml` auto-applied by Docker Compose without `-f` flag. Perfect for dev-specific resource limits and debug port mappings:

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
        limits: { cpus: '2', memory: 4G }
  postgres:
    ports:
      - "5432:5432"      # direct DB access for migrations/tools
```

Run with `docker compose up -d` — override auto-applied. Override files environment-specific, not for production config.

### Per-Environment Overlays

Each overlay (`docker/docker-compose.{dev,demo,stage,prod}.yml`) adds runtime services, overrides base env vars. Never duplicate base services.

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
      postgres: { condition: service_healthy }
    networks:
      - infra
  nginx:
    image: nginx:alpine
    ports: ["443:443"]
    volumes:
      - ./nginx/prod.conf:/etc/nginx/conf.d/default.conf:ro
      - certbot-conf:/etc/letsencrypt
    depends_on: [app]
    networks:
      - infra
```

**Usage:**
```bash
docker compose -f docker-compose.yml -f docker/docker-compose.prod.yml up -d
```

### Multi-App Compose Pattern

For suites of related applications sharing infrastructure. Single Dockerfile with build args produces N images from one template.

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
      interval: 10s; timeout: 5s; retries: 5
    networks: [infra]

  redis:
    image: docker.dragonflydb.io/dragonfly/dragonfly:latest
    volumes: [redis-data:/data]
    networks: [infra]

  keycloak:
    image: quay.io/keycloak/keycloak:26.0
    command: start --import-realm
    environment:
      KC_DB: postgres; KC_DB_URL_HOST: postgres
      KC_DB_USERNAME: ${POSTGRES_USER:-appuser}
      KC_DB_PASSWORD: ${POSTGRES_PASSWORD:-changeme}
      KEYCLOAK_ADMIN: ${KEYCLOAK_ADMIN:-admin}
      KEYCLOAK_ADMIN_PASSWORD: ${KEYCLOAK_ADMIN_PASSWORD:-changeme}
    volumes: [./keycloak/realm-export.json:/opt/keycloak/data/import/realm.json:ro]
    depends_on: { postgres: { condition: service_healthy } }
    networks: [infra]

  # ── Applications (one per module, same Dockerfile via build args) ──
  app-portal:
    build:
      context: .
      dockerfile: Dockerfile
      args: { APP_PROJECT: src/App.Portal/App.Portal.csproj, APP_DLL: App.Portal.dll }
    env_file: .env.apps
    environment: { ASPNETCORE_URLS: http://+:8080 }
    ports: ["5101:8080"]
    depends_on: { postgres: { condition: service_healthy }, redis: { condition: service_started }, keycloak: { condition: service_started } }
    networks: [infra]

  app-accounting:
    build:
      context: .
      dockerfile: Dockerfile
      args: { APP_PROJECT: src/App.Accounting/App.Accounting.csproj, APP_DLL: App.Accounting.dll }
    env_file: .env.apps
    environment: { ASPNETCORE_URLS: http://+:8080 }
    ports: ["5102:8080"]
    depends_on: { postgres: { condition: service_healthy }, redis: { condition: service_started } }
    networks: [infra]

volumes: { postgres-data:, redis-data: }
networks: { infra: { driver: bridge } }
```

**Shared env file** (`.env.apps`):
```bash
# .env.apps — shared by all app services
ConnectionStrings__Database=Host=postgres;Database=appdb;Username=appuser;Password=changeme
ConnectionStrings__Redis=redis:6379
Keycloak__Authority=http://keycloak:8080/realms/master
Keycloak__ClientId=app-suite
```

**Key patterns:**
- **One Dockerfile, N apps** — `ARG APP_PROJECT` / `ARG APP_DLL` avoids duplicating Dockerfiles.
- **`env_file` sharing** — connection strings, Redis URL, OIDC in one file.
- **Per-app port mapping** — dev overlay exposes per-port; prod uses reverse proxy.
- **`pg_isready` healthcheck** — apps wait for Postgres.
- **`--import-realm`** — Keycloak bootstraps realm from mounted JSON export.

## 2. Environment Variable Validation

Escalate strictness from dev to prod. Never commit real secrets.

| Environment | Pattern | Behavior |
|---|---|---|
| dev/demo | `${VAR:-default}` | Uses default if missing |
| stage | Mix | Critical required, others default |
| prod | `${VAR:?error}` | Fails immediately if missing |

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

Base `appsettings.json` uses empty strings for secrets. Real values arrive via env vars or secrets manager.

## 3. Nginx Reverse Proxy

One config per environment with env-specific upstreams, server_names, TLS.

```nginx
# nginx/prod.conf
upstream app { server app:8080; }
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

Blazor Server + SignalR require WebSocket upgrade headers, long read timeouts, disabled buffering.

### ASP.NET Core Forwarded Headers

**1. `KnownNetworks` obsolete in .NET 10 (ASPDEPR005):**
```csharp
// .NET 10+ (correct)
options.KnownIPNetworks.Clear();
options.KnownIPNetworks.Add(new AspNetCore.HttpOverrides.IPNetwork(IPAddress.Loopback, 8));

// .NET 9 and earlier (obsolete in .NET 10)
options.KnownNetworks.Clear();  // ASPDEPR005
```

**2. `UseForwardedHeaders()` ordering:**
```csharp
app.UseForwardedHeaders();     // 1st
app.UseHsts();                 // 2nd
app.UseHttpsRedirection();     // 3rd
app.UseAuthentication();       // 4th
```

Wrong ordering causes redirect loops, broken auth, incorrect scheme detection.

### Caddy Reverse Proxy

Alternative to nginx with automatic HTTPS via Let's Encrypt — no certbot sidecar. Well-suited for multi-app deployments behind path-based routing.

```Caddyfile
# Caddyfile
{$DOMAIN:localhost} {
    # Automatic HTTPS — Caddy provisions/renews certs via ACME

    handle_path /auth/* { reverse_proxy keycloak:8080 }
    handle_path /portal/* { reverse_proxy app-portal:8080 }
    handle_path /accounting/* { reverse_proxy app-accounting:8080 }

    handle { redir /portal/ permanent }
    handle /admin* { respond "Forbidden" 403 }
}
```

**Docker Compose:**
```yaml
services:
  caddy:
    image: caddy:2-alpine
    ports: ["80:80", "443:443"]
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile:ro
      - caddy-data:/data
      - caddy-config:/config
    depends_on: [app-portal, app-accounting, keycloak]
    networks: [infra]
volumes: { caddy-data:, caddy-config: }
```

**Key advantages:**
- **Automatic HTTPS** — provisions/renews Let's Encrypt without certbot.
- **WebSocket** — enabled by default, no manual upgrade headers.
- **Simpler config** — one file, no upstream blocks, no separate SSL directives.
- **`handle_path`** strips matched prefix before forwarding.

**When to choose Caddy over nginx:** Caddy for automatic TLS with minimal config. Nginx for advanced features (rate limiting, complex rewrites). Do NOT use both simultaneously.

## 4. TLS/SSL

### Dev and Demo (Self-Signed)

Generate at image build time or via entrypoint:
```bash
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
  -keyout /etc/nginx/certs/server.key \
  -out /etc/nginx/certs/server.crt \
  -subj "/CN=localhost"
```

Named volume persists certs across restarts:
```yaml
services:
  nginx:
    volumes:
      - nginx-certs:/etc/nginx/certs
volumes: { nginx-certs: }
```

Manual one-shot:
```bash
openssl req -x509 -newkey rsa:4096 -nodes \
  -keyout key.pem -out cert.pem -days 365 \
  -subj "/CN=localhost"
```

### Stage and Prod (Let's Encrypt)

Certbot sidecar with shared volumes:
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
volumes: { certbot-www:, certbot-conf: }
```

ACME challenge:
```nginx
location /.well-known/acme-challenge/ { root /var/www/certbot; }
```

Production hardening:
```nginx
ssl_protocols TLSv1.2 TLSv1.3;
ssl_ciphers HIGH:!aNULL:!MD5;
ssl_prefer_server_ciphers on;
ssl_stapling on; ssl_stapling_verify on;
gzip on; gzip_types text/plain application/json text/css application/javascript;
```

### Content Security Policy (CSP)

Move CSP from hardcoded nginx to `appsettings.json`:
```json
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

Production overrides (stricter):
```json
{
  "ContentSecurityPolicy": {
    "ScriptSrc": "'self'",
    "ConnectSrc": "'self' wss://app.example.com"
  }
}
```

Build CSP header from settings, emit via middleware. Per-environment relaxation without nginx config changes.

## 5. Encrypted Backup/Restore

### Backup
```bash
pg_dump -Fc "$DATABASE_URL" \
  | openssl enc -aes-256-cbc -salt -pbkdf2 -pass pass:"$BACKUP_KEY" \
  > "/backups/$(date +%Y%m%d_%H%M%S).enc"
find /backups -name "*.enc" -mtime +7 -type f -delete
```

### Restore
```bash
openssl enc -d -aes-256-cbc -pbkdf2 -pass pass:"$BACKUP_KEY" \
  -in "$BACKUP_FILE" \
  | pg_restore -d "$DATABASE_URL" --no-owner --no-acl
```

### Verification + Scheduling

Verify: encrypt test dump → decrypt → restore to temp DB → compare row counts → clean up.

Backup container uses cron:
```bash
#!/bin/sh
echo "$BACKUP_SCHEDULE /usr/local/bin/backup.sh >> /var/log/cron.log 2>&1" | crontab -
echo "$VERIFY_SCHEDULE /usr/local/bin/verify.sh >> /var/log/cron.log 2>&1" | crontab -
crond -f
```

Backup keys from env vars or secrets manager. Never hardcode.

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

- `/health/live` — lightweight, is process alive?
- `/health/ready` — thorough, checks DB, cache, external deps.

Docker HEALTHCHECK: `HEALTHCHECK --interval=30s --timeout=5s --retries=3 CMD curl -f http://localhost:8080/health/live || exit 1`

Use `wget` if curl not in image. CI/CD gates deploy on `/health/ready` returning 200.

### Keycloak Health Check via Management Port

```yaml
keycloak:
  image: quay.io/keycloak/keycloak:latest
  command: start        # Production mode (not start-dev)
  environment:
    KC_HEALTH_ENABLED: "true"
    KC_DB: postgres
    KC_DB_URL: jdbc:postgresql://postgres:5432/keycloak
    KC_HOSTNAME: auth.example.com
    KC_HOSTNAME_STRICT: "true"
  ports:
    - "8080:8080"
    - "9000:9000"  # Management port for health checks
  healthcheck:
    test: ["CMD", "curl", "-f", "http://localhost:9000/health/ready"]
    interval: 15s; timeout: 5s; start_period: 60s; retries: 5
```

Management port (default `9000`) must be explicit. Use HTTP checks, not TCP — TCP accepts connections before init finishes.

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

Separate build/runtime stages. Non-root user. Healthcheck baked in.

### Per-App Dockerfile with Build Args

Single parameterized Dockerfile avoids N nearly-identical files:
```dockerfile
# Dockerfile — shared by all app services
ARG NET_VERSION=10.0
FROM mcr.microsoft.com/dotnet/sdk:${NET_VERSION} AS build
ARG APP_PROJECT
ARG APP_DLL

WORKDIR /src
COPY ["src/Domain/Domain.csproj", "src/Domain/"]
COPY ["src/Infrastructure/Infrastructure.csproj", "src/Infrastructure/"]
COPY ["src/Shared/Shared.csproj", "src/Shared/"]
COPY ["src/Contracts/Contracts.csproj", "src/Contracts/"]
COPY ["src/Hosting/Hosting.csproj", "src/Hosting/"]
COPY ["${APP_PROJECT}", "src/App/"]

WORKDIR /src/src/App
RUN dotnet restore
COPY . .
RUN dotnet publish -c Release -o /app --no-restore

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

**Docker Compose:**
```yaml
services:
  app-accounting:
    build:
      context: .
      dockerfile: Dockerfile
      args: { APP_PROJECT: src/App.Accounting/App.Accounting.csproj, APP_DLL: App.Accounting.dll }
```

**Layer caching:**
1. Shared `.csproj` files first (cache shared deps)
2. App-specific `.csproj` next (only app dep changes invalidate)
3. All source last (routine code changes only invalidate publish layer)

## 8. CI/CD Integration

Pipeline: `build → test → publish image → migrate DB → deploy → health check → rollback`

Tag images with commit SHA + environment: `registry.example.com/app:abc1234-prod`

Deploy via SSH:
```bash
ssh deploy@vps "docker compose -p prod \
  -f /opt/app/docker-compose.yml \
  -f /opt/app/docker/docker-compose.prod.yml \
  up -d --no-build"
```

Health gate:
```bash
for i in $(seq 1 30); do
  if curl -sf https://app.example.com/health/ready; then exit 0; fi
  sleep 10
done
exit 1
```

Rollback: keep previous image tag, redeploy on failure.

Branch mapping: `develop`→dev, `demo`→demo, `main`→stage, prod=manual gate on main.

## 9. PostgreSQL Configuration for Multi-Tenant Apps

### Custom postgresql.conf
```ini
# postgres-conf/postgresql.conf
max_connections = 200
superuser_reserved_connections = 3
shared_buffers = 256MB
effective_cache_size = 768MB
work_mem = 4MB
maintenance_work_mem = 64MB
wal_level = replica
max_wal_size = 1GB
min_wal_size = 80MB
checkpoint_completion_target = 0.9
random_page_cost = 1.1          # SSD-optimized
effective_io_concurrency = 200
default_statistics_target = 100
log_min_duration_statement = 500
log_checkpoints = on
log_connections = on
log_disconnections = on
log_lock_waits = on
shared_preload_libraries = 'pg_stat_statements'
```

### pg_stat_statements
```sql
CREATE EXTENSION IF NOT EXISTS pg_stat_statements;

SELECT query, calls, mean_exec_time, total_exec_time
FROM pg_stat_statements ORDER BY total_exec_time DESC LIMIT 20;
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
      interval: 10s; timeout: 5s; retries: 5
```

### Schema-Per-Tenant Connection Pattern
```csharp
await context.Database.ExecuteSqlRawAsync(
    "SET search_path TO {0}, public", tenantSchema);
```

### Connection Pooling
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
    depends_on: { postgres: { condition: service_healthy } }
    networks: [infra]
```

`POOL_MODE: transaction` releases connections between transactions — critical for Blazor Server's long-lived circuits.

## 10. Observability (Optional)

OTEL collector, Prometheus, Grafana, Loki, Tempo in base compose:
```yaml
services:
  otel-collector:
    image: otel/opentelemetry-collector-contrib:latest
    volumes: [./otel/otel-collector-config.yaml:/etc/otelcol/config.yaml]
    networks: [infra]
  grafana:
    image: grafana/grafana:latest
    ports: ["3000:3000"]
    volumes:
      - ./grafana/provisioning:/etc/grafana/provisioning
      - ./grafana/dashboards:/var/lib/grafana/dashboards
    environment:
      GF_DASHBOARDS_DEFAULT_HOME_DASHBOARD_PATH: /var/lib/grafana/dashboards/overview.json
    networks: [infra]
```

App ships traces/metrics/logs to collector via OTLP. Grafana queries Loki (logs), Tempo (traces), Prometheus/Mimir (metrics).

### Docker Healthchecks for Observability
```yaml
services:
  tempo:
    image: grafana/tempo:latest
    healthcheck:
      test: ["CMD", "wget", "-qO-", "http://localhost:3200/ready"]
      interval: 15s; timeout: 5s; retries: 5
  mimir:
    image: grafana/mimir:latest
    healthcheck:
      test: ["CMD", "wget", "-qO-", "http://localhost:9009/ready"]
      interval: 15s; timeout: 5s; retries: 5
```

Use `wget` — available in base Grafana images.

```yaml
# grafana/provisioning/dashboards/dashboard.yaml
apiVersion: 1
providers:
  - name: "default"
    orgId: 1
    folder: ""
    type: file
    disableDeletion: true
    editable: false  # Config-as-code enforcement
    updateIntervalSeconds: 30
    options:
      path: /var/lib/grafana/dashboards
      foldersFromFilesStructure: true
```

## 11. Anti-Patterns

- **Never use `latest` for production images.** Pin to specific SHA or version.
- **Never run Keycloak in `start-dev` mode in production.**
- **Never commit secrets to compose files.** Use env vars, secrets managers.
- **Never skip health check gates in CI/CD.**
- **Never use `docker-compose` v1 syntax.** Target Compose Specification.
- **Never expose database ports to internet.** Bind to `127.0.0.1` or internal network.
- **Never skip backup verification.** Unverified backup is not a backup.
- **Never run containers as root in production.**
- **Never hardcode env-specific values in base compose.** Use overlays.
- **Never run nginx and Caddy simultaneously.** Pick one reverse proxy.
- **Never expose per-app localhost ports in production.** Use reverse proxy with path-based routing.
- **Never copy entire source tree before restore.** Copy `.csproj` first, restore, then source. Preserves layer caching.
- **Never use TCP checks for app health endpoints.** HTTP server accepts TCP before init finishes. Always use HTTP health checks.
- **Never make Grafana dashboards editable in production with config-as-code.** Manual edits lost on restart. Set `editable: false`.

## Related Skills

- **aspnetcore-health** — Health check endpoints Docker HEALTHCHECK probes, CI/CD health gates
- **keycloak-oidc** — Keycloak in docker-compose, health check, production hardening
- **dotnet-clean-arch** — Multi-stage Dockerfile, migration management in CI/CD
- **quartz-scheduling** — Running Quartz jobs in containers
- **blazor-mudblazor** — Blazor Server patterns, SignalR WebSocket proxy config
- **lgpd-data-privacy** — Schema-per-tenant isolation, field-level encryption