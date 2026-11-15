# Operations

Running hearth on the Mac, where things live, and what to do when something breaks.

## Requirements

- macOS on Apple silicon (Linux support is planned), 8 GB RAM is the design target.
- [OrbStack](https://orbstack.dev) (or Docker Desktop) for containers.
- [Tailscale](https://tailscale.com) for remote access (or NetBird / your own Headscale).
- Homebrew, if you want packs with native apps (Ollama).
- An external SSD for the Vault / Media / Backups roles (recommended).

## Build and run

```bash
brew install rustup pnpm && rustup default stable
pnpm -C web install && pnpm -C web build      # dashboard → web/apps/dashboard/dist
cargo build --release -p hearthd               # embeds the dashboard
cp hearth.example.toml hearth.toml
./target/release/hearthd --config hearth.toml
```

Open `http://127.0.0.1:9000`. From another device before the gateway is on:
`ssh -L 9000:localhost:9000 <mac>`. Then in Settings: connect Tailscale, add a login, turn on the
gateway, and the dashboard and apps are reachable over HTTPS on your tailnet.

Running it at login with launchd is on the roadmap (`hearth install-service`); for now use a
LaunchAgent that runs the command above with `KeepAlive`.

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
├── gateway/             Caddyfile, compose, Authelia config, users.json, authelia.env, DB
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

### Locked out of 2FA
On the Mac: Settings → Logins → **Remove** 2FA for that login, or from a shell:
`docker compose -p hearth-gateway exec authelia authelia storage user totp delete <user> --config /config/configuration.yml`
(add `--encryption-key` from `gateway/authelia.env`).

### Logs
hearthd logs to stderr (`RUST_LOG=hearthd=debug` for more). App logs are in Containers, or
`docker compose -p hearth-<id> logs`.
