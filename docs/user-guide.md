# User guide

What every part of hearth does and how to use it. For setting the machine up, see
[operations.md](operations.md).

## Product tour

The first time you open hearth on a computer, a one-minute tour walks through the address bar, health,
Packs, the App Store, Backups, Settings, ⌘K search and Ember. Replay it anytime with the **?** button in
the header.

## Overview

The home screen. At the top is **your server's address**: the link to open hearth from your other
devices (your Tailscale name on port 9443 once the gateway is on), with copy, **Open** and
**Scan on Phone** (a QR code). Other ways in are listed under it (your domain, NetBird, this Mac).

**Remote access needs four things**, and until they're done the bar lists them with **Fix**/**Open**
buttons: (1) connect Tailscale, (2) turn on MagicDNS and HTTPS Certificates for your tailnet in the
Tailscale admin console, (3) create a login, (4) turn on the gateway. Port 9000 itself is never
reachable from other devices; the gateway publishes the dashboard on 9443 behind your login.

**Can't turn on HTTPS Certificates** (a work tailnet, for example)? Use **hearth's own certificate**
instead (step 2 has a button for it). The address bar then shows *First, install hearth's
certificate on each device* with **Install on Phone** (a QR code), **This Mac** (download) and
**How?** (steps for iPhone/iPad, Android, Mac and Windows). Trust it once per device; until then
browsers show a warning.

**Your Apps** comes next. Below it, a health card (collapsible; it remembers your choice) lists anything that needs attention (setup steps, apps that were
rolled back, available updates, drives offline), with radial gauges for memory, CPU and disk, 1-hour
charts, your app launcher, top memory users and the activity timeline.

**Ember** (the fire sprite, bottom right) reacts to load, installs and drive events and comments on
what's happening. Click it to talk, or hide it from the header.

## Apps

### Today, One-Click, Discover

- **Today**: editor's picks, **Packs**, *Top 5 this month* (by commits), *Recommended for this Mac*
  (fits comfortably right now) and leaderboards (popular, top rated by you, lightest).
