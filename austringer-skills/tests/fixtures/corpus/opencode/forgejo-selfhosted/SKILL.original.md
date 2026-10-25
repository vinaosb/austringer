---
name: forgejo-selfhosted
description: "Deploy and manage a self-hosted Forgejo instance with Actions runner via Docker Compose. Covers server deployment, runner registration, nginx proxying, API usage, and migration from GitLab. USE FOR: Forgejo setup, Forgejo Actions CI/CD, self-hosted git server, git server migration. DO NOT USE FOR: GitHub Actions, GitLab CI, non-Forgejo git hosting."
---

# Forgejo Self-Hosted Deployment Skill

## Quick Reference

| Component | Image | Port |
|-----------|-------|------|
| Forgejo Server | `codeberg.org/forgejo/forgejo:9` | 3000 (HTTP), 2222 (SSH) |
| Forgejo Runner | `data.forgejo.org/forgejo/runner:12` | N/A (connects to dind) |
| Docker-in-Docker | `docker:dind` | 2375 (internal TCP) |

## Architecture

```
Internet → [nginx:443/8443/8444] → Forgejo (direct on :3000)
                                   Mattermost (:8443 via nginx)
                                   Vaultwarden (:8444 via nginx)
                                         ↓
                              forgejo-runner ←→ docker-in-docker
                              (spawns build containers)
```

All services share a Docker network (`fo-network`).

## Docker Compose Template

```yaml
services:
  forgejo:
    image: codeberg.org/forgejo/forgejo:9
    container_name: forgejo
    restart: unless-stopped
    environment:
      - USER_UID=1000
      - USER_GID=1000
      - FORGEJO__server__ROOT_URL=http://YOUR_IP:3000/
      - FORGEJO__server__SSH_DOMAIN=YOUR_IP
      - FORGEJO__server__SSH_PORT=2222
      - FORGEJO__server__HTTP_PORT=3000
      - FORGEJO__server__LFS_START_SERVER=true
      - FORGEJO__actions__ENABLED=true
      - FORGEJO__actions__DEFAULT_ACTIONS_URL=https://codeberg.org
      - FORGEJO__repository__ENABLE_PUSH_CREATE_USER=true
      - FORGEJO__security__INSTALL_LOCK=true
    volumes:
      - forgejo-data:/data
      - /etc/timezone:/etc/timezone:ro
      - /etc/localtime:/etc/localtime:ro
    ports:
      - "3000:3000"
      - "2222:22"
    networks:
      - fo-network
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/"]
      interval: 10s
      timeout: 5s
      retries: 10

  docker-in-docker:
    image: docker:dind
    container_name: forgejo-dind
    privileged: true
    command: ["dockerd", "-H", "tcp://0.0.0.0:2375", "--tls=false"]
    restart: unless-stopped
    networks:
      - fo-network

  runner:
    image: data.forgejo.org/forgejo/runner:12
    container_name: forgejo-runner
    depends_on:
      - docker-in-docker
    environment:
      DOCKER_HOST: tcp://docker-in-docker:2375
    user: 1001:1001
    volumes:
      - runner-data:/data
    restart: unless-stopped
    networks:
      - fo-network
    command: forgejo-runner daemon

volumes:
  forgejo-data:
  runner-data:

networks:
  fo-network:
    external: true
```

## Setup Steps

### 1. Deploy Server

```bash
mkdir -p /opt/forgejo
# Write docker-compose.yml (only server + dind sections first)
cd /opt/forgejo && docker compose up -d
# Wait for healthy
```

### 2. Open Firewall

```bash
ufw allow 3000/tcp   # Forgejo web UI
ufw allow 2222/tcp   # Forgejo SSH (git clone)
```

### 3. Create Admin User

```bash
# MUST run as git user (-u git), Forgejo refuses root
docker exec -u git forgejo forgejo admin user create \
  --admin \
  --username USERNAME \
  --password 'PASSWORD' \
  --email admin@example.com
```

### 4. Create Org & Repo via API

