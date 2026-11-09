# Architecture

## Big picture

```
┌─ UI (Svelte 5 + Vite + TS) ──── only while the user has the app open
│
├─ src-tauri (Tauri v2 app) ───── commands: settings, preview, like/skip, "change now"
│
├─ crates/core (pure Rust, no Tauri) ──┐  THE single home of the logic:
│     place  source/  image/  score/   │  pick → score → crop → cache
│     store/                           │
│                                      │
└─ Android (gen/android) ──────────────┘
      DailyAlarmReceiver (AlarmManager) → DailyWallpaperWorker (Kotlin, WorkManager)
        → System.loadLibrary(...) → JNI function in the same .so → crates/core
        → Canvas: place caption → WallpaperManager.setBitmap(FLAG_SYSTEM|FLAG_LOCK)
```

The daily wallpaper change **never starts a WebView**. Tauri already builds `src-tauri` as a `cdylib`; we export a
plain `extern "system"` JNI function from it that does not touch the Tauri runtime, and the WorkManager job calls it.

### Scheduling (Android)

The daily change is a self-rescheduling chain, not a periodic job: `Scheduler.schedule` stores the time and the Wi-Fi rule in
SharedPreferences and arms `AlarmManager.setAndAllowWhileIdle(RTC_WAKEUP)` for the next local HH:MM. The alarm starts
`DailyAlarmReceiver`, which arms the next alarm first and then enqueues an expedited one-time `DailyWallpaperWorker` (network
constraint only; `UNMETERED` with Wi-Fi only). The worker re-arms after its final result. `BootReceiver` re-arms after reboot,
app update and clock / time zone changes (and runs a missed change once after a reboot). `schedule` is idempotent: the same
time with an alarm still pending changes nothing, so app starts and settings saves never move the next run. The prefetch job
stays periodic (`KEEP`). The last daily attempt (queued / changed / retry / failed + message) and the next planned time are kept in
SharedPreferences and shown on the Schedule screen (`scheduleStatus` plugin command). See OPEN-QUESTIONS Q28.

Split of responsibilities when setting a wallpaper:

- **Rust**: network, candidate selection, scoring, crop plan, writing the final JPEG into the cache, metadata
  (place, author, license).
- **Kotlin**: decode JPEG → draw caption → `setBitmap`. The caption is drawn in Kotlin on purpose: Android's system
  fonts give free CJK fallback, so we don't ship Noto CJK fonts and their licenses.

## Core crate (`crates/core`, package `wallpaper-core`)

| Module | Responsibility |
|---|---|
| `place` | Wikidata search (`wbsearchentities`) and entity lookup → `Place { qid, label, coords, kind, commons_category }` |
| `source` | `Source` trait; Commons (default), Openverse, Wallhaven, Pinterest boards (no key), Unsplash, Pexels, Pixabay (user key) |
| `subject` | what to show: Place, free-text Query, or Pinterest board; one active, one source for it |
| `http` | tiny blocking HTTP abstraction so everything above can be tested with fixtures, no network in tests |
| `image` | tone, sharpness, colorfulness, palette (k-means), dHash, crop plan |
| `score` | feature vector + linear/logistic model with online SGD |
| `store` | SQLite: history, feedback, model weights, cache index with LRU budget |
| `pipeline` | glue: place → candidates → download → analyse → filter → rank → render |

Network code is **blocking** (`ureq`), deliberately: it runs once a day inside a background job or behind a Tauri
command on a worker thread. An async runtime would add binary size and complexity for nothing.

## Decisions log

- 2026-11-05: package name `wallpaper-core` (see OPEN-QUESTIONS Q2).
- 2026-11-05: blocking HTTP via `ureq` behind a `HttpClient` trait; fixtures in tests.
- 2026-11-05: places resolved without SPARQL (WDQS was rate-limited to 1 req/min): `wbsearchentities`, `wbgetclaims`, and `haswbstatement:` CirrusSearch for sub-places.
- 2026-11-05: TLS provider is a feature: `tls-ring` (default) or `tls-graviola` (pure Rust), see OPEN-QUESTIONS Q4.
- 2026-11-05: storage is SQLite via `rusqlite` (WAL, append-only migrations keyed by `user_version`). Desktop links system libsqlite3; Android enables `sqlite-bundled`. Rendered wallpapers live as files in a cache directory indexed in SQLite: `ready` (prefetched) → `used`, LRU eviction that never touches the current wallpaper. Photos are never stored in the DB.

- 2026-11-05: `service::Engine` is the single entry point for everything the app does; Tauri commands (and later the JNI worker) are thin wrappers. It holds only configuration and opens a short-lived SQLite connection per call.
- 2026-11-05: prefetched wallpapers carry their tone (store migration v2) and the tone filter is applied again when one is taken, so changing the filter or the system theme never hands out the wrong tone.
- 2026-11-05: UI talks to the backend only via `src/lib/api.ts`; in a browser it uses `src/lib/mock.ts`. Theme colours come from `core/theme.rs` (palette → contrast-safe colours); the caption scrim is computed in the UI per photo.
- 2026-11-06: a Pinterest board only works with the Pinterest source (and the reverse); activating a board switches the source, see OPEN-QUESTIONS Q19. Pinterest dimensions come from ranged header reads, Q20.
- 2026-11-06: a failed background change posts a notification (Kotlin `FallbackNotifier` / `FallbackReceiver`). The worker op `change:<source id>` runs `Engine::change_now_with_source`, a one-time override that is never persisted. The worker outcome JSON carries a `fallback` object (failed source, up to three alternatives, language); see OPEN-QUESTIONS Q23.
