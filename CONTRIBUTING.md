# Contributing

Thanks for wanting to help!

## Ground rules

- **No telemetry, analytics or tracking of any kind.** PRs adding them will be closed.
- **No secrets in the repo.** API keys come from user settings, never from code or build files.
- New network endpoints must be added to [docs/privacy.md](docs/privacy.md) in the same PR.
- Photo attribution (author + license) must stay visible wherever a photo is shown.

## Workflow

- Work happens on `dev`; `main` gets releases.
- Commits follow [Conventional Commits](https://www.conventionalcommits.org/) (`feat(core): ...`, `fix(ui): ...`).
- Before sending a PR:

  ```sh
  cargo fmt --all
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  bun run check
  ```

- Tests must not hit the network. Use the fixtures in `crates/core/tests/fixtures`.

## Android

`src-tauri/gen/android` is committed because it contains hand-written code (worker, plugin, manifest changes).
**Never run `tauri android init` on top of it.** To pick up template changes, generate into a scratch directory and port
the diff by hand.

Local changes to generated files that must survive a regeneration: `buildSrc/.../BuildTask.kt` runs the Tauri CLI by
path (`../node_modules/@tauri-apps/cli/tauri.js`) instead of `node tauri`, which only works through a package-manager
runner; `app/build.gradle.kts` has the release signing config read from `DTW_*` environment variables.

Releases: see [docs/releasing.md](docs/releasing.md).