```bash
# Auth: Basic base64(user:pass)
# Create org
curl -X POST http://IP:3000/api/v1/orgs \
  -H "Authorization: Basic TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"username":"org-name","visibility":"private"}'

# Create repo under org
curl -X POST http://IP:3000/api/v1/orgs/org-name/repos \
  -H "Authorization: Basic TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"repo-name","private":true,"auto_init":false}'
```

### 5. Register Runner

```bash
# Get registration token (org-level)
curl http://IP:3000/api/v1/orgs/ORG/actions/runners/registration-token \
  -H "Authorization: Basic TOKEN"

# Register (deprecated in v12 but still works)
docker run --rm \
  -v forgejo_runner-data:/data \
  --network fo-network \
  data.forgejo.org/forgejo/runner:12 \
  forgejo-runner register \
    --instance http://forgejo:3000 \
    --token REGISTRATION_TOKEN \
    --name runner-name \
    --labels 'ubuntu-latest:docker://node:20-bookworm,docker:docker://node:20-bookworm' \
    --no-interactive

# Then add runner service to docker-compose.yml and: docker compose up -d
```

### 6. Git Remote Setup

```bash
# Set Forgejo as origin, GitLab as backup mirror
git remote rename origin gitlab
# URL-encode special chars in password: @ → %40, ! → %21
git remote add origin http://USER:ENCODED_PASS@IP:3000/ORG/REPO.git
git push -u origin --all
git push origin --tags
```

## Nginx Proxy (if behind reverse proxy)

```nginx
upstream forgejo {
    server forgejo:3000;
}

server {
    listen PORT ssl;
    http2 on;
    server_name _;

    ssl_certificate     /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;
    ssl_protocols       TLSv1.2 TLSv1.3;
    ssl_ciphers         HIGH:!aNULL:!MD5;

    client_max_body_size 100m;

    location / {
        proxy_pass         http://forgejo;
        proxy_http_version 1.1;
        proxy_set_header Upgrade    $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host              $host;
        proxy_set_header X-Real-IP         $remote_addr;
        proxy_set_header X-Forwarded-For   $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Forwarded-Host  $host;
    }
}
```

## Gotchas & Lessons Learned

1. **pscp adds UTF-8 BOM** — use base64 encode/decode instead to transfer config files:
   ```powershell
   $b64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($content))
   # Send b64 string to VPS, then: echo $b64 | base64 -d > file
   ```

2. **Forgejo CLI refuses root** — always use `docker exec -u git forgejo ...`

3. **Runner image is at `data.forgejo.org`** — NOT `codeberg.org`. Runner v12 is current.

4. **`forgejo-runner register` deprecated in v12** — still works but warns. Future: use config file with connection declarations.

5. **Runner user must be non-root** — `user: 1001:1001` in compose. Volume permissions need `chown 1001:1001`.

6. **URL-encode passwords** in git remote URLs: `@` → `%40`, `!` → `%21`

7. **Docker-in-Docker needs privileged mode** — `privileged: true` for the dind container.

8. **Registration token is single-use per runner** — get a new token for each runner from the API.

9. **UFW must allow both ports** — 3000 (web) and 2222 (SSH git operations).

10. **Nginx `listen 443 ssl http2` deprecated** in newer nginx — use `listen 443 ssl; http2 on;` instead.

## Useful API Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/v1/orgs` | POST | Create organization |
| `/api/v1/orgs/{org}/repos` | POST | Create repo under org |
| `/api/v1/orgs/{org}/actions/runners/registration-token` | GET | Get runner token |
| `/api/v1/repos/{owner}/{repo}` | GET | Repo info |
| `/api/v1/admin/users` | POST | Create user (admin only) |

## Forgejo Actions Workflow Syntax

Same as GitHub Actions (`.gitea/workflows/` or `.forgejo/workflows/`):

```yaml
name: Build
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: echo "Hello from Forgejo Actions!"
```

Default actions URL: `https://codeberg.org` (set via `FORGEJO__actions__DEFAULT_ACTIONS_URL`).
