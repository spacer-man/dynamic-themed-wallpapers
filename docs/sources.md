# Photo sources

A source turns a **subject** — a place or free text — into candidates (`crates/core/src/source/`). It never downloads images and never judges
beauty beyond what humans already curated — that's the job of `image/` and `score/`.

## Choosing and checking sources

The user picks one source in settings; sources are never mixed. A subject is a saved Place, Query (text) or Pinterest board; several
can be saved and one is active. A place is served by Commons geographically and by every other source through its
label as a search text. Each source reports a status: **available**, **needs key**, **unreachable** (last error and
time) or **unsupported** (for the active subject kind: a Pinterest board is served by Pinterest only, and Pinterest
serves nothing else).
`check_source` sends one lightweight request with an 8 s timeout and no retry.

For the sources below that search by words, an image is kept only if it **covers the screen without upscaling**
(width ≥ screen width and height ≥ screen height). The final wallpaper always uses the largest rendition we can fetch;
scoring uses a preview of roughly 1000 px (or the biggest thumbnail the source offers).

## Wikimedia Commons (default, no key)

One `generator=search` request returns photos with size, URLs, license, author, coordinates and categories.

**Curated only.** Every query is restricted to

```
incategory:Featured_pictures_on_Wikimedia_Commons|Quality_images|Valued_images_sorted_by_promotion_date
```

plus `filemime:image/jpeg fileh:>N` where N ≈ 80% of the screen height. Results use `gsrsort=random`, so every refill
brings a different subset.

**Scope.**

| Place | Query |
|---|---|
| City, district, landmark, mountain… (radius ≤ 40 km) | `nearcoord:<r>km,<lat>,<lon>` with r from the place's area (`√(area/π)`) or a per-kind default |
| Country, big region | 4 random notable sub-places (Wikidata, photogenic types) × `nearcoord:3km`, plus `Featured_pictures_of_<P373>` / `Quality_images_of_<P373>` |

A radius around a country's centroid would miss most of it and bleed into neighbours, hence the hierarchy mode.

**Off-topic filter.** Curated categories are full of insect macros, birds, food. A file is dropped when one of its
`Featured pictures of …` / `Quality images of …` categories mentions a subject from `OFF_TOPIC_SUBJECTS`
(insects, birds, mammals, fungi, food, portraits, maps…). Plants are deliberately *not* excluded: cherry blossoms and
bamboo forests are exactly what we want.

**Licensing.** Files without `LicenseShortName` are skipped. Author comes from `Artist` (HTML stripped). The UI and the
wallpaper caption always carry "Photo: <author>, <license>".

**Query mode.** Free text is reduced to plain words (no search operators) and searched with the same curated-category
restriction and size filter, paged by the refill seed.

**Sizes.** Wikimedia pre-renders thumbnails at 250/330/500/960/1280/1920/3840 px. Scoring uses the 960 px preview;
the wallpaper uses the smallest step that still covers the screen after cropping, or the original.

## Unsplash (opt-in, user's own key)

- `GET /search/photos?query=<place label>&orientation=portrait&content_filter=high`, header `Authorization: Client-ID <key>`.
- No geo search in the public API — the place label is the query.
- `likes` feed the curation score (≈1000 likes counts as "featured").
- When a photo becomes the wallpaper we call `/photos/<id>/download`, as the API guidelines require.
- Links to the photographer carry `utm_source=dynamic_themed_wallpapers&utm_medium=referral` (required; identifies the
  app, not the user).
- The key is validated for shape only and is never logged.

## Openverse (no key)

- `GET api.openverse.org/v1/images/?q=<text>&license_type=modification&aspect_ratio=tall&size=large&category=photograph&extension=jpg&mature=false&page_size=20&page=1..3`
  — one request per refill; anonymous limits are low.
- `license_type=modification` excludes "NoDerivatives" licences (the app crops). NC licences are included: a personal wallpaper.
- Caption: "Photo: <creator>, CC BY-SA 4.0" built from `license` + `license_version`. No curation signal.
- Images live on the providers' own hosts; only `https` URLs are accepted. Preview is Openverse's thumbnail (about 600 px).

## Wallhaven (no key)

- `GET wallhaven.cc/api/v1/search?q=<text>&categories=100&purity=100&atleast=WxH&ratios=portrait&sorting=random&seed=…`
- **SFW and General only, as constants.** `purity=100` is never configurable; no Anime, no People.
- The search API names no author and no licence: the caption reads "Wallpaper: Wallhaven #<id>" and the licence field
  says "No licence stated". Favourites feed the curation score.

## Pexels (user's key)

