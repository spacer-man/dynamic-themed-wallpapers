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

## Q18. `pin.it` short links

**Resolved with the owner:** the Share button of the Pinterest app only gives `pin.it` links, so they are followed now
(`HttpClient::redirect_target`, one hop at a time, at most 5) until a board address appears. A link to a single pin is
refused with a message. Board names in Cyrillic (or any script) are accepted encoded or not and saved in one encoded form.

## Q19. A board and the Pinterest source are tied together in the engine

Pinterest serves only boards and a board works only with Pinterest. **Applied (simplest clear UX):** the engine switches the
source. Activating a board (adding or selecting it) sets `Settings.source = pinterest` and remembers the previous source;
activating a place or words (or deleting the board) restores it (default Commons). `save_settings` refuses to move the source
away from what the active subject needs (it keeps the old one), so a stale screen cannot break it. In the UI every other
source shows "does not support this subject" with a line explaining why, and the board form says that adding one switches the
source. Doing it in the engine means the background worker is consistent too.

## Q20. Pinterest sizes come from header reads; what gets dropped

Sizes are read from the first 64 KB of each image (`Range` request; the decoder reads JPEG/PNG headers, not WebP/GIF, so a
`.webp` original falls back to `/1200x/` then `/736x/`, which are tried with the extension from the feed). That is up to 25
small requests per refill (six in parallel, remembered within one refill, repeated on the next refill). Per the agreed rule
anything not covering the screen at its best working size is dropped, which on a phone leaves only large pins. **Recommended:**
if too many pins are dropped in practice, consider allowing the `/1200x/` size to count only when the screen is smaller than it.

## Q21. Pinterest caption, author and licence

Caption on the wallpaper is "Pinterest: <board name>" (no pin title). The author is taken from `dc:creator`/`author`/`creator`
in the feed if present; I do not know that Pinterest's RSS has one, so the credit is usually just "Pinterest" ("Pin by X on
Pinterest" when there is an author). Licence: "No licence stated". The check request is `robots.txt` on `www.pinterest.com`
(no board is known to the source at check time). The pin id, the feed and Pinterest's behaviour towards non-browser clients
(403, rate limits, format changes) are from memory and **not verified against the live service**.

## Q22. Pinterest and the "latest ~25 pins" limit

A board with fewer large pins than this may often fail with "no suitable photo"; pins already shown are not repeated, so a
small board runs dry quickly. The hint in the UI and `docs/sources.md` say the feed holds only the latest pins.

## Q23. When the failure notification is posted, and what it offers

Posted by the worker after the **last** WorkManager retry of a `change` (or at once for a non-retryable failure) when the error
is about the source: network, HTTP status, "no suitable photo", unparsable answer. Not for a missing subject, storage or image
errors, and not for prefetch. Alternatives are the sources with state *available* for the active subject, other than the failed
one: a key source without a key and any source with a failure on record are left out (so after a failure, sources never checked
are offered). At most three. "Retry later" queues one more change in an hour. A notification button only queues work; the
actual change runs in the same worker (`change:<source id>`). Text is English or Russian by the language setting (`auto` uses
the device language); the channel has low importance (no sound); the icon is a system drawable.

## Q24. Notification permission has no "enable schedule" moment

There is no on/off switch for the schedule (it is always on). **Applied:** the permission dialog is requested when the user
changes the schedule time, and the Schedule screen shows a button while notifications are off. The call returns immediately
(the dialog's answer is read again a few seconds later); nothing is requested at app start.

## Q25. Opening the app on the Sources screen

The notification starts `MainActivity` with an extra; `onNewIntent` keeps it, the plugin command `launchRoute` returns it once,
and the UI asks at start and every time the page becomes visible. A tap while the app is already in front may not switch the
screen until the page is visible again.

## Q26. Not verified here (stages 5–6)

`src-tauri` and the Kotlin code were not compiled (no gtk, no Android SDK); `worker.rs` was compiled and its tests run in a
scratch crate. Overridden signatures (`onNewIntent`, `Plugin` commands), the manifest receiver and the R8 keep rule for
`FallbackReceiver` need a real build. The UI was not looked at in a browser. Pinterest and its RSS were never contacted; all
Pinterest behaviour rests on fixtures from the described format. A one-time fallback photo is recorded in the history with its
own source, and the next prefetch goes back to the chosen source.

## Q27. Pinterest boards: widget JSON primary, RSS fallback

Real device, Nov 2026: `pinterest.com/<user>/<board>.rss` and `<user>/feed.rss` answered 404 "Board not found" for a public
board. **Owner's choice:** read the board from the embed-widget endpoint `widgets.pinterest.com/v3/pidgets/boards/<user>/<board>/pins/`
first and keep the RSS as the fallback; the saved board identity stays the `.rss` address (no migration). Risks: the endpoint
is unofficial and may change; the JSON shape is written from memory and tested only on a fixture (Pinterest was not contacted),
so a different real shape would show up as "no usable pins" and silently use the RSS.

## Q28. Daily change: alarm chain instead of a 24 h periodic job

Real device (Xiaomi), Nov 2026: the daily change did not happen and nothing was shown. Cause: `schedule()` ran at every launch and
settings save with `ExistingPeriodicWorkPolicy.UPDATE`, which keeps the original enqueue time but applies the new initial delay, so
the first run drifted by hours; periodic work is also very inexact in Doze. **Applied:** `setAndAllowWhileIdle` alarm (no
exact-alarm permission, so it can be a few minutes to hours late in deep Doze, which is acceptable for a wallpaper) → `DailyAlarmReceiver`
→ expedited one-time work (falls back to ordinary work when the quota is used up; Android 11 and older show a low-importance
"Changing the wallpaper" notification while it runs) → re-arm. The daily change no longer requires a healthy battery (prefetch still
does). `BootReceiver` (the only exported receiver, system broadcasts only) re-arms after reboot, update and clock changes.
The old `daily-wallpaper` periodic work is cancelled once. Diagnostics (last attempt, message, next alarm) are shown on the
Schedule screen, with a "Battery settings" button (MIUI autostart list on Xiaomi, else the app details page) because some OEMs
kill alarms and background work of apps without autostart. Not verified: nothing here was compiled or run on a device. A planned
time more than 10 minutes in the past is shown as "the phone did not start it". The outcome "skipped, no network" is shown as
"waiting to start" (the work simply waits for the network constraint). Relies on work-runtime merging the FOREGROUND_SERVICE
permission into the manifest (needed by expedited work on Android 11 and older).
