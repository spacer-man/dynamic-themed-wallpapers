# Privacy

The app has **no telemetry, no analytics, no accounts and no crash reporting**.

## Network endpoints

This is the complete list. Adding an endpoint requires updating this file in the same change.

| Host | When | What is sent |
|---|---|---|
| `www.wikidata.org` | you search for a place | the text you typed |
| `query.wikidata.org` | after you pick a place | the place's Wikidata ID (SPARQL query for sub-places) |
| `commons.wikimedia.org` | fetching candidate photos | place coordinates / category names; image downloads from `upload.wikimedia.org` |
| `upload.wikimedia.org` | downloading the chosen photos | file URL |
| `api.unsplash.com`, `images.unsplash.com` | **only if you enable Unsplash and enter your own key** | search query, your key |

All requests carry a generic `User-Agent` naming the app and its repository, as Wikimedia's policy requires. No device
identifiers, no account data.

## What stays on the device

- history of shown photos, likes and skips;
- the weights of the taste model learned from them;
- the photo cache;
- the Unsplash key (Android Keystore-backed encrypted storage).

None of it is uploaded anywhere. Uninstalling the app removes all of it.
