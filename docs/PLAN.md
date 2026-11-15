# hearth: plan

A single native Rust daemon that manages a small self-hosted machine (first target: an
8 GB M1 Mac with OrbStack, Tailscale and a 1 TB external SSD).

## Goals

1. **Marketplace**: install/remove apps from a catalog of manifests, one click.
2. **Capacity-aware**: refuse or warn before an install would overload RAM/disk, based on
   declared needs + live measurements.
3. **Auth on everything**: every app is published behind one Authelia login (2FA), with
   per-app exemptions for mobile-app APIs/webhooks. Nothing is published without auth.
4. **Connectivity**: Tailscale HTTPS for private access; Cloudflare Tunnel for opt-in public apps.
5. **Observability**: live + historical CPU / memory / swap / disk, per-container usage.

Non-goals (for now): multi-node, Kubernetes, Windows, running hearth itself in a container.

## Key decisions

| Decision | Why |
|---|---|
| hearth runs **natively** (launchd), apps run in Docker | Inside a container it would see OrbStack's VM, not the Mac: wrong metrics, no access to native apps (Ollama, Jellyfin) or the `tailscale` CLI. |
| **No k3s** | ~0.5–1 GB overhead on an 8 GB box; single node gains nothing over compose. |
| Apps are **docker compose projects** (`hearth-<id>`) | Huge existing ecosystem; manifests embed compose verbatim; `docker compose` handles the hard parts. |
| **Docker labels are the source of truth** for "installed" | No database to drift out of sync; reconcile = list containers. |
| Gateway = **Caddy + Authelia** on a shared `hearth` network, ports on 127.0.0.1 only, published by `tailscale serve` | Proven in `~/homeserver`; Caddy config is easy to generate; every route passes `forward_auth`. |
| Pure crates (`hearth-core`, `hearth-gateway`) vs side-effect crate (`hearth-host`) | Decision logic and rendering are unit-testable without Docker/Tailscale. |
| axum + tokio, thiserror in libs, anyhow in the binary | Mainstream, Tokio-team maintained, minimal magic. |
| React dashboard embedded with `rust-embed` | Still one binary. Debug builds read `web/apps/dashboard/dist` from disk, release builds embed it. |
| Kumo (Cloudflare) components + Geist (Vercel) type, wrapped in `@hearth/ui` | Accessible Base UI primitives and semantic light/dark tokens; one shared design system for every surface. |
| Motion for interactions, TanStack Query for live data | Spring-based small interactions; polling + cache invalidation after installs. |

## Architecture

```
crates/
  hearth-core      pure: AppManifest, Catalog, capacity::evaluate
  hearth-gateway   pure: Route, render_caddyfile, render_compose
  hearth-host      I/O: system metrics (sysinfo), Docker (bollard), docker compose, tailscale CLI
  hearthd          binary: config, axum API, background sampler, embedded UI
catalog/           app manifests (YAML), the marketplace
```

Request path once the gateway is live:

```
https://<ts-host>:<port> → tailscale serve → 127.0.0.1:<port> Caddy
   → forward_auth authelia:9091 → <service>:<port> on network `hearth`
hearth UI: https://<ts-host>:9443 → Caddy → host.docker.internal:9000 (hearthd)
```

## Phases

### Phase 1: foundation ✅ (this commit)
- [x] Workspace, lints (`unsafe_code = forbid`, clippy pedantic), release profile
- [x] Manifest format + validation, catalog loading (port clash / duplicate detection)
- [x] Capacity planner (RAM reserve, headroom warning, disk floor, vault presence/space)
- [x] Host sampler (CPU, load, memory, swap, disks) with 1 h history ring buffer
- [x] Docker inventory + per-container memory/CPU (same formula as `docker stats`)
- [x] Install/remove via compose (shared network override, stable per-app `APP_SECRET`)
- [x] Gateway rendering (Caddyfile + compose) with exemptions
- [x] API: `/api/{health,system,containers,tailscale,catalog,gateway}`, `POST /api/apps/{id}/{install,remove}` (dry-run, force)
- [x] Optional `Remote-User` enforcement middleware
- [x] Dashboard: metrics, CPU chart, marketplace with verdicts, containers
- [x] 8 starter manifests ported from `~/homeserver`

### Phase 1.5: React dashboard ✅
- [x] pnpm workspace: `@hearth/ui` design system + `@hearth/dashboard` app
- [x] Kumo + Geist, theme (system/light/dark, no flash), motion tokens, reduced-motion support
- [x] Overview (animated stats, sparklines, storage, running apps), Apps (search, URL-synced filters,
      capacity verdict, install dialog with RAM projection, confirm-to-remove), Containers table
- [x] Pets: Ember + Miso pixel sprites whose mood tracks system load and install activity
- [x] Vendor chunk splitting; served by hearthd with SPA fallback + immutable asset caching
- [x] Command palette (⌘K) for apps/actions; per-app detail page with logs
- [ ] Notifications feed (installs, capacity warnings) using Kumo toasts history

### Phase 1.6: everything requested ✅
- [x] Storage: drive detection (diskutil/lsblk), markers + UUID identity, role links with `/dev/null` offline
      sentinel, impostor detection, auto pause/resume, attach/detach/eject; verified with disk images
- [x] Settings page: setup checklist, general, Tailscale (login link, connect, rename, serve ports), logins
      (argon2id), gateway on/off/apply, Cloudflare automation, Funnel, ngrok
