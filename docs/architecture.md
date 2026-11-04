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
      DailyWallpaperWorker (Kotlin, WorkManager)
        → System.loadLibrary(...) → JNI function in the same .so → crates/core
        → Canvas: place caption → WallpaperManager.setBitmap(FLAG_SYSTEM|FLAG_LOCK)
```

The daily wallpaper change **never starts a WebView**. Tauri already builds `src-tauri` as a `cdylib`; we export a
plain `extern "system"` JNI function from it that does not touch the Tauri runtime, and the WorkManager job calls it.

Split of responsibilities when setting a wallpaper:

- **Rust**: network, candidate selection, scoring, crop plan, writing the final JPEG into the cache, metadata
  (place, author, license).
- **Kotlin**: decode JPEG → draw caption → `setBitmap`. The caption is drawn in Kotlin on purpose: Android's system
  fonts give free CJK fallback, so we don't ship Noto CJK fonts and their licenses.

## Core crate (`crates/core`, package `wallpaper-core`)

| Module | Responsibility |
|---|---|
| `place` | Wikidata search (`wbsearchentities`) and entity lookup → `Place { qid, label, coords, kind, commons_category }` |
| `source` | `Source` trait; `CommonsSource` (default, no key) and `UnsplashSource` (opt-in, user key) |
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
