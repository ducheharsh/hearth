# Operations

Running hearth on the Mac, where things live, and what to do when something breaks.

## Requirements

- macOS on Apple silicon (Linux support is planned), 8 GB RAM is the design target.
- [OrbStack](https://orbstack.dev) (or Docker Desktop) for containers.
- [Tailscale](https://tailscale.com) for remote access (or NetBird / your own Headscale).
- Homebrew, if you want packs with native apps (Ollama) or the Homebrew install.
- An external SSD for the Vault / Media / Backups roles (recommended).

## Install

| Way | Install | Runs at login via |
|---|---|---|
| **Hearth.app** (DMG from Releases) | Drag to Applications, open it | `hearthd service install` (done by the app on first launch) |
| **Homebrew** | `brew install ducheharsh/tap/hearth` | `brew services start hearth` |
| **Source** | `cargo build --release -p hearthd` (after `pnpm -C web build`) | `./target/release/hearthd service install` |

Use one login service, not two: the app and `hearthd service install` refuse to install when the
Homebrew service exists.

The app isn't notarized yet, so macOS 15 blocks the first open ("Apple could not verify 'Hearth'…";
the old right-click → Open shortcut no longer exists). Click **Done**, then System Settings →
Privacy & Security → **Open Anyway** next to "Hearth was blocked". From a terminal:
`xattr -dr com.apple.quarantine /Applications/Hearth.app`. Homebrew's formula isn't affected.

On first run the app installs the login service, waits for hearth to answer and says so (or explains
why it couldn't start, e.g. port 9000 in use). Opening Hearth again goes to the dashboard, which helps
when the menu bar icon is hidden behind the notch.

### The binary

`hearthd` is a single file: the dashboard and the curated catalog (apps, packs, logos) are built in.
With no catalog next to it, it unpacks the built-in one to `<state_dir>/catalog` on every start, so
upgrades bring new apps.

```
hearthd                      run the daemon (default)
hearthd service install      start at login (launchd agent dev.hearth.hearthd, KeepAlive)
hearthd service uninstall    stop doing that (apps and data untouched)
hearthd service status       is it running?
hearthd open                 open the dashboard
hearthd --config <file>      use a specific hearth.toml (default: ./hearth.toml, else ~/.hearth/hearth.toml)
```

Logs: `<state_dir>/logs/hearthd.log` (login service), or `$(brew --prefix)/var/log/hearth.log` (Homebrew).

### Building the app and DMG

```bash
packaging/macos/build.sh      # → dist/Hearth.app, dist/Hearth-<version>.dmg, Homebrew tarball, SHA256SUMS
```

It builds the dashboard and `hearthd`, compiles the Swift menu bar app (`packaging/macos/HearthMenu.swift`),
makes the icon, signs ad-hoc and packs the DMG. The **Release** GitHub workflow runs the same script.

## First run

Open `http://127.0.0.1:9000`. From another device before the gateway is on:
`ssh -L 9000:localhost:9000 <mac>`. Then in Settings: connect Tailscale, add a login, and turn on the
gateway, and the dashboard and apps are reachable over HTTPS on your tailnet.

### Tailnets without HTTPS certificates

Some tailnets (often a work one) can't turn on HTTPS Certificates. In **Settings → Network →
Tailscale → Certificates**, choose **hearth's own certificate** (or click *Use hearth's Certificate*
on the Gateway page). Caddy then serves HTTPS on your Mac's Tailscale IP with hearth's CA, and
`tailscale serve` isn't used. MagicDNS must still be on.

Each device trusts the certificate once. The Overview address bar shows how: scan the QR code on a
phone, or download it from `http://<tailscale-ip>:9444` on any device in the tailnet. The certificate
is named "Caddy Local Authority". Port 9444 is reserved for this.

## Configuration

`hearth.toml` holds only bootstrap settings; everything else is set from the dashboard and stored in
`<state_dir>/settings.json`.

| Key | Default | |
|---|---|---|
| `listen` | `127.0.0.1:9000` | Keep on loopback; the gateway is the way in |
| `catalog_dir` | `catalog` | Curated apps, icons and packs |
| `state_dir` | `~/.hearth` | Everything hearth writes |
| `ts_host` | auto | Override the Tailscale name used in app URLs |

## State directory

```
~/.hearth/
├── settings.json        user settings
├── state.json           adopted containers (routes) and your ratings
├── storage.json         drive attachments and identities
├── history.json         per-app 7-day usage
├── backups.json         backup settings + last results
├── updates.json         update status, auto-update list, history
├── alerts.json          alert settings
├── apps/<id>/           compose.yml, hearth.override.yml, [hearth.pin.yml], .env, data/
├── custom/              your own app definitions
├── gateway/             Caddyfile, compose, Authelia config, users.json, authelia.env, DB,
│                        caddy-data/ (hearth's CA), public/hearth-ca.crt
├── volumes/             role links (vault, media, backups) → the attached drives
├── secrets/             backup.json, alerts.json, cloudflare, ngrok… (all 0600)
├── headscale/           if installed
└── discover/            awesome-selfhosted cache, GitHub cards
```

Back up `~/.hearth/secrets/` and your **backup recovery password** somewhere safe: with them, any
machine can read your restic repository.

## Recovery

### Restore an app from backup
Backups → the app → **Restore** → pick a snapshot. The app is stopped, restored exactly and started.

### Restore onto a new machine
1. Install hearth, attach the Backups drive (or enter the same S3/B2 settings).
2. Put the old `secrets/backup.json` back (it holds the repository password), or set up backups with
   the same destination and enter the recovery password when asked.
3. Install the apps, then restore each from its latest snapshot.

### An update was rolled back
The app keeps running the previous images (`apps/<id>/hearth.pin.yml`). Check its logs in Containers,
then **Release Hold** when a fixed version is out.

### Locked out as a member, or no admin left
hearth refuses changes that would leave no active admin. If `users.json` was edited by hand, set
`"role": "admin"` (and `"disabled": false`) for your login in `~/.hearth/gateway/users.json`, then
restart hearth.

### Locked out of 2FA
On the Mac: Settings → Logins → **Remove** 2FA for that login, or from a shell:
`docker compose -p hearth-gateway exec authelia authelia storage user totp delete <user> --config /config/configuration.yml`
(add `--encryption-key` from `gateway/authelia.env`).

### Logs
hearthd logs to stderr (`RUST_LOG=hearthd=debug` for more). App logs are in Containers, or
`docker compose -p hearth-<id> logs`.
