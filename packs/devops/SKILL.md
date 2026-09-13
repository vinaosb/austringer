---
name: devops
description: "Deployment and infrastructure guidance: Docker Compose multi-environment overlays, nginx TLS termination and reverse proxying, CI/CD pipelines, infrastructure as code, self-hosted git, and backup/restore drills."
license: MIT OR Apache-2.0
version: "1.0.0"
languages: ["YAML", "Shell", "HCL", "Dockerfile"]
allowed-tools: ["view_window", "edit_lines", "execute_command", "tool_search"]
triggers:
  - docker compose
  - dockerfile
  - nginx reverse proxy
  - tls certificate
  - let's encrypt
  - kubernetes manifest
  - helm chart
  - ci/cd pipeline
  - github actions
  - terraform
  - infrastructure as code
  - self-hosted git server
  - backup and restore
  - deployment pipeline
  - déploiement docker
  - pipeline de implantação
  - 部署 docker
do-not-trigger-on:
  - react component
  - entity framework
  - ef core
  - lease agreement
  - trial balance
  - electrical panel
  - financial model
  - derivative pricing
---

# Devops pack

{{shared:pack-conventions}}

## Workflow

1. **Environment parity**: one base compose file + per-environment
   overlays (`compose.override.yml`, `compose.prod.yml`); secrets via
   environment files never committed; the same images run everywhere,
   only config differs.
2. **TLS**: terminate TLS at nginx with an explicit cipher/protocol
   policy; self-signed certs for internal hops, Let's Encrypt (ACME
   webroot or DNS-01) for public names; renewal on a timer with a
   reload hook, and an expiry monitor.
3. **Reverse proxy**: upstream health checks before traffic; timeouts
   explicit (connect/read/send); WebSocket/SSE upgrade blocks declared;
   request-size limits set.
4. **CI/CD**: pipeline stages lint → build → test → package → deploy;
   artifacts are content-addressed and promoted between stages —
   rebuilt binaries never ship; deploy jobs are replayable and
   auditable.
5. **IaC**: Terraform state remote + locked; changes via plan review;
   modules versioned; `drift detect` on a schedule.
6. **Backup/restore**: backups are proven by RESTORE DRILLS, not by
   backup job logs — encrypted at rest, off-site copy, restore rehearsed
   quarterly with a timed RTO.
7. **Self-hosted services** (git, runners): updates are deliberate
   (pin versions, read release notes), data dirs are on the backup
   schedule, and auth is federated rather than local-only.

## Worked example

Request: *"Add HTTPS to our compose stack."*

```yaml
# compose.prod.yml (overlay)
services:
  proxy:
    image: nginx:1.27
    ports: ["80:80", "443:443"]
    volumes:
      - ./nginx/conf.d:/etc/nginx/conf.d:ro
      - certs:/etc/nginx/certs:ro
    depends_passing_healthcheck: api  # fail closed
volumes: { certs: }
```

```nginx
server {
  listen 443 ssl;
  ssl_certificate     /etc/nginx/certs/fullchain.pem;
  ssl_certificate_key /etc/nginx/certs/privkey.pem;
  location / { proxy_pass http://api:8080; proxy_set_header Host $host; }
}
```

Plus: certbot renewal container with `deploy-hook: nginx -s reload`,
port 80 redirecting to 443, and an expiry check in monitoring.

## Eval scenario

Prompt: `"Set up Docker Compose deployment with nginx reverse proxy and
TLS"`. Expected routing: `devops` (triggers: `docker compose`, `nginx
reverse proxy`). First actions: inspect the existing stack layout and
domain names, then propose base+overlay structure, TLS plan (internal
vs public), and a restore/renewal drill before touching files.
