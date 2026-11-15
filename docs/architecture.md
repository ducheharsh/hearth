# Architecture

hearth is one native Rust daemon (`hearthd`) plus a React dashboard compiled into it. Apps run as
Docker Compose projects; hearth itself does **not** run in a container, because inside one it would
see OrbStack's VM instead of the Mac: wrong RAM/disk numbers, no `tailscale` CLI, no native apps
(Ollama), no `diskutil`.

```
                       phone / laptop (Tailscale, NetBird, Cloudflare, Funnel, ngrok)
                                              │ HTTPS
                ┌─────────────────────────────▼──────────────────────────────┐
                │ hearth-gateway compose project                             │
                │   Caddy (one port per app, domain hosts) ──► Authelia (SSO,│
                │   2FA/TOTP, passkeys)                                      │
                └───────┬─────────────────────────────────┬──────────────────┘
                        │ shared docker network            │ host.docker.internal
          ┌─────────────▼───────────┐            ┌─────────▼──────────┐
          │ app compose projects     │            │ hearthd (loopback) │◄── launchd
          │ hearth-<id>              │            │  axum API + UI     │
          │ ${APP_DATA} ${VAULT} …   │            │  background tasks  │
          └──────────────────────────┘            └─────────┬──────────┘
                                                            │
                       Docker API (bollard) · docker compose · tailscale · netbird · diskutil
                       brew (Ollama) · restic (container) · Authelia CLI (container)
```

## Crates

| Crate | Role | Rules |
|---|---|---|
| `hearth-core` | Domain model: manifests, catalog, capacity planner, packs, backup path selection, storage roles, Discover data | **Pure.** No tokio, no processes, no network. Deterministic and unit-tested. |
| `hearth-gateway` | Renders Caddyfile, gateway compose, Authelia config/users; argon2id hashing | **Pure.** Strings in, strings out. |
| `hearth-host` | Side effects: sysinfo, Docker API (bollard), `docker compose`, `tailscale`, `netbird`, drives, native Ollama | All I/O lives here. `thiserror` errors. |
| `hearth-search` | Tantivy index for ⌘K (apps, Discover entries, containers, drives, settings) | In-process, ~1 ms queries. |
| `hearthd` | The daemon: config, state, HTTP API, background tasks, embedded UI | Only crate that uses `anyhow`. |

Dependencies flow one way: `hearthd → hearth-host → hearth-core`, `hearthd → hearth-gateway/search → hearth-core`.

## The daemon

`main.rs` loads `hearth.toml`, the catalog (`catalog/*.yml` + custom apps), packs (`catalog/packs/*.yml`),
and persisted state, builds one `AppState` (shared as `SharedState = Arc<AppState>`), spawns the
background tasks, and serves `/api/*` plus the dashboard on a **loopback** address.

### Shared state (`state.rs`)

Everything mutable sits behind `tokio::sync::RwLock`/`Mutex` on `AppState`:

| Field | What |
|---|---|
| `settings` | User settings (`settings.json`) |
| `catalog` | Built-in + custom manifests |
| `metrics` | Latest host sample + 1 h ring buffer |
| `events` | Activity feed (ring buffer); every `st.event(..)` also goes to alerts |
| `history` | Per-app 7-day usage buckets |
| `backups`, `updates`, `alerts`, `pack_jobs` | Feature state |
| `ops` | **Global operations lock**: anything that runs `docker compose` for an app holds it |

### Background tasks

| Task | Every | Does |
|---|---|---|
| `state::run_sampler` | 5 s | Host metrics (RAM, swap, CPU, disks) |
| `state::run_tailscale_poller` | 15 s | Tailscale status / MagicDNS name |
| `storage::run_watcher` | 3 s | Drive arrivals/removals → pause/resume apps on that role |
| `discover::run_syncer` | daily | Refresh awesome-selfhosted data + GitHub stats |
| `search::run_indexer` | 15 s | Re-index when apps, containers or drives change |
| `history::run_recorder` | 30 s | Per-app memory/CPU into 5-minute buckets |
| `backup::run_scheduler` | 5 min | Nightly restic run at the configured hour |
| `updates::run_checker` | 6 h | Registry digest checks; auto-updates opted-in apps |
| `alerts::run_dispatcher` | event-driven | Forward events to ntfy / Telegram / webhook |
| `alerts::run_monitors` | 60 s | App crashes/recoveries, low disk, memory pressure |
| `alerts::run_login_watch` | 30 s | Failed sign-ins from Authelia's log |

### Events

`st.event(level, source, title, message)` is the single way to tell the user something happened. It
appends to the activity feed (shown on Overview, spoken by Ember, toasted) **and** is offered to the
alert dispatcher, which filters by level and de-duplicates.

## How an app install works

1. `GET /api/catalog` returns each manifest with a **capacity verdict** (`hearth-core::capacity`):
   declared RAM (or measured p95 after 1 h of history) vs available RAM minus a reserve, plus disk/SSD checks.
2. The install dialog merges the user's Docker config form into the manifest compose
   (`compose_config::resolve`).
3. `POST /api/apps/{id}/install` takes the `ops` lock, then `hearth-host::compose` writes
   `<state>/apps/<id>/{compose.yml, hearth.override.yml, .env}` (shared network, stable `APP_SECRET`)
   and runs `docker compose up -d`.
4. `gateway::apply` re-renders Caddy routes and `tailscale serve` mappings so the app is reachable at
   `https://<host>:<port>` **behind Authelia**.

## Security model

- hearthd binds **loopback only**. Remote access always goes through the gateway.
- Every published route passes Authelia unless the manifest sets `auth.protect: false` or lists
  `exempt_paths` (e.g. mobile APIs that have their own auth).
- Secrets (`<state>/secrets/`, `gateway/authelia.env`) are written `0600`. Tokens and passwords are
  write-only through the API, with two deliberate exceptions: the 2FA secret during enrollment, and
  the backup recovery password (`GET /api/backups/password`, so it can be shown again). Turn on
  *Lock down the dashboard* (`require_remote_user`) so the API itself sits behind Authelia.
- App data is never deleted by hearth: remove stops containers and keeps `<state>/apps/<id>/data`.
- Two-factor (TOTP) is enrolled through Authelia's own storage CLI; hearth verifies a code before
  marking a login enrolled.

## Frontend

`web/` is a pnpm workspace:

- `packages/ui` (`@hearth/ui`): the design system. Kumo (Cloudflare) components, Geist type, motion
  tokens, settings layout, dialog frame, form controls, gauges, logos, pets.
- `apps/dashboard`: the app. TanStack Query for server state (`src/api/hooks.ts`), URL search params
  for view state (`?tab=apps&view=discover&app=immich`), feature folders under `src/features/`.

Release builds embed `apps/dashboard/dist` into `hearthd` (rust-embed); debug builds read it from disk.
A service worker (`public/sw.js`) caches hashed assets and the shell; `/api` is never cached.

See [STANDARDS.md](STANDARDS.md) for the patterns every change follows.
