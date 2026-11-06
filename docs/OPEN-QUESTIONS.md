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

## Q12. Single source per user; Unsplash "on/off" became "choose a source"

The old `unsplash_enabled` mixed Unsplash into Commons. The plan says the user picks the source and nothing is mixed,
so `Settings.source` replaces it. **Applied:** old settings with `unsplash_enabled: true` migrate to `source: unsplash`
(which shows "needs key" until a key is entered), anything else to Commons. Changing the source drops the prefetched queue.

## Q13. Where API keys live

Q8 left the key UI open. **Applied:** keys are entered in settings and stored in the app's private SQLite database
(`source_keys`), so the background worker can use them. They are never in `Settings`, never sent to the UI (it only
sees `has_key`) and never logged. This is not Android-Keystore-backed encryption. **Recommended:** keep for now; move
to EncryptedSharedPreferences through a small Kotlin plugin if you want it.

## Q14. Pixabay is mostly useless without full API access

Without approval, Pixabay only gives `largeImageURL` (≤ 1280 px), too small for a phone (plan: no upscaling, drop what
does not cover the screen). **Applied:** honest sizes, so most hits are dropped. **Recommended:** keep, and say so in the
UI text; or apply for full access.

## Q15. Wallhaven has no author or licence in search results

**Applied:** caption "Wallpaper: Wallhaven #<id>", licence "No licence stated". The per-wallpaper endpoint has the
uploader but costs one request per photo. Wallpapers there are often third-party art; General + SFW only limits that.
**Recommended:** keep, or drop Wallhaven if unlicensed content is a concern.

## Q16. Openverse licence filter and thumbnail size

**Applied:** `license_type=modification` (no ND, NC allowed) and `extension=jpg` (the decoder handles only JPEG/PNG).
Openverse thumbnails are about 600 px, below the ~1000 px preview used elsewhere. Pexels `?w=1000`, Wallhaven
`thumbs.original` and Pixabay `largeImageURL` previews, and Pixabay full-size URLs, are from memory of the docs.

## Q17. Not verified here

`src-tauri` is not compiled here (gtk missing), the UI was not looked at in a browser (none installed), and no live API
was called: all source behaviour rests on recorded-style fixtures written from the API docs as remembered.
