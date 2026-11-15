# hearth

A small, native control plane for a self-hosted machine: an app **marketplace** with
**capacity-aware** installs, **one login (Authelia + 2FA) in front of everything**, and
**Tailscale** connectivity. Built for an 8 GB M1 Mac running OrbStack; Linux later.

## What it does

- **App Store**: editor's picks, *Top 5 this month*, *Recommended for this Mac*, leaderboards, ratings and
  real logos. One-click installs for curated apps; **Discover** browses ~1,360 apps from
  [awesome-selfhosted](https://awesome-selfhosted.net).
- **Packs**: whole stacks in one click: **Local AI** (native Ollama on the GPU + models, Open WebUI,
  SearXNG), Family Cloud, Media Server, Security & Privacy, Developer, Home Lab, Storage & Sync.
- **Capacity-aware**: every install (and pack) is checked against live RAM and disk, using measured
  per-app usage once it exists.
- **Install your way**: every Docker setting is an editable form; bring your own image/compose or adopt
  a running container.
- **One login for everything**: Caddy + Authelia in front of every app, with **two-factor** (authenticator
  app, QR enrollment) and passkeys, over Tailscale HTTPS.
- **Backups**: nightly, encrypted, deduplicated restic snapshots per app to an SSD, S3/R2/MinIO or B2,
  with one-click exact restore.
- **Updates with automatic rollback**: digest checks, pre-update backup, health gate, and automatic
  rollback to the previous images and data if the new version isn't healthy. Optional auto-update.
- **Alerts**: ntfy, Telegram or webhook notifications for crashes, low disk, memory pressure, failed
  sign-ins, backup failures and rollbacks.
- **Storage that knows its drives**: Vault / Media / Backups roles, drive identity, pause on unplug,
  resume on return, safe eject.
- **Networks and domains**: Tailscale, NetBird or your own Headscale; Cloudflare Tunnel for your domain;
  Funnel and ngrok links.
- **Containers**: live stats, logs, actions and an in-browser terminal.
- **⌘K everywhere**: in-process Tantivy search across apps, Discover, containers, drives and settings.
- **Installable**: a PWA for your phone's home screen.
- **Ember**: a pixel companion that reacts to what your server is doing.

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
| `crates/hearth-host` | Host metrics, Docker API, `docker compose`, `tailscale`, `netbird`, drives, Ollama |
| `crates/hearth-search` | Tantivy index for ⌘K |
| `crates/hearthd` | The daemon: config, HTTP API, sampler, serves the embedded UI |
| `web/packages/ui` | `@hearth/ui` design system: Kumo + Geist, motion tokens, data viz, pets |
| `web/apps/dashboard` | React dashboard (Vite, TanStack Query, Motion) |
| `catalog/` | App manifests, logos (`icons/`) and packs (`packs/`) |
| `docs/` | Documentation (start at `docs/README.md`) |

## Documentation

| | |
|---|---|
| [User guide](docs/user-guide.md) | Every feature, how to use it |
| [Operations](docs/operations.md) | Running it, config, state directory, recovery |
| [Architecture](docs/architecture.md) | Crates, daemon, background tasks, security model |
| [API](docs/api.md) | Every endpoint |
| [Catalog](docs/catalog.md) | Writing app manifests and packs |
| [Standards](docs/STANDARDS.md) | Repo standards and patterns |
| [Plan](docs/PLAN.md) | Decisions and roadmap |

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md) and [docs/STANDARDS.md](docs/STANDARDS.md). Every commit passes:

```bash
cargo fmt --all
cargo clippy -q --all-targets --all-features -- -D warnings
cargo test -q --workspace
pnpm -C web -r typecheck && pnpm -C web --filter @hearth/dashboard build
```
