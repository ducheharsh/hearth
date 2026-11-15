# Catalog: apps and packs

The curated catalog is plain YAML in `catalog/`. hearthd loads it at start; a manifest that doesn't
validate stops startup, and a pack that doesn't validate is skipped with a warning (a test catches it
in CI first).

```
catalog/
├── <app-id>.yml        one file per one-click app
├── icons/<app-id>.svg  the app's real logo
└── packs/<pack-id>.yml one file per pack
```

## App manifest

```yaml
id: uptime-kuma                 # 1-48 chars [a-z0-9-]; also the compose project name (hearth-<id>)
name: Uptime Kuma
description: Know the moment anything goes down, with beautiful status pages.
category: monitoring            # ai | automation | media | monitoring | network | productivity | security | storage
website: https://uptime.kuma.pet
repo: louislam/uptime-kuma      # GitHub owner/name: stars and commit activity in the Store
docker_hub: louislam/uptime-kuma  # optional: pull counts
featured: true                  # optional: Must-Have Essentials row
editors_note: "…"               # optional: one line for featured placements
port: 3002                      # published as https://<host>:3002 (>= 1024, unique, not 9000/9091)
upstream: { service: uptime-kuma, port: 3001 }   # compose service + its container port
resources:
  ram_mb: 150                   # typical steady-state RAM (measured p95 replaces it after 1 h)
  needs_vault: false            # true if compose uses ${VAULT}
  vault_gb: 0                   # expected SSD growth
auth:
  protect: true                 # default; false only for apps that must be reachable without a login
  exempt_paths: ["/api/push/*"] # paths that skip Authelia (the app's own auth must cover them)
compose: |
  services:
    uptime-kuma:
      image: louislam/uptime-kuma:2
      restart: unless-stopped
      volumes:
        - ${APP_DATA}:/app/data
```

### Variables

| Variable | Value |
|---|---|
| `${APP_DATA}` | `<state>/apps/<id>/data`: the app's private data (backed up) |
| `${APP_SECRET}` | Random per-app secret, stable across reinstalls (DB passwords, signing keys) |
| `${VAULT}`, `${MEDIA}`, `${BACKUPS}` | The storage role folders (external SSD) |
| `${TS_HOST}` | The Mac's Tailscale name, for apps that need their public URL |
| `${TZ}` | Time zone from Settings |

hearth adds an override that joins every service to the shared `hearth` network, so the gateway can
reach `upstream.service` by name. Don't publish ports in compose: the gateway publishes `port`.

### Rules for catalog apps

- **Logo**: the project's real logo at `icons/<id>.svg` (we use [selfh.st/icons](https://selfh.st/icons)).
  Check it contains no `<script>`.
- **Image**: a maintained, official image. Prefer a major-version tag (`:2`, `:v2`) over `latest` when
  the project publishes one. Don't add archived projects (we skip upstream File Browser for this
  reason and ship the maintained Quantum fork).
- **Auth**: keep `protect: true`. Use `exempt_paths` only for clients that can't do a browser login
  (mobile/desktop apps, Subsonic, push endpoints) and only where the app authenticates them itself.
  If the app has its own login *and* sits behind Authelia, prefer turning its login off (like File
  Browser) so people sign in once.
- **RAM**: measure it (`docker stats` after a few minutes of use) rather than guessing; it decides
  whether the install is allowed on an 8 GB machine.
- **Port**: pick an unused one (`grep -h '^port:' catalog/*.yml`).
- **Verify**: install it on a test daemon, open it through the gateway, remove it.

## Packs

```yaml
id: ai
name: Local AI
tagline: A private ChatGPT that runs on this Mac      # card subtitle
description: >-                                        # dialog intro
  Everything for local AI in one go…
icon: brain           # brain | users-three | film-strip | shield-check | code | hard-drives | folder-simple
apps:
  - { id: open-webui, role: "Chat app with document Q&A and voice" }
  - { id: n8n, role: "AI agents and automations", optional: true }
ollama:                # optional: native Ollama + models
  models:
    - { name: "llama3.2:3b", role: "Everyday chat model", size_mb: 2000, ram_mb: 3000 }
    - { name: "gemma3:4b", role: "Understands images", size_mb: 3300, ram_mb: 4500, optional: true }
next_steps:            # shown when the install finishes
  - Open Open WebUI and create your account.
```

- **Always quote `role`** (and anything else with a comma) inside `{ … }`: a bare comma ends the field.
- Every `apps[].id` must exist in the catalog. `cargo test -p hearthd packs` loads every shipped pack.
- Optional parts are off by default; the user toggles them in the dialog.
- Capacity counts the sum of new apps' RAM plus the **largest** new model (Ollama loads one at a time).
- Adding an icon name means adding it to `ICONS` in `web/apps/dashboard/src/features/packs/packs.tsx`.

### How a pack installs

`POST /api/packs/{id}/install` plans against the machine, then runs steps in order:

1. **Ollama** (if the pack has models): `brew install ollama` if missing, `brew services start ollama`.
2. **Models**: `POST /api/pull` to Ollama with streamed progress.
3. **Apps**: each one exactly like a Store install (same compose writer, gateway apply and event).

A failed step is recorded and the rest continue; reopening the pack shows what's still missing.