- [x] Gateway live: Authelia config/users/secrets rendering, `docker compose up`, `caddy reload`,
      `tailscale serve`/unserve diffing, domain host routing, open sites for Funnel/ngrok
- [x] Marketplace v2: App Store layout, logos, ratings, leaderboards, Top 5 of the month,
      Discover (awesome-selfhosted-data, 1,360 apps), install-time Docker config form, custom apps
- [x] Containers: detail sheet, actions, SSE logs, adopt external containers
- [x] Pets speak (event feed → speech bubbles + toasts)
- [ ] Verify on the M1: gateway end-to-end, Cloudflare/Funnel/ngrok with real accounts,
      `host.docker.internal` → loopback hearthd under OrbStack
- [x] launchd agent: `hearthd service install|uninstall|status`; Homebrew `service do`; Hearth.app menu bar + DMG

### Phase 1.7: polish + reach ✅
- [x] Settings redesign: section sidebar, setting groups/rows, sticky save bar (`@hearth/ui/components/settings`)
- [x] No browser-default controls: Kumo Select / searchable Combobox / Checkbox / custom radios; text search inputs
- [x] Dialog anatomy (`DialogFrame`): fixed header/footer, scrolling body; mobile overflow audit at 390px (CDP) clean
- [x] Overview: health hero with issues, radial gauges, 1h charts, app launcher, top memory, activity timeline
- [x] Container terminal (docker exec TTY over WebSocket, xterm.js, lazy-loaded)
- [x] ⌘K global search + app search on Tantivy (in-process BM25, prefix, typo tolerance; ~1 ms)
- [x] Discover detail page (description, licenses, gallery via cached GitHub card, activity bars, rating, similar apps)
- [x] Category browser (tiles, chip rail, searchable sheet)
- [x] Network: NetBird (connect, serve over internal-CA HTTPS, CA download), Headscale (install wizard, users, keys, join)
- [x] Always On: stay awake (caffeinate), server power settings via admin prompt, connectivity watchdog with Wi-Fi/Tailscale repair
- [x] Signed-in devices: sessions in Redis, Caddy access-log stream, sign out per device / person / everyone; identity headers stripped everywhere; startup reconcile
- [x] Tailnets without HTTPS certificates: hearth's own CA on the Tailscale IP, certificate download + per-device install steps
- [x] Ember companion via agent-pet (Apache-2.0) with a runtime-rendered atlas; works around agent-pet 0.8.6 mount bug

### Phase 1.8: ops suite ✅ (branch `feature/ops-suite`)
- [x] Git repository, one commit per feature
- [x] Per-app 7-day history; measured RAM in install checks
- [x] Backups and restore (restic in a container; Local / S3 / B2; recovery password shown once)
- [x] Two-factor (TOTP) enrollment per login via Authelia's storage CLI, QR + verify-before-enable
- [x] Settings: Cloudflare-style collapsible left tree with scroll-spy
- [x] Updates: registry digest checks, pre-update backup, health gate, automatic rollback + pin, auto-update
- [x] Alerts: ntfy / Telegram / webhook; crash, recovery, low disk, memory pressure, failed sign-in monitors
- [x] Packs: Local AI (native Ollama + models), Family Cloud, Media Server, Security & Privacy, Developer,
      Home Lab, Storage & Sync; 7 new catalog apps
- [x] PWA: manifest, icons, service worker, home-screen install
- [x] Docs: user guide, operations, architecture, API, catalog, repo standards, contributing

### Phase 2: gateway goes live (mostly done in 1.6)
- [ ] Bootstrap Authelia (secrets, `configuration.yml`, first user) from `hearth init`
- [ ] `gateway::apply`: write files → `docker compose up -d` (ports changed) or `caddy reload` (routes only)
- [ ] `tailscale serve` per route; `unserve` on removal; reconcile on startup
- [ ] Manifest `upstream.host_header` (Syncthing needs `Host: localhost`)
- [ ] Set `require_remote_user = true` by default once the gateway fronts hearthd
- [ ] Verify `host.docker.internal` reaches the loopback-bound hearthd under OrbStack
- [x] launchd agent: `hearthd service install|uninstall|status`; Homebrew `service do`; Hearth.app menu bar + DMG

### Phase 3: operations
- [x] Start/stop/restart/logs per app; update (pull + recreate) with "update available" badge, health gate and automatic rollback
- [x] Per-app usage over time (7-day 5-minute buckets in `history.json`; SQLite not needed)
- [x] Capacity: use *measured* per-app RAM (p95) once installed, not just declared
- [x] Native Ollama via packs (Homebrew + brew services, model pulls); general `kind: native` apps still open
- [x] Backups: restic per app (`APP_DATA` + vault paths), SSD/S3/B2, retention, exact restore

### Phase 4: marketplace scale-out
- [ ] Importer for an existing app store format (Runtipi / CasaOS) → hundreds of apps
- [ ] Per-app settings form (env vars declared in the manifest)
- [ ] Cloudflare Tunnel integration for apps marked `public: true` (still behind Authelia unless exempted)
- [ ] OIDC client registration in Authelia for apps that support SSO
- [ ] Linux support (systemd unit; sysinfo/Docker already portable)

## Open questions
- Port allocation: fixed per manifest today; may need auto-allocation when importing large catalogs.
- How to model apps with multiple web UIs (AdGuard's setup wizard on another port).
