# Privacy

The app has **no telemetry, no analytics, no accounts and no crash reporting**.

## Network endpoints

This is the complete list. Adding an endpoint requires updating this file in the same change.

| Host | When | What is sent |
|---|---|---|
| `www.wikidata.org` | you search for / pick a place | the text you typed; the place's Wikidata ID |
| `commons.wikimedia.org` | fetching candidate photos | place coordinates / category names |
| `upload.wikimedia.org`, `thumb.wikimedia.org` | downloading previews and the chosen photos | file URL |
| `api.openverse.org` | **only if you pick Openverse** | search words (query or place name) |
| image hosts named by Openverse (e.g. `live.staticflickr.com`, `upload.wikimedia.org`) | downloading a chosen Openverse photo (https only) | file URL |
| `wallhaven.cc`, `w.wallhaven.cc`, `th.wallhaven.cc` | **only if you pick Wallhaven** (SFW, General category only) | search words, screen size |
| `api.unsplash.com`, `images.unsplash.com` | **only if you pick Unsplash and enter your own key** | search words, your key |
| `api.pexels.com`, `images.pexels.com` | **only if you pick Pexels and enter your own key** | search words, your key (header) |
| `pixabay.com`, `cdn.pixabay.com` | **only if you pick Pixabay and enter your own key** | search words, screen size, your key (in the URL; scrubbed from errors) |
| `www.pinterest.com` | **only if you pick Pinterest** (adding a board) | the board address you pasted (`/<user>/<board>.rss`); `robots.txt` for the availability check |
| `widgets.pinterest.com` | **only if you pick Pinterest** (adding a board, refreshing) | the board's user and name in the path (`/v3/pidgets/boards/<user>/<board>/pins/`), read first; the RSS address above is the fallback |
| `i.pinimg.com` | **only if you pick Pinterest** | the pin images: the first 64 KB of each (to read its size), then the previews and the chosen photo |

Only the source you picked is contacted. "Check all" in the source settings sends one small request to each source that
is usable (no request to a key source without a key). Free-text subjects are sent to the chosen source as search words.

All requests carry a generic `User-Agent` naming the app and its repository, as Wikimedia's policy requires. No device
identifiers, no account data.

## Notifications

If the scheduled change cannot get a photo, the app can show a local notification offering another source. Nothing is sent
anywhere for this. Android 13+ asks for the notification permission when you set the schedule time, never at app start.

## What stays on the device

- history of shown photos, likes and skips;
- the weights of the taste model learned from them;
- the photo cache;
- the API keys you entered (Unsplash, Pexels, Pixabay), in the app's private database (see OPEN-QUESTIONS Q13). They are never part of the settings the UI reads and never logged.

None of it is uploaded anywhere. Uninstalling the app removes all of it.
