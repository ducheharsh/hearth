# User guide

What every part of hearth does and how to use it. For setting the machine up, see
[operations.md](operations.md).

## Overview

The home screen. At the top is **your server's address**: the link to open hearth from your other
devices (your Tailscale name on port 9443 once the gateway is on), with copy, **Open** and
**Scan on Phone** (a QR code). Other ways in are listed under it (your domain, NetBird, this Mac).
Until the gateway is on, only the local address works, and the bar offers **Set Up Access**.

Below that, a health hero lists anything that needs attention (setup steps, apps that were
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
- **Network**: Tailscale (connect, rename, HTTPS, published ports, custom control server),
  NetBird (connect, serve apps with hearth's certificate authority), Headscale (install wizard, users,
  keys, join this Mac).
- **Logins & Gateway**: turn the gateway on, add logins, **two-factor** per login, see what's published.
- **Alerts**: notification channels and what to watch.
- **Domains & Tunnels**: your domain via Cloudflare Tunnel (from an API token), Tailscale Funnel, ngrok.
- **Capacity**: memory reserve and disk reserve used by install checks.
- **Advanced**: require the gateway login for the dashboard itself.

### Two-factor

Logins & Gateway → **Set Up 2FA** on a login. Scan the QR code with any authenticator app
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
