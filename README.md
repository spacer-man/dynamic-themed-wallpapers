# dynamic-themed-wallpapers

An Android app that sets one beautiful photo of a place you choose as your wallpaper every day,
with a small caption saying what the place is.

- **Any place**: a country, a city, a district, a mountain, a temple — found via Wikidata.
- **Tone-aware**: ask for dark photos only (or light, or follow the system theme) and you will never get the other kind.
- **Picks beautiful photos, not random ones**: curated sets first (Wikimedia Commons Featured / Quality / Valued images),
  then technical filters (sharpness, clipping, resolution, crop loss), then a small on-device model that learns from your likes and skips.
- **Battery-friendly**: the daily change runs in a WorkManager job that calls straight into Rust over JNI — no WebView,
  no foreground service, no polling. Photos are prefetched while on Wi-Fi and charging.

## Privacy

**No telemetry. No analytics. No accounts. No crash reporters.**

The app only talks to `commons.wikimedia.org`, `www.wikidata.org`, `query.wikidata.org` and — only if you turn it on and
supply your own key — `api.unsplash.com`. Everything it learns about your taste stays on the device.
See [docs/privacy.md](docs/privacy.md).

## Attribution

Most photos are licensed CC BY / CC BY-SA. The app always shows the author and license of the current photo — that is
a license requirement, not a courtesy.

## Project layout

```
crates/core   pure-Rust engine: place search, photo sources, scoring, cropping, storage
src           Svelte 5 UI
src-tauri     Tauri v2 shell + JNI entry point used by the background worker
docs          architecture, scoring, sources, privacy, open questions
```

## Trying it on a desktop

No Android SDK needed.

**The engine** (place search, photo selection, scoring):

```sh
cargo test --workspace
cargo run -p wallpaper-core --example pick -- --place "Kyoto" --tone dark --screen 1080x2400 --explain
```

The example talks to the real Wikidata/Commons APIs, prints every candidate with its measurements and verdict, and
writes the chosen photo (already cropped to the screen) into `out/`.

**The UI** (Svelte 5), in a browser with an in-memory mock backend:

```sh
bun install
bun run dev            # http://localhost:1420, sample scenes instead of real photos
bun test               # unit tests of the UI logic
bun run check          # svelte-check
```

**The desktop app** (UI + real engine): `bun run tauri dev`. On Linux this needs the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

Build notes: the default TLS backend is `ring` (needs a C compiler). Without one, build with
`--no-default-features --features tls-graviola` (pure Rust). `wallpaper-core` links the system SQLite on desktop;
Android builds must enable the `sqlite-bundled` feature.

## Status

Early development. Android only — iOS has no public API for changing the wallpaper.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work
by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