- **One-Click**: every curated app, filterable by category tiles, each with a capacity badge.
- **Discover**: all ~1,360 apps from [awesome-selfhosted](https://awesome-selfhosted.net) with search,
  tags, stars and activity. *View* opens a hearth detail page (description, licences, gallery,
  activity, similar apps).

### Installing an app

Open an app, then **Install**. The dialog shows the capacity check (RAM after install, SSD space) and
an editable Docker config (image tag, environment, ports, volumes, restart policy, CPU/RAM limits or
raw compose) pre-filled with recommended defaults. If the app doesn't fit, you can still
**Install Anyway**.

Every installed app is published at `https://<your-mac>.<tailnet>.ts.net:<port>` behind your login.

### Packs

Packs set up a whole use case in one go. Open one from the Store's **Packs** row to see each app's
role, toggle optional parts, and check the combined capacity before installing. Progress is shown
step by step; you can close the dialog and it keeps going.

| Pack | Includes | Optional |
|---|---|---|
| **Local AI** | Ollama (native, GPU) + `llama3.2:3b` + `nomic-embed-text`, Open WebUI, SearXNG | n8n, `qwen2.5-coder:3b`, `gemma3:4b` |
| **Family Cloud** | Immich, Syncthing, Vaultwarden | Paperless-ngx |
| **Media Server** | Jellyfin, Navidrome | Audiobookshelf |
| **Security & Privacy** | AdGuard Home, Vaultwarden | Uptime Kuma |
| **Developer** | Forgejo, n8n, Uptime Kuma | |
| **Home Lab** | Homepage, Uptime Kuma, AdGuard Home | Syncthing |
| **Storage & Sync** | File Browser, Syncthing | Paperless-ngx, Immich |

Ollama runs **natively** (installed with Homebrew and started as a login service) because Docker on
macOS cannot use the GPU. Only one model is loaded at a time, so the capacity check counts the largest.

### App details

The app sheet shows status, URL, your rating, **Updates**, **Usage** (7-day average/p95/peak memory
and CPU), and settings for port, subdomain and public exposure.

### Updates

hearth checks every installed image against its registry every 6 hours without downloading anything.
**Update Now**:

1. takes a pre-update backup (if backups are set up),
2. records the exact image digests that are running,
3. pulls and recreates,
4. waits for the app to stay running (and healthy, if it has a health check) for 25 s, within 2 minutes.

If it doesn't, hearth **rolls back automatically**: it pins the previous digests
(`hearth.pin.yml`) and restores the pre-update data. The app shows *Held at previous version* until you
**Release Hold**. Turn on **Update automatically** per app to do this after each check.

### Your own apps

**Add Your Own** turns any image or compose file into an app, or adopts a container you already run.

## Containers

Every container with live CPU/memory, details (image, ports, mounts, environment with secrets
redacted), start/stop/restart, streaming logs and an in-browser **terminal** (`docker exec`).

## Storage

Attach drives to **roles**: *Vault* (photos, files), *Media* (movies, music) and *Backups*. hearth
writes a marker to recognise the same drive after renames or remounts, refuses impostors, pauses the
apps that use a role when its drive is unplugged and resumes them when it returns. **Eject** stops
those apps first.

## Backups

Nightly, per-app, encrypted, deduplicated snapshots with [restic](https://restic.net).

- **Destination**: the Backups drive, S3-compatible storage (AWS, R2, MinIO, …) or Backblaze B2.
- **What's included**: each app's data folder and its Vault folders. Media is optional (large and
  usually replaceable). Backups never back up backups.
- **Consistency**: apps are paused during their backup by default.
- **Retention**: keep 7 daily, 4 weekly and 6 monthly snapshots (adjustable).

**Set Up Backups** shows a **recovery password once**. Store it in your password manager: without
it the backups cannot be read. **Restore** stops the app, restores the chosen snapshot exactly (files
added since are removed), and starts it again.

## Settings

The left tree jumps to any group; the group in view is highlighted.

- **Setup**: a checklist of what's left (Tailscale, logins, gateway, drives, backups).
- **General**: name, time zone, app directory.
- **Network**: Tailscale (connect, rename, HTTPS, **Certificates**: Tailscale's or hearth's own,
  published ports, custom control server),
  NetBird (connect, serve apps with hearth's certificate authority), Headscale (install wizard, users,
  keys, join this Mac).
- **Users**: people, roles and per-app access (below).
- **Gateway**: turn the gateway on and see what's published.
  Turning the gateway on shows each step live (check tailnet → write config → download Caddy and
  Authelia → start → publish on Tailscale) and explains any failure with **Try Again**. The toggle stays
  off, with the reason, until Tailscale, HTTPS certificates and a login are in place.
- **Alerts**: notification channels and what to watch.
- **Domains & Tunnels**: your domain via Cloudflare Tunnel (from an API token), Tailscale Funnel, ngrok.
- **Capacity**: memory reserve and disk reserve used by install checks.
- **Advanced**: require the gateway login for the dashboard itself.

### Users, roles and app access

Settings → **Users** lists everyone who can sign in. **Add User** or **Edit** opens one dialog for name,
email, password reset, role, app access, two-factor, disabling and deleting.

- **Admin**: every app, plus the hearth dashboard (settings, users, installs). The first user is an
  admin, and hearth always keeps at least one active admin.
- **Member**: only apps. Either **All apps** (including ones installed later) or a chosen list. Members
  can't open the dashboard.
- **Disable sign-in** blocks a login without deleting it.

**Signed-in devices** lists every device that's signed in right now, grouped by person: the
browser and device ("Safari on iPhone"), the tailnet device name when hearth can see it, which apps it
used, and when it was last active. The device you're using is marked *This device*. **Sign Out** ends
that device's session immediately; **Sign Out All** does it for one person's devices and **Sign Out
Everyone** for everybody (they sign in again, with 2FA; logins aren't changed). Disabling a user,
deleting them or setting a new password also signs them out everywhere. A device shows up once it
opens an app or the dashboard after signing in.

**App access** shows, per installed app, who can open it. Access is enforced by the gateway: Authelia
tells Caddy who signed in and their groups, and Caddy shows "You don't have access" to anyone not
allowed. Apps that are public (no sign-in) can't be limited. Changes apply immediately.

### Two-factor

Settings → Users → **Edit** → **Set Up 2FA**. Scan the QR code with any authenticator app
(1Password, Google/Microsoft Authenticator, Authy, Bitwarden), or type the key, then enter the 6-digit
code. 2FA is only switched on once a code verifies. **Reset** issues a new secret; **Remove** turns it
off. Passkeys can also be registered from Authelia's sign-in page.

### Alerts

Send notifications to **ntfy** (easiest: install the app, subscribe to your topic), **Telegram** (bot
token + chat ID) or a **webhook** (JSON `{server, level, source, title, message, at}`).

Watched by default: app crashes and failing health checks (and recoveries), low internal disk,
sustained memory pressure, failed sign-ins, plus anything hearth already reports (backup failures,
update rollbacks, drives disappearing). Choose the minimum level; repeats within 10 minutes are
dropped. **Send Test** checks every enabled channel.

## Search (⌘K)

One box for apps, Discover entries, containers, drives and settings. Typo-tolerant and instant.

## Install on your phone

hearth is a PWA. Open it over HTTPS (Tailscale), then **Add to Home Screen** (iOS Share menu, or the
install prompt on Android/desktop Chrome). It opens full-screen with shortcuts to Apps, Containers
and Backups.
