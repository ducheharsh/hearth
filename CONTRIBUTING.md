# Contributing

1. Read [docs/architecture.md](docs/architecture.md) and [docs/STANDARDS.md](docs/STANDARDS.md).
2. Branch from `main`: `git switch -c feature/<topic>`.
3. Make one feature per commit. Each commit passes:
   ```bash
   cargo fmt --all
   cargo clippy -q --all-targets --all-features -- -D warnings
   cargo test -q --workspace
   pnpm -C web -r typecheck && pnpm -C web --filter @hearth/dashboard build
   ```
4. Exercise side effects on a test daemon with a throwaway `state_dir` (STANDARDS §7) and say what you
   verified in the commit message.
5. Update the docs your change touches (STANDARDS §9) in the same commit.

Adding an app or pack? See [docs/catalog.md](docs/catalog.md).
