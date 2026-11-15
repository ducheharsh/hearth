# Repository standards and patterns

How code in this repo is written. Follow these for every change; reviewers (human or AI) check
against them. When a rule needs to change, change it here in the same commit.

---

## 1. Principles

1. **8 GB is the budget.** Every feature must be cheap at rest. Prefer in-process (Tantivy, not a
   search cluster), containers that idle small, and polling intervals measured in seconds, not
   milliseconds. Large work (backups, pulls) runs in the background.
2. **Safe by default.** Everything published goes through Authelia; hearthd listens on loopback; data
   is never deleted; risky operations (updates) are reversible (rollback); destructive ones confirm.
3. **One way to do each thing.** Install from the Store and install from a pack use the same
   `deploy()`. Every user-visible happening is one `st.event(..)`. Every dialog uses `DialogFrame`.
4. **Plain language.** The UI talks to someone who isn't a sysadmin: "Your photos are backed up",
   not "restic snapshot 3f2a created".
5. **Verify for real.** Features that touch Docker, drives or networks are exercised against real
   containers on a throwaway state dir before they're committed, and the commit says what was verified.

---

## 2. Repository layout

```
crates/
  hearth-core/      pure domain logic (manifests, capacity, packs, backups paths, storage roles)
  hearth-gateway/   pure rendering (Caddyfile, Authelia config, gateway compose)
  hearth-host/      all host I/O (Docker, compose, tailscale, netbird, drives, Ollama)
  hearth-search/    Tantivy index
  hearthd/          daemon: config, state, API, background tasks, embedded UI
web/
  packages/ui/      @hearth/ui design system
  apps/dashboard/   the dashboard (feature folders under src/features/)
catalog/            app manifests, icons/, packs/
docs/               this documentation
```

---

## 3. Rust

### 3.1 Crate boundaries (hard rule)

| Crate | May use | Must not |
|---|---|---|
| `hearth-core`, `hearth-gateway` | serde, thiserror, pure std | tokio, `std::process`, network, filesystem writes |
| `hearth-host` | tokio, bollard, reqwest, `crate::run` for commands | axum, `anyhow`, knowledge of HTTP routes |
| `hearthd` | everything, `anyhow` | business rules that could be pure (move them to core) |

If logic can be tested without Docker, it belongs in `hearth-core` with unit tests
(see `pack::plan`, `backup::paths_for`, `capacity::evaluate`).

### 3.2 Errors

- Libraries return `thiserror` enums with messages a user can act on:
  `#[error("Homebrew is needed to install Ollama. Install it from brew.sh …")]`.
- `hearthd` uses `anyhow::Result` with `.context("downloading the new version")`; API handlers return
  `Result<Json<_>, ApiError>`; use `bad(..)` for 400, `ApiError::NotFound`, `ApiError::Conflict(json)`,
  and `internal` for 500.
- Never `unwrap()` outside tests (`clippy::unwrap_used`). Use `?`, `unwrap_or_default`, or `expect`
  with a reason only where failure is impossible.
- Ignored results are explicit: `let _ = save(st).await;` only when failure is genuinely harmless.

### 3.3 Async and concurrency

- Blocking work (`sysinfo`, big `std::fs` reads) goes through `spawn_blocking` or tokio equivalents.
- Shared state lives on `AppState` behind `RwLock`; take the lock, copy what you need, **drop it
  before awaiting I/O**:
  ```rust
  let settings = st.alerts.read().await.clone();   // lock released here
  send(build(&settings, …)).await;
  ```
- Anything that runs `docker compose` for an app holds `st.ops.lock()` for the duration.
- Long operations: the handler validates, spawns, returns `{"started": true}`. Progress lives in
  state (`running`, job steps); the outcome is an event.

### 3.4 Background tasks

