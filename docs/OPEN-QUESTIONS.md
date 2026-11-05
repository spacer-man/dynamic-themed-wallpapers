# Open questions

Decisions made without the owner during autonomous work. Each one has a recommended option that was
**already applied**; the code is marked `TODO(open-question): Qn` where relevant. Agree, or say which to redo.

## Q1. Git author e-mail in commit history

Commits are created with the global git identity of the machine, which includes a personal e-mail.
That is commit metadata, not code, but it becomes public on push.

**Resolved:** before the first public push the history was rewritten so every commit uses the GitHub noreply
address (`<id>+<login>@users.noreply.github.com`).

## Q2. Crate name `wallpaper-core` instead of `core`

The plan says `cargo run -p core`. A crate named `core` collides with Rust's built-in `core` crate and breaks
`use core::...` everywhere. The directory stays `crates/core`, the package is named `wallpaper-core`
(`cargo run -p wallpaper-core --example pick ...`).

## Q3. Repository URL in the User-Agent

Wikimedia asks API clients to send a User-Agent with contact info. **Resolved:** it points to
`https://github.com/spacer-man/dynamic-themed-wallpapers` (also in `Cargo.toml` `repository`).

## Q4. The sandbox cannot compile C code → pure-Rust choices tonight

nono does not allow reading `/usr/include`, so any crate with a C part fails to build here
(`ring` for TLS, `rusqlite`'s bundled SQLite). Also `~/.cargo` is read-only, so cargo runs with
`CARGO_HOME=$PWD/.cargo-home` (git-ignored).

- TLS: default feature `tls-ring` (standard, builds fine with the Android NDK). Here I build with
  `--no-default-features --features tls-graviola` (pure-Rust provider by the rustls author).
  **Recommended:** keep as is. Optional fix for the sandbox: `nono run --allow /usr/include -- claude`.
- Storage (stage 5): see Q5.

## Q6. Strict tone filters need big batches

Real runs show most daytime photos are "mid" tone, so `dark`/`light` leave few candidates per refill
(2 of 18 for dark Kyoto). Currently: widen by paging/seed and fetch more. Alternatives:

- **A (current, recommended):** keep absolute thresholds, fetch more candidates (prefetch makes this cheap).
- B: relative filter (darkest third of the batch) — always finds something, but "dark" may then be mid-grey.
- C: loosen thresholds (median ≤ 0.35) — more candidates, slightly less dark.

## Q5. SQLite: system library here, bundled for Android

Stage 5 uses `rusqlite` as planned. `rusqlite/bundled` compiles SQLite from C, which this sandbox cannot do
(see Q4), so desktop builds link the system `libsqlite3` and the Android build must enable the
`sqlite-bundled` feature of `wallpaper-core` (there is no system SQLite to link on Android).
**Recommended:** keep. Nothing to decide unless you want a pure-Rust store (`redb`) instead — then stage 5 is redone.

## Q7. External links need an opener plugin

The details sheet shows the Commons/Unsplash page and renders it as a plain `<a target="_blank">`. In a Tauri
webview this does nothing without `tauri-plugin-opener`. **Recommended:** add the plugin (and its capability for
https URLs on the three hosts only) when the Android plugin work starts; until then the URL is selectable text.

## Q8. Unsplash key has no UI yet

The engine supports Unsplash (`Engine::set_unsplash_key`, `Settings::unsplash_enabled`) but the key is held in
memory only and there is no input field. Stage 7 (Android) will store it in EncryptedSharedPreferences as planned;
a desktop UI field would need an OS keyring (e.g. `keyring` crate), which pulls in D-Bus/C libraries.
**Recommended:** Android only for the key; on desktop Unsplash stays off.

## Q9. App identifier

**Resolved:** `io.github.spacerman.dynamicthemedwallpapers` (in `src-tauri/tauri.conf.json`, the Kotlin packages and the JNI
symbol name in `src-tauri/src/worker.rs`). It is the Android
package name and must not change once releases are out.

## Q10. TypeScript pinned to 6

`svelte-check` 4.x does not work with TypeScript 7 yet (it asks for a `--tsgo` setup), so `typescript` is pinned to
`~6` in `package.json`. Revisit when svelte-check supports TS 7 directly.

## Q11. No BootReceiver; background job not yet verified on a device

The plan lists a `BootReceiver`. WorkManager already persists periodic work across reboots and app
updates, so a receiver would only duplicate that and add a `RECEIVE_BOOT_COMPLETED` permission.
**Recommended (applied):** no receiver; the daily and prefetch jobs are (re)scheduled from `save_settings`
and at app start.

Not yet verified (needs a Gradle build and a device): that the Kotlin compiles, that `Scheduler.schedule`
works when called from the Rust `setup` hook before the activity is up (errors there are ignored; the
next `save_settings` schedules it), and that the worker never starts a WebView (check the process log).

The Unsplash key is not passed to the worker yet (see Q8), so background changes use Commons only.
