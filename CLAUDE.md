# hearth: notes for AI agents

Read `docs/STANDARDS.md` (repo standards and patterns) and `docs/architecture.md` first; `docs/PLAN.md` has
decisions and the roadmap. Update its checkboxes when you finish an item, and update the docs your change
touches in the same commit (STANDARDS §9).

## Rules
- Keep crate boundaries: `hearth-core` and `hearth-gateway` stay **pure** (no tokio, no process/network I/O).
  All side effects go in `hearth-host`; wiring/HTTP in `hearthd`.
- Libraries return `thiserror` enums; only `hearthd` uses `anyhow`.
- No `unwrap()` outside tests (clippy `unwrap_used` is on); `unsafe` is forbidden workspace-wide.
- Blocking calls (`sysinfo`, `std::fs` in hot paths) go through `spawn_blocking` or tokio equivalents.
- **Security invariant:** nothing may be published (Caddy port or `tailscale serve`) without passing
  Authelia unless the manifest explicitly sets `auth.protect: false` or lists `exempt_paths`.
  The daemon binds loopback only.
- App data is never deleted by hearth.
- New dependencies go in `[workspace.dependencies]` and are referenced with `.workspace = true`.
- Long operations run in the background and report through `st.event(..)`; `docker compose` runs hold `st.ops`.
- One feature per commit on a `feature/*` branch; verify side effects on a test daemon and say so in the message.

## Frontend (web/)
- UI primitives come from `@hearth/ui` (read `web/packages/ui/README.md`) and Kumo granular imports
  (`@cloudflare/kumo/components/<name>`). Don't add another component library.
- Semantic `kumo-*` tokens only; no raw colors, no `dark:`. Shared motion from `@hearth/ui/motion`.
- Follow the installed skills: `react-best-practices`, `composition-patterns`, `web-design-guidelines`.
- Server state lives in TanStack Query hooks (`apps/dashboard/src/api/hooks.ts`); URL holds view state.
- No browser-default controls (native select/checkbox/radio/details/search inputs, alert/confirm).
- Title Case buttons, curly quotes, `…`; check 390px and 1280px for overflow.

## Before finishing any change
```bash
cargo fmt --all && cargo clippy --all-targets --all-features && cargo test --workspace
```
For UI changes also: `pnpm -C web typecheck && pnpm -C web build`.
All must be clean. For gateway changes, also validate the rendered Caddyfile:
`docker run --rm -e TS_HOST=x.ts.net -v $PWD/Caddyfile:/etc/caddy/Caddyfile:ro caddy:2 caddy validate --config /etc/caddy/Caddyfile`