A task is `pub async fn run_<name>(st: SharedState)` with a `tokio::time::interval` loop, spawned in
`main.rs`. It must tolerate Docker/Tailscale being absent (skip the tick, don't exit) and must not
hold locks across sleeps.

### 3.5 Persistence

- JSON files under `state_dir` via `store::load` / `store::save` (atomic temp + rename, mode 0600).
- Secrets go in `config.secrets_dir()`, never in `settings.json`, and are **write-only** through the
  API (return `has_token: true`, not the token).
- Structs use `#[serde(default)]` so older files keep loading; new fields need sensible defaults.

### 3.6 Events

```rust
st.event(Level::Error, "updates", format!("{} rolled back", app.name), message).await;
```

- `source` is a short static area name (`apps`, `backups`, `updates`, `monitor`, `security`, `packs`).
- Title is a sentence fragment a person understands; message says what to do next if anything.
- Every event can become a push notification, so don't emit noise in loops; de-dupe at the source.

### 3.7 Style and lints

- Edition 2024, `cargo fmt` (rustfmt.toml), clippy `all` + `pedantic` as errors in CI.
  Allow a lint locally only with a reason: `#[allow(clippy::too_many_lines)] // one loop, three watchers`.
- `unsafe` is forbidden workspace-wide.
- Module docs (`//!`) explain *why* the module exists and its key decisions; item docs explain
  non-obvious behaviour. Don't narrate the code.
- New dependencies go in `[workspace.dependencies]` and are referenced with `.workspace = true`.
  Prefer `default-features = false` and only the features you use.

### 3.8 Tests

- Pure logic: unit tests next to the code (`#[cfg(test)] mod tests` at the end of the file).
- Parsers of external output (Authelia CLI, restic JSON, Ollama stream) get a test with a real
  captured sample.
- Data shipped in the repo gets a validity test (`every_shipped_pack_is_valid`).
- I/O paths are verified manually against a test daemon (see §7) and noted in the commit message.

---

## 4. HTTP API

- One module per area in `crates/hearthd/src/api/`, each with `pub fn routes() -> Router<SharedState>`,
  merged in `api/mod.rs`.
- REST-ish paths: nouns, ids in the path, verbs only for actions (`/apps/{id}/install`,
  `/updates/{id}/unpin`).
- Request bodies are small `#[derive(Deserialize)]` structs; optional bodies use
  `Option<Json<T>>` + `#[serde(default)]`.
- Validate early with user-facing messages (`bad("The webhook URL must start with http(s)://")`).
- Document new routes in [api.md](api.md).

---

## 5. Frontend

### 5.1 Stack and structure

React 19 · Vite · Tailwind v4 · [Kumo](https://github.com/cloudflare/kumo) · Geist · Motion ·
TanStack Query · Phosphor icons. No other component library.

```
apps/dashboard/src/
  api/client.ts      api / post / put / del (fetch wrappers, error → Error(message))
  api/hooks.ts       every server call as a TanStack Query hook
  api/types.ts       response types shared across features
  features/<area>/   one folder per area: settings/, marketplace/, packs/, backups/, containers/ …
  components/        app-wide pieces (command palette, header)
```

### 5.2 Server state and view state

- All server data comes from hooks in `api/hooks.ts`:
  ```ts
  export const useUpdates = () => useQuery({ queryKey: ["updates"], queryFn: () => api<UpdatesOverview>("/updates"), refetchInterval: 10_000 });
  export const useUpdateApp = () => useAction((id: string) => post(`/updates/${id}`), upKeys, "update-app");
  ```
  `useAction(fn, keysToInvalidate, mutationKey)` invalidates those queries when the mutation settles;
  show `mutation.error.message` next to the control that triggered it.
- Poll faster only while something is running (`refetchInterval: (q) => running ? 1500 : 30_000`).
- View state (tab, section, open app, filters) lives in the **URL** search params so it survives
  reloads and can be linked: `?tab=settings&section=alerts`.
- Local form state is plain `useState`, compared to server data for `dirty`, saved with a `SaveBar`.

### 5.3 Components

Use what exists before writing new UI:

| Need | Use |
|---|---|
| Buttons, badges, switches, banners, dialogs, toasts, loaders, clipboard | Kumo granular imports: `@cloudflare/kumo/components/<name>` |
| Text inputs, selects, searchable selects, fields | `@hearth/ui/components/form` (`TextInput`, `Select`, `SearchSelect`, `Field`, `TextArea`) |
| Settings pages | `SettingsLayout` (left tree), `SettingsHeader`, `SettingsGroup`, `SettingRow`, `SaveBar` |
| Any dialog | Kumo `Dialog` + `DIALOG_FRAME` + `DialogFrame` (fixed header/footer, scrolling body) |
| Cards and sections | `Card`, `InteractiveCard`, `Section` |
| App logos | `AppLogo` (real logo, category fallback) |
| Numbers and charts | `AnimatedNumber`, `RadialGauge`, `Sparkline`, `StatTile`, `UsageBar` |
| Formatting | `@hearth/ui/format` (`formatBytes`, `formatCompact`, `formatRelative` …) |
| Motion | `@hearth/ui/motion` (`springSnappy`, `springSoft`, `riseIn`, `pressScale` …) |

**No browser-default controls.** No native `<select>`, checkbox, radio, `<details>`/`<summary>`,
`type="search"`, date pickers or `alert/confirm/prompt`. Use Kumo/`@hearth/ui` equivalents
(confirmation = a dialog).

### 5.4 Styling

- Semantic `kumo-*` tokens only (`text-kumo-subtle`, `bg-kumo-base`, `ring-kumo-hairline`); no raw
  colours, no `dark:` variants: tokens handle both themes.
- Every grid has a base column count (`grid grid-cols-1 sm:grid-cols-2`) and min-width-safe tracks
  (`grid-cols-[minmax(0,1fr)]`), so nothing overflows at 390 px.
- Text that can be long: `truncate` in single-line rows, `text-pretty` in descriptions, `min-w-0` on
  flex children.
- Numbers that change use `tabular-nums`.

### 5.5 Accessibility

- Every interactive element is a `button` or link with a visible label or `aria-label`.
- Icons are `aria-hidden="true"`; status is never colour-only (dot + text).
- Live progress regions use `aria-live="polite"`; errors use `role="alert"`.
- Inputs have labels (`label htmlFor` or `aria-label`), correct `inputMode`/`autoComplete`
  (`one-time-code` for 2FA), and `spellCheck={false}` for URLs, tokens and IDs.
- Motion respects reduced-motion (shared motion tokens already do).

### 5.6 Copy

- **Buttons and titles: Title Case** ("Set Up 2FA", "Release Hold", "Install Pack").
- Descriptions: sentence case, short, second person, what it does for you.
- Typographic punctuation: curly quotes (`’`, `“ ”`), real ellipsis `…` for placeholders and
  in-progress states ("Generating a secret…").
- Errors say what happened and what to do: "Attach a Backups drive in Storage (or choose S3/B2)
  before backing up."
- No jargon in primary text (snapshots, digests, compose) unless the user needs it; put details in
  secondary text.

### 5.7 Performance

Follow the installed skills `react-best-practices` and `composition-patterns`: lazy-load heavy
features (terminal, charts), avoid prop-drilled booleans (compose components), derive instead of
duplicating state, and keep lists keyed by stable ids.

---

## 6. Catalog

See [catalog.md](catalog.md). In short: real logo, maintained official image, `protect: true`,
measured RAM, unique port, quoted strings in pack flow maps, verified install.

---

## 7. Verification

Before every commit:

```bash
cargo fmt --all
cargo clippy -q --all-targets --all-features -- -D warnings
cargo test -q --workspace
pnpm -C web -r typecheck
pnpm -C web --filter @hearth/dashboard build
```

For features with side effects, run a **test daemon** against a throwaway state dir:

```bash
./target/debug/hearthd -c /tmp/hearth-test.toml   # listen 127.0.0.1:9123, state_dir /tmp/hearth-state
curl -s localhost:9123/api/<route> | jq
```

Exercise the happy path *and* the failure path with real containers (e.g. a healthy update and a
crashing one that must roll back), then remove what you installed.

For UI changes, check 390 px and 1280 px for horizontal overflow and look at a screenshot
(headless Chrome via CDP; any element wider than the viewport outside a scroll container fails).

For gateway changes, validate the rendered Caddyfile:
`docker run --rm -e TS_HOST=x.ts.net -v $PWD/Caddyfile:/etc/caddy/Caddyfile:ro caddy:2 caddy validate --config /etc/caddy/Caddyfile`

---

## 8. Git

- `main` is always releasable. Work happens on `feature/<topic>` branches.
- **One feature per commit**, each passing §7 on its own.
- Commit messages: imperative summary under ~70 characters, blank line, then *why* and *what was
  verified* in prose. Example:

  ```
  App updates with health checks and automatic rollback

  Check each service's registry digest against the local image (no
  download) every 6 hours. Updating takes a pre-update backup, …

  Verified both paths with real containers: a healthy update succeeded
  and a crashing one rolled back and pinned.
  ```
- Commits are authored by the maintainer's GitHub identity. No AI co-author trailers or AI-tool emails.
- Never commit secrets, `target/`, `node_modules/`, `dist/` or state directories.

---

## 9. Documentation

Docs are part of the change, not a follow-up:

| You changed | Update |
|---|---|
| A user-visible feature | [user-guide.md](user-guide.md), README feature list |
| An API route | [api.md](api.md) |
| A background task, crate or data flow | [architecture.md](architecture.md) |
| A config key, state file or recovery step | [operations.md](operations.md) |
| Manifest/pack format | [catalog.md](catalog.md) |
| A rule in this file | this file |
| Roadmap progress | [PLAN.md](PLAN.md) checkboxes |
