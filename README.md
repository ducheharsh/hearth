# hearth

A small, native control plane for a self-hosted machine: an app **marketplace** with
**capacity-aware** installs, **one login (Authelia + 2FA) in front of everything**, and
**Tailscale** connectivity. Built for an 8 GB M1 Mac running OrbStack; Linux later.

## What it does

- **App Store**: editor's picks, *Top 5 this month*, *Recommended for this Mac*, leaderboards, your own
  ratings, real app logos. One-click install for curated apps; **Discover** browses all 1,360 apps from
  [awesome-selfhosted](https://awesome-selfhosted.net) with search, tags and live charts.
- **Install your way**: every Docker setting (image tag, env, ports, volumes, restart, CPU/RAM limits, flags,
  or raw compose) is an editable form pre-filled with the recommended defaults.
- **Bring your own**: add any image/compose as an app, or publish a container you already run.
- **Capacity-aware**: installs are checked against live RAM/disk before they happen.
- **One login for everything**: Caddy + Authelia (2FA/passkeys) in front of every app, published over
  Tailscale HTTPS. Per-app port, subdomain and public toggle.
- **Your domain**: Cloudflare Tunnel set up automatically from an API token (`app.yourdomain.com`), plus
  Tailscale Funnel and ngrok for single-app public links.
- **Storage that knows its drives**: attach SSDs to roles (vault/media/backups); the same drive is recognised
  after renames/remounts, impostors are refused, apps pause on unplug and resume on return, safe eject.
- **Containers**: live stats, details, start/stop/restart, streaming logs and an in-browser terminal.
- **⌘K everywhere**: one search box for apps, 1,300+ directory entries, containers, drives and settings
  (Tantivy, in-process: BM25 ranking, typo tolerance, ~1 ms; no search cluster to run).
- **Networks**: Tailscale, NetBird or your own Headscale, managed from Settings.
- **Pets**: Ember and Miso react to load, installs and drive events, and talk about it.

## Quick start

```bash
# Rust via Homebrew: brew install rustup && rustup default stable
pnpm -C web install && pnpm -C web build          # React dashboard → embedded in the binary
cargo run -p hearthd -- --config hearth.example.toml
open http://127.0.0.1:9000
```

UI development with hot reload (proxies `/api` to a running hearthd on :9000):

```bash
pnpm -C web dev        # http://localhost:5173
```

Everything listens on loopback. Until the gateway is applied, reach it from another device
with an SSH tunnel: `ssh -L 9000:localhost:9000 m1`.

## Layout

| Path | What |
|---|---|
| `crates/hearth-core` | Manifests, catalog, capacity planner (pure, no I/O) |
| `crates/hearth-gateway` | Caddyfile + gateway compose rendering (pure) |
| `crates/hearth-host` | Host metrics, Docker API, `docker compose`, `tailscale` |
| `crates/hearthd` | The daemon: config, HTTP API, sampler, serves the embedded UI |
| `web/packages/ui` | `@hearth/ui` design system: Kumo + Geist, motion tokens, data viz, pets |
| `web/apps/dashboard` | React dashboard (Vite, TanStack Query, Motion) |
| `catalog/` | App manifests (the marketplace) |
| `docs/PLAN.md` | Decisions, architecture, roadmap |

## API

| Method | Path | |
|---|---|---|
| GET | `/api/health` | liveness |
| GET | `/api/system` | latest sample + 1 h history |
| GET | `/api/containers` | all containers with memory/CPU |
| GET | `/api/tailscale` | tailnet name and IPs |
| GET | `/api/catalog` | apps + installed/running + capacity verdict |
| POST | `/api/apps/{id}/install` | body `{"dry_run":true}` to preview, `{"force":true}` to override a deny |
| POST | `/api/apps/{id}/remove` | stops containers, keeps data |
| GET | `/api/gateway` | rendered Caddyfile / compose / Authelia config |
| … | `/api/settings`, `/api/tailscale/*`, `/api/gateway/*`, `/api/cloudflare/*`, `/api/tunnels/*` | Settings page |
| … | `/api/storage/*`, `/api/containers/*`, `/api/routes`, `/api/discover/*`, `/api/events`, `/api/setup` | see `crates/hearthd/src/api/` |

## Writing an app manifest

```yaml
id: uptime-kuma               # [a-z0-9-], also the compose project suffix
name: Uptime Kuma
description: Monitors your services.
category: monitoring
port: 3002                    # https://<ts-host>:3002
upstream: { service: uptime-kuma, port: 3001 }
resources: { ram_mb: 150 }    # needs_vault / vault_gb for SSD apps
auth:
  exempt_paths: ["/api/push/*"]   # skip Authelia (app's own auth still applies)
compose: |
  services:
    uptime-kuma:
      image: louislam/uptime-kuma:2
      volumes: ["${APP_DATA}:/app/data"]
```

Variables available in `compose`: `${APP_DATA}`, `${APP_SECRET}`, `${VAULT}`, `${TS_HOST}`, `${TZ}`.

## Development

```bash
cargo fmt --all
cargo clippy --all-targets --all-features   # must be warning-free
cargo test --workspace
pnpm -C web typecheck && pnpm -C web build
```

Frontend stack: React 19 · Vite · Tailwind v4 · [Kumo](https://github.com/cloudflare/kumo) (Cloudflare) ·
Geist (Vercel) · Motion · TanStack Query · Phosphor icons. See `web/packages/ui/README.md`.
