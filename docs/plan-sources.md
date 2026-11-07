# Plan: more photo sources, query mode, history → wallpaper, release crash

Agreed with the owner on 2026-11-06. Work goes to branch `claude/photo-sources-plan-xoy6sd`.
Decisions taken without the owner go to `docs/OPEN-QUESTIONS.md`.

## Decisions

| Topic | Decision |
|---|---|
| Pinterest | Public boards via RSS (`pinterest.com/<user>/<board>.rss`), no key, no login. No query search on Pinterest. |
| Low resolution | Always request the original (`i.pinimg.com/originals/…`, or the largest size a source offers); drop anything smaller than the screen. No upscaling. |
| New sources | Openverse (no key), Wallhaven (no key, SFW only), Pexels (user's key), Pixabay (user's key). |
| Keys | Entered by the user in settings, like Unsplash. Nothing secret ships in the APK. |
| What to search | A mode chosen in settings: **Place** (as today), **Query** (free text, e.g. "foggy forest"), **Pinterest board**. Several can be saved and switched. |
| Combining sources | The user picks the source. Settings show each source's availability (reachable / no key / error). If a background refresh fails, a notification offers another source for this one time (temporary fallback until the main one is back). |

## Stages (each one: code + tests + docs, own commit)

1. **Release crash.** Release APK closes right after launch. Likely cause: `optimization { enable = true }` in the
   release build type with an empty `proguard-rules.pro`, so R8 strips or renames Kotlin classes that Rust reaches
   through JNI (wallpaper plugin, worker). Confirm, add keep rules (or turn off minify), and check the release APK
   build in CI.
2. **History → set as wallpaper.** A "Set as wallpaper" action in the History photo details; reuses the existing
   apply path (download in the right size, caption, Unsplash download ping).
3. **Source framework.** Common source trait for query-based sources, per-source status, source picker and key fields
   in settings, query mode next to place mode. Commons keeps its curated place search.
4. **New sources.** Openverse, Wallhaven, Pexels, Pixabay — with licence/author captions and minimum-size filter.
5. **Pinterest boards.** Board URL in settings → RSS → original image URLs → size filter. *(done, see OPEN-QUESTIONS Q18–Q22)*
6. **Fallback notification.** Background refresh failure → notification with a choice of another available source. *(done, see Q23–Q26)*
7. **Docs.** `docs/sources.md`, READMEs (en/ru), privacy notes for new hosts.

## How the work runs

The main session plans and reviews; implementation runs in subagents on Sonnet to save limits and keep context clean.

## Next: Pinterest sign-in (official API v5)

Board feeds without sign-in (RSS, embed widget) only see public profiles; the owner keeps a private profile.
Agreed on 2026-11-07:

- **Two board modes:** "My boards" through sign-in to Pinterest (pick from the list, private profile and secret boards
  included) and "Board by link" (today's widget/RSS path, public profiles only, also other people's boards).
- **Owner's own Pinterest app.** The client id and secret are entered in settings by the user, never shipped in the APK.
- **Redirect URI:** `dtw://pinterest-auth` (custom scheme opens the app). If Pinterest refuses custom schemes, fall back to
  an https page on GitHub Pages that forwards to the app.
- **Scopes:** `user_accounts:read`, `boards:read`, `pins:read`, `boards:read_secret`, `pins:read_secret`.
- **Order:** the owner registers the app first; the code is written once trial access is approved, so it can be tested
  against the live API right away. Token lifetimes and image sizes are taken from the real responses, not from memory.