- `GET api.pexels.com/v1/search?query=<text>&orientation=portrait&size=large`, header `Authorization: <key>`.
- Attribution required: "Photo: <photographer> on Pexels". Final image is `src.original`; the preview is the original
  with `?w=1000` (the ready-made `large` rendition is a landscape crop).

## Pixabay (user's key)

- `GET pixabay.com/api/?key=<key>&q=<text>&image_type=photo&orientation=vertical&min_width=W&min_height=H&safesearch=true`.
- Pixabay forbids long-term hotlinking; photos are downloaded and cached. The key travels in the URL, so errors are
  scrubbed of it.
- `imageURL` / `fullHDURL` exist only with Pixabay full API access; otherwise the best file is `largeImageURL`
  (≤ 1280 px), which cannot cover a typical phone screen, so most hits are dropped. Likes feed the curation score.

## Pinterest (public boards, no key)

- **Subject:** a board address pasted by the user (`pinterest.com/<user>/<board>/`, also `ru.pinterest.com`, `pinterest.co.uk`,
  a trailing section or query; the `.rss` address itself works). It is normalised to
  `https://www.pinterest.com/<user>/<board>.rss`, which stays the saved identity of the board. Names may be percent-encoded or not
  (`обои` = `%D0%BE%D0%B1%D0%BE%D0%B8`); the saved form is encoded. Short `pin.it` links (the Share button of the Pinterest
  app) are opened one redirect at a time (at most 5, via `pin.it` → `api.pinterest.com/url_shortener/…` → board) until a
  board address shows up; a link that ends on a single pin is refused. Adding a board reads it once to check it exists
  (public) and to learn its name.
- **Reading a board: widget first, RSS as the fallback.** The primary read is the public JSON behind Pinterest's own board embed
  widget, `https://widgets.pinterest.com/v3/pidgets/boards/<user>/<board>/pins/` (names encoded as in the RSS address). Pins
  with `is_video` are skipped, the image is the largest of `images.*`, the pin links to `pinterest.com/pin/<id>/`, the author is
  the pinner's full name or username, the title is the board name. On any error, a `status` other than `success`, or no usable
  pin, the RSS feed is tried. If both fail the widget's error is shown, except that two 404s become "Pinterest did not find this
  board; is it public?". Why: on a real device in Nov 2026 the RSS answered 404 "Board not found" for a public board. The widget
  endpoint is unofficial and may change or disappear; nothing is sent beyond the app's normal headers.
- **Boards and Pinterest go together.** Activating a board switches the source to Pinterest and remembers the previous one;
  choosing a place or words restores it. Other sources are shown as unsupported while a board is active.
- **Only the latest ~25 pins.** Neither the widget nor the RSS feed carries more; older pins of a big board never appear. The UI says so.
- **Images.** (RSS) Items hold `<img src="https://i.pinimg.com/236x/ab/cd/ef/hash.jpg">` in the escaped description. The wallpaper
  uses `…/originals/ab/cd/ef/hash.jpg`; if that cannot be fetched or decoded (404, WebP — the decoder handles JPEG/PNG only),
  `/1200x/` then `/736x/`. Scoring and the in-app preview use `/736x/`. GIFs are skipped.
- **Dimensions** are not trusted from the feed (the widget's sizes are thumbnails): each image is probed with a ranged request for its first 64 KB and the header is read
  (up to 25 small requests per refill, six at a time, memoised within one refill). An image is kept only if it **covers the
  screen without upscaling**; everything smaller is dropped (decision with the owner), so on a phone only large pins qualify.
- **Caption:** "Pinterest: <board name>" on the wallpaper; the credit line is "Pin by <author> on Pinterest" if the feed names
  an author, otherwise "Pinterest". The pin links to its Pinterest page. No licence is stated (Pinterest content belongs to
  its owners; this is for a personal wallpaper).
- Pinterest may refuse automated requests or change the feed; a failed refresh is reported like any other source failure.

## When a source fails

- **In the app.** If a manual change fails because of the source (network, bad answer, nothing usable, missing key), the error
  banner offers "Try <source>" for up to three other sources that are available for the active subject (no key missing, no
  failure on record). The click runs one change with that source; the chosen source is not changed.
- **In the background.** If the scheduled change fails the same way (after WorkManager's retries), the app posts the Android
  notification "Couldn't get a photo from <source>" with one button per working alternative (up to three) and "Retry later"
  (one more attempt in an hour). A source button queues one change with that source only (`change:<source id>` for the
  worker, `Engine::change_now_with_source`); the override is never saved. Opening the notification opens the Sources screen.
  Android 13+ needs the notification permission, requested when the user sets the schedule time (or from the Schedule screen).

## Keys

Entered in the source settings, shape-checked only (length and characters), stored in the app database, never part of
the settings the UI reads, never logged. `check_source` finds out whether a service accepts the key.
