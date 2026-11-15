# HTTP API

Everything the dashboard does goes through this JSON API on hearthd's loopback address
(`/api/...`). Handlers live in `crates/hearthd/src/api/<module>.rs`; the module is noted per section.

Conventions:

- JSON in and out. Errors are `{"error": "message"}` with `400` (bad input), `404`, `409` (conflict,
  e.g. a capacity *deny*, with `verdict`), or `500`.
- Long operations (updates, backups, pack installs) **start in the background** and return
  `{"started": true}`; progress is in the matching `GET`, and the outcome arrives as an event.
- With *Lock down the dashboard* on, every route except `/api/health` requires the `Remote-User`
  header that Authelia sets.

## System (`system.rs`)

| Method | Path | |
|---|---|---|
| GET | `/health` | Liveness (always open) |
| GET | `/system` | Latest host sample + 1 h history |
| GET | `/events` | Activity feed (`?after=<seq>`) |
| GET | `/setup` | Setup checklist items with state |
| GET | `/search?q=` | ⌘K search across apps, Discover, containers, drives, settings |

## Apps (`apps.rs`)

| Method | Path | |
|---|---|---|
| GET | `/catalog` | Apps with installed/running, capacity verdict, stats, ratings |
| POST | `/apps/{id}/install` | `{dry_run?, force?, config?}`; dry run returns verdict, plan and resolved compose |
| POST | `/apps/{id}/remove` | Stop and remove containers; **data is kept** |
| POST | `/apps/{id}/{start\|stop\|restart}` | Lifecycle |
| GET | `/apps/{id}/config` | Saved Docker config form |
| PUT | `/apps/{id}/override` | Port / subdomain / public exposure |
| GET | `/apps/{id}/history` | 7-day usage summary + buckets |
| POST | `/custom-apps` | Add your own app (image or compose) |
| DELETE | `/custom-apps/{id}` | Remove a custom app definition |
| PUT | `/ratings/{key}` | Your rating for an app |
| GET | `/icons/{file}` | App logo |

## Packs (`packs.rs`)

| Method | Path | |
|---|---|---|
| GET | `/packs` | Every pack with plan, capacity verdict, Ollama status and current job |
| POST | `/packs/{id}/plan` | `{with: [optional ids/models]}` → plan for that choice |
| POST | `/packs/{id}/install` | `{with, force?}`; starts the job (409 if running or doesn't fit) |

## Updates (`updates.rs`)

| Method | Path | |
|---|---|---|
| GET | `/updates` | Per-app status (`available`, `pinned`, services), auto list, history, running |
| POST | `/updates/check` | Check every installed app now |
| POST | `/updates/{id}` | Update with health gate and automatic rollback (background) |
| PUT | `/updates/{id}/auto` | `{enabled}` |
| POST | `/updates/{id}/unpin` | Release the rollback hold |

## Backups (`backups.rs`)

| Method | Path | |
|---|---|---|
| GET | `/backups` | Settings, last results, running, whether a repository exists |
| PUT | `/backups/settings` | Destination, hour, retention, pause, media, credentials |
| POST | `/backups/init` | Create the repository; returns the recovery password |
| GET | `/backups/password` | Show the recovery password again |
| POST | `/backups/run` | `{app?}`; back up one app or all (background) |
| GET | `/backups/{id}/snapshots` | Snapshots for an app |
| POST | `/backups/{id}/restore` | `{snapshot}`; stop, restore exactly, start |

## Alerts (`alerts.rs`)

| Method | Path | |
|---|---|---|
| GET | `/alerts` | Settings + whether tokens are saved |
| PUT | `/alerts` | `{settings, ntfy_token?, telegram_bot_token?}` |
| POST | `/alerts/test` | Send a test to every enabled channel → `{sent, errors}` |

## Containers (`containers.rs`, `terminal.rs`)

| Method | Path | |
|---|---|---|
| GET | `/containers` | All containers with CPU/memory |
| GET | `/containers/{id}` | Details (env secrets redacted) |
| GET | `/containers/{id}/logs` | Recent logs |
| GET | `/containers/{id}/logs/stream` | Server-sent events |
| POST | `/containers/{id}/{start\|stop\|restart}` | Lifecycle |
| GET | `/containers/{id}/terminal` | WebSocket TTY (`docker exec`) |
| GET, POST | `/routes` | Published external containers (adopt) |
| DELETE | `/routes/{id}` | Unpublish |

## Storage (`storage.rs`)

| Method | Path | |
|---|---|---|
| GET | `/storage` | Drives, roles, attachments, health |
| POST | `/storage/refresh` | Rescan |
| POST | `/storage/attach` | Attach a drive to a role |
| POST | `/storage/{id}/detach` | Detach (apps on that role pause) |
| POST | `/storage/{id}/eject` | Stop dependent apps, then eject |

## Discover (`discover.rs`)

| Method | Path | |
|---|---|---|
| GET | `/discover` | Search/filter awesome-selfhosted entries |
| GET | `/discover/meta` | Tags, counts, sync time |
| GET | `/discover/leaderboards` | Popular / active lists |
| POST | `/discover/sync` | Refresh now |
| GET | `/discover/app/{slug}` | Detail page data |
| GET | `/discover/app/{slug}/preview` | Cached GitHub social card |

## Settings, access and networks (`settings.rs`)

| Method | Path | |
|---|---|---|
| GET, PUT | `/settings` | User settings |
| GET | `/tailscale` | Status, name, IPs |
| POST | `/tailscale/login`, `/tailscale/logout` | Connect / disconnect |
| PUT | `/tailscale/hostname` | Rename |
| GET | `/tailscale/serve` | Published ports |
| POST | `/tailscale/serve/reset` | Clear published ports |
| POST | `/tailscale/login-server` | Join a custom control server (Headscale) |
| GET | `/gateway` | Rendered Caddyfile / compose / Authelia config |
| POST | `/gateway/enable`, `/gateway/disable`, `/gateway/apply` | Gateway lifecycle |
| GET | `/gateway/ca.crt` | hearth's internal CA (NetBird HTTPS) |
| GET, POST | `/gateway/users` | Logins (`totp` flag per user) |
| DELETE | `/gateway/users/{name}` | Remove a login |
| POST | `/gateway/users/{name}/totp` | Start 2FA enrollment → `{uri, secret, qr_svg}` |
| POST | `/gateway/users/{name}/totp/verify` | `{secret, code}`; turns 2FA on if the code is valid |
| DELETE | `/gateway/users/{name}/totp` | Turn 2FA off |
| GET | `/cloudflare` | Tunnel status |
| POST | `/cloudflare/connect`, `/cloudflare/disconnect` | Set up from an API token / remove |
| PUT | `/tunnels/funnel`, `/tunnels/ngrok` | Public links per app |
| GET | `/netbird` | Status |
| POST | `/netbird/up`, `/netbird/down` | Connect / disconnect |
| PUT | `/netbird/expose` | Serve apps on NetBird |
| GET, DELETE | `/headscale` | Status / uninstall |
| POST | `/headscale/install`, `/headscale/users`, `/headscale/keys`, `/headscale/join-self` | Wizard steps |
