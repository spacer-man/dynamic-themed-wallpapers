# Photo sources

A source turns a place into candidates (`crates/core/src/source/`). It never downloads images and never judges
beauty beyond what humans already curated — that's the job of `image/` and `score/`.

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
