use wallpaper_core::http::fixture::FixtureHttp;
use wallpaper_core::source::{
    self, CandidateRequest, CommonsSource, OpenverseSource, PexelsSource, PixabaySource, Search,
    Source, SourceKind, UnsplashSource, WallhavenSource,
};
use wallpaper_core::{GeoPoint, Place, PlaceKind, Scope, SubPlace};

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn kyoto() -> Place {
    Place {
        qid: "Q34600".into(),
        label: "Киото".into(),
        description: None,
        coords: Some(GeoPoint::new(35.0116, 135.7681)),
        kind: PlaceKind::City,
        area_km2: Some(827.83),
        commons_category: Some("Kyoto".into()),
    }
}

#[test]
fn commons_radius_parses_and_filters() {
    let http = FixtureHttp::new().route(
        &["generator=search", "nearcoord:16233m,35.01160,135.76810"],
        fixture("commons/kyoto_radius.json"),
    );
    let place = kyoto();
    let scope = place.default_scope();
    let req = CandidateRequest::new(1080, 2400);
    let got = CommonsSource::new(&http)
        .candidates(
            &Search::Place {
                place: &place,
                scope: &scope,
            },
            &req,
        )
        .unwrap();

    // Insect close-up and the file without license are dropped; order follows search index.
    let ids: Vec<_> = got.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["commons:93947345", "commons:85860114"]);

    let bamboo = &got[1];
    assert_eq!(bamboo.source, SourceKind::Commons);
    assert_eq!(bamboo.title, "Bamboo Forest, Arashiyama, Kyoto, Japan");
    assert_eq!(bamboo.author, "Basile Morin");
    assert_eq!(bamboo.license.short_name, "CC BY-SA 4.0");
    assert!(bamboo.license.attribution_required);
    assert_eq!((bamboo.width, bamboo.height), (6558, 4372));
    assert!(!bamboo.image_url.contains("utm_"), "{}", bamboo.image_url);
    assert!(bamboo.preview_url.contains("/960px-"));
    assert!(bamboo
        .description
        .as_deref()
        .unwrap()
        .starts_with("Bamboo Forest, Arashiyama, Kyoto, Japan. Focus stacking"));
    assert_eq!(bamboo.place_label, "Киото");
    assert!(bamboo.coords.is_some());
    // Curation comes from the categories present on the page.
    assert!(
        !bamboo.curation.featured,
        "fixture lists no FP category for bamboo"
    );
    assert!(got[0].curation.featured);
    assert_eq!(bamboo.credit_line(), "Photo: Basile Morin, CC BY-SA 4.0");
}

#[test]
fn commons_api_warnings_are_errors_not_silent_garbage() {
    // Real-world case: a malformed nearcoord makes the API drop the geo filter and return
    // photos from anywhere, with only a warning to show for it.
    let http = FixtureHttp::new().route(
        &["generator=search"],
        r#"{"warnings":{"search":{"*":"The geo distance '16.2km' provided to 'nearcoord' could not be parsed."}},"query":{"pages":[]}}"#,
    );
    let place = kyoto();
    let err = CommonsSource::new(&http)
        .candidates(
            &Search::Place {
                place: &place,
                scope: &place.default_scope(),
            },
            &CandidateRequest::new(1080, 2400),
        )
        .unwrap_err();
    assert!(err.to_string().contains("could not be parsed"), "{err}");
}

#[test]
fn commons_sized_url_picks_smallest_sufficient_thumb() {
    let http =
        FixtureHttp::new().route(&["generator=search"], fixture("commons/kyoto_radius.json"));
    let src = CommonsSource::new(&http);
    let place = kyoto();
    let got = src
        .candidates(
            &Search::Place {
                place: &place,
                scope: &place.default_scope(),
            },
            &CandidateRequest::new(1080, 2400),
        )
        .unwrap();
    let bamboo = got.iter().find(|c| c.id == "commons:85860114").unwrap();
    // 6558x4372 landscape → to be 2400 tall it must be 3600 wide → 3840 step.
    let url = src.sized_url(bamboo, 1080, 2400);
    assert!(url.contains("/3840px-"), "{url}");
    // Bigger than the original → original.
    assert_eq!(src.sized_url(bamboo, 1080, 5000), bamboo.image_url);
}

#[test]
fn commons_merges_continued_categories() {
    let http = FixtureHttp::new()
        .route(
            &["clcontinue=500|Quality_images"],
            fixture("commons/continue_part2.json"),
        )
        .route(
            &["generator=search"],
            fixture("commons/continue_part1.json"),
        );
    let place = kyoto();
    let got = CommonsSource::new(&http)
        .candidates(
            &Search::Place {
                place: &place,
                scope: &place.default_scope(),
            },
            &CandidateRequest::new(1080, 2400),
        )
        .unwrap();
    // The second half of the categories reveals an insect close-up → dropped.
    assert!(got.is_empty());
    assert_eq!(http.requests().len(), 2);
}

#[test]
fn commons_hierarchy_samples_sub_places() {
    let http =
        FixtureHttp::new().route(&["generator=search"], fixture("commons/kyoto_radius.json"));
    let mut place = kyoto();
    place.kind = PlaceKind::Country;
    place.label = "Япония".into();
    place.commons_category = Some("Japan".into());
    let mut req = CandidateRequest::new(1080, 2400);
    req.sub_places = vec![SubPlace {
        qid: "Q39231".into(),
        label: "Фудзияма".into(),
        coords: GeoPoint::new(35.36, 138.73),
    }];
    let got = CommonsSource::new(&http)
        .candidates(
            &Search::Place {
                place: &place,
                scope: &Scope::Hierarchy,
            },
            &req,
        )
        .unwrap();
    let reqs = http.requests();
    assert_eq!(reqs.len(), 2, "{reqs:#?}");
    let decoded: Vec<String> = reqs.iter().map(|u| percent_decode(u)).collect();
    assert!(decoded[0].contains("nearcoord:3000m,35.36000,138.73000"));
    assert!(decoded[1].contains("Featured_pictures_of_Japan"));
    // Same fixture served twice → deduplicated.
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].place_label, "Фудзияма, Япония");
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[test]
fn unsplash_requires_plausible_key() {
    let http = FixtureHttp::new();
    assert!(UnsplashSource::new(&http, "").is_err());
    assert!(UnsplashSource::new(&http, "short").is_err());
    assert!(UnsplashSource::new(&http, "has spaces in it and is long enough").is_err());
}

#[test]
fn unsplash_search_and_download_ping() {
    let key = "AbCdEfGhIjKlMnOpQrStUvWxYz0123456789_-ab";
    let http = FixtureHttp::new()
        .route(
            &["/search/photos", "query=Киото", "orientation=portrait"],
            fixture("unsplash/search_kyoto.json"),
        )
        .route(&["/photos/abc123/download"], "{}");
    let src = UnsplashSource::new(&http, key).unwrap();
    let place = kyoto();
    let got = src
        .candidates(
            &Search::Place {
                place: &place,
                scope: &place.default_scope(),
            },
            &CandidateRequest::new(1080, 2400),
        )
        .unwrap();
    assert_eq!(got.len(), 1, "too-small photo filtered");
    let c = &got[0];
    assert_eq!(c.id, "unsplash:abc123");
    assert_eq!(c.title, "red torii gates in a forest");
    assert_eq!(c.author, "Jane Doe");
    assert!(c
        .author_url
        .as_deref()
        .unwrap()
        .contains("utm_medium=referral"));
    assert!(c.curation.score() > 0.99);
    assert_eq!(c.credit_line(), "Photo: Jane Doe on Unsplash");
    assert_eq!(
        src.sized_url(c, 1080, 2400),
        "https://images.unsplash.com/photo-1?ixid=x&w=1600&fm=jpg&q=85"
    );

    src.report_used(c).unwrap();
    let auth = http.headers(0);
    assert!(auth.contains(&("Authorization".into(), format!("Client-ID {key}"))));
    assert!(http.requests()[1].ends_with("/photos/abc123/download"));
}

fn phone() -> CandidateRequest {
    CandidateRequest::new(1080, 2400)
}

#[test]
fn commons_query_mode_searches_text_inside_the_curated_categories() {
    let http = FixtureHttp::new().route(
        &[
            "generator=search",
            "foggy forest",
            "Featured_pictures_on_Wikimedia_Commons",
        ],
        fixture("commons/kyoto_radius.json"),
    );
    let got = CommonsSource::new(&http)
        .candidates(&Search::Query("foggy \"forest\" incategory:Evil"), &phone())
        .unwrap();
    assert!(!got.is_empty());
    assert_eq!(got[0].place_label, "foggy \"forest\" incategory:Evil");
    let url = percent_decode(&http.requests()[0]);
    // Operators in the user's text are neutralised; the curated restriction stays.
    assert!(url.contains("foggy forest incategory Evil"), "{url}");
    assert!(url.contains("Quality_images"), "{url}");
    assert!(!url.contains("nearcoord"), "{url}");
}

#[test]
fn every_source_supports_places_and_queries_today() {
    use wallpaper_core::SubjectKind;
    for k in SourceKind::ALL {
        assert!(
            k.supports(SubjectKind::Place) && k.supports(SubjectKind::Query),
            "{k:?}"
        );
    }
    assert_eq!(
        SourceKind::ALL.iter().filter(|k| k.needs_key()).count(),
        3,
        "Unsplash, Pexels, Pixabay"
    );
}

#[test]
fn key_shape_validation() {
    assert!(SourceKind::Pexels.validate_key("short").is_err());
    assert!(SourceKind::Pexels
        .validate_key("has spaces and is certainly long enough")
        .is_err());
    assert_eq!(
        SourceKind::Pixabay
            .validate_key("  12345678-0123456789abcdef01234567  ")
            .unwrap(),
        "12345678-0123456789abcdef01234567"
    );
    let http = FixtureHttp::new();
    assert!(source::build(SourceKind::Pexels, &http, None).is_err());
    assert!(source::build(SourceKind::Pixabay, &http, Some("  ")).is_err());
    assert!(source::build(SourceKind::Wallhaven, &http, None).is_ok());
}

#[test]
fn openverse_parses_filters_and_names_licences() {
    let http = FixtureHttp::new().route(
        &["api.openverse.org/v1/images/"],
        fixture("openverse/search_fog.json"),
    );
    let src = OpenverseSource::new(&http);
    let got = src.candidates(&Search::Query("fog"), &phone()).unwrap();
    // Too small and plain-http results are dropped.
    let ids: Vec<_> = got.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "openverse:0f9a5c3e-6f0e-4b7c-9d1a-111111111111",
            "openverse:0f9a5c3e-6f0e-4b7c-9d1a-222222222222"
        ]
    );
    let c = &got[0];
    assert_eq!(c.author, "Anna Example");
    assert_eq!(c.license.short_name, "CC BY-SA 4.0");
    assert!(c.license.attribution_required);
    assert_eq!(c.credit_line(), "Photo: Anna Example, CC BY-SA 4.0");
    assert_eq!(c.image_url, "https://live.staticflickr.com/65535/111_o.jpg");
    assert!(c.preview_url.ends_with("/thumb/"));
    assert_eq!(c.place_label, "fog");
    assert_eq!(src.sized_url(c, 1080, 2400), c.image_url);
    let pd = &got[1];
    assert_eq!(pd.author, "Unknown");
    assert_eq!(pd.license.short_name, "CC0 1.0");
    assert!(!pd.license.attribution_required);

    let url = percent_decode(&http.requests()[0]);
    for needle in [
        "q=fog",
        "license_type=modification",
        "aspect_ratio=tall",
        "size=large",
        "page_size=20",
    ] {
        assert!(url.contains(needle), "{needle} in {url}");
    }
    assert_eq!(http.requests().len(), 1, "one request per refill");
    assert!(src.check().is_ok());
}

#[test]
fn openverse_place_uses_the_label_as_query() {
    let http = FixtureHttp::new().route(&["q=Киото"], fixture("openverse/search_fog.json"));
    let place = kyoto();
    let scope = place.default_scope();
    let got = OpenverseSource::new(&http)
        .candidates(
            &Search::Place {
                place: &place,
                scope: &scope,
            },
            &phone(),
        )
        .unwrap();
    assert_eq!(got[0].place_label, "Киото");
}

#[test]
fn wallhaven_is_sfw_general_only_and_filters_by_size() {
    let http = FixtureHttp::new().route(
        &["wallhaven.cc/api/v1/search"],
        fixture("wallhaven/search_fog.json"),
    );
    let src = WallhavenSource::new(&http);
    let mut req = phone();
    req.seed = 7;
    let got = src.candidates(&Search::Query("fog"), &req).unwrap();
    assert_eq!(got.len(), 1, "the two smaller wallpapers are dropped");
    let c = &got[0];
    assert_eq!(c.id, "wallhaven:abc123");
    assert_eq!(
        c.image_url,
        "https://w.wallhaven.cc/full/ab/wallhaven-abc123.jpg"
    );
    assert!(c.preview_url.contains("/orig/"));
    assert_eq!((c.width, c.height), (1440, 3040));
    assert_eq!(c.curation.likes, Some(340));
    assert!(c.curation.score() > 0.8);
    assert_eq!(c.credit_line(), "Wallpaper: Wallhaven #abc123");
    assert_eq!(c.license.short_name, "No licence stated");

    let url = percent_decode(&http.requests()[0]);
    for needle in [
        "purity=100",
        "categories=100",
        "atleast=1080x2400",
        "ratios=portrait",
        "q=fog",
        "seed=000007",
    ] {
        assert!(url.contains(needle), "{needle} in {url}");
    }
    // The check request is SFW as well.
    src.check().unwrap();
    assert!(percent_decode(&http.requests()[1]).contains("purity=100"));
}

#[test]
fn pexels_sends_the_key_in_the_authorization_header() {
    let key = "563492ad6f91700001000001abcdefabcdefabcdefabcdefabcdef12";
    let http = FixtureHttp::new().route(
        &["api.pexels.com/v1/search"],
        fixture("pexels/search_fog.json"),
    );
    let src = PexelsSource::new(&http, key).unwrap();
    let got = src.candidates(&Search::Query("fog"), &phone()).unwrap();
    assert_eq!(got.len(), 1);
    let c = &got[0];
    assert_eq!(c.id, "pexels:1234567");
    assert_eq!(c.title, "Fog between tall pine trees");
    assert_eq!(c.author, "Jane Photographer");
    assert_eq!(c.credit_line(), "Photo: Jane Photographer on Pexels");
    assert!(c.license.attribution_required);
    assert_eq!(
        c.image_url,
        "https://images.pexels.com/photos/1234567/pexels-photo-1234567.jpeg"
    );
    assert!(c.preview_url.ends_with("?auto=compress&cs=tinysrgb&w=1000"));
    assert!(http
        .headers(0)
        .contains(&("Authorization".into(), key.to_owned())));
    let url = percent_decode(&http.requests()[0]);
    assert!(
        url.contains("orientation=portrait") && url.contains("size=large"),
        "{url}"
    );
    assert!(!url.contains(key), "the key never goes into the URL");
}

#[test]
fn pixabay_uses_the_best_rendition_it_can_fetch() {
    let key = "12345678-0123456789abcdef01234567";
    let http = FixtureHttp::new().route(&["pixabay.com/api/"], fixture("pixabay/search_fog.json"));
    let src = PixabaySource::new(&http, key).unwrap();

    // Phone: only the hit with a full-size URL covers 1080x2400.
    let got = src.candidates(&Search::Query("fog"), &phone()).unwrap();
    assert_eq!(got.len(), 1);
    let c = &got[0];
    assert_eq!(c.id, "pixabay:5550001");
    assert_eq!(c.image_url, "https://pixabay.com/get/g11_original.jpg");
    assert_eq!(c.preview_url, "https://pixabay.com/get/g11_1280.jpg");
    assert_eq!((c.width, c.height), (4000, 6000));
    assert_eq!(c.author, "some_user");
    assert_eq!(
        c.author_url.as_deref(),
        Some("https://pixabay.com/users/some_user-777/")
    );
    assert_eq!(c.credit_line(), "Photo: some_user on Pixabay");
    assert_eq!(c.curation.likes, Some(120));
    let url = percent_decode(&http.requests()[0]);
    for needle in [
        "key=12345678-0123456789abcdef01234567",
        "image_type=photo",
        "orientation=vertical",
        "min_width=1080",
        "min_height=2400",
        "safesearch=true",
    ] {
        assert!(url.contains(needle), "{needle} in {url}");
    }

    // A small screen: the 1280 px rendition (853x1280) is enough for the hit without
    // full-size access, and its size is reported as 853x1280, not the original's.
    let small = CandidateRequest::new(720, 1280);
    let got = src.candidates(&Search::Query("fog"), &small).unwrap();
    let ids: Vec<_> = got.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(
        ids,
        ["pixabay:5550001", "pixabay:5550002", "pixabay:5550003"]
    );
    assert_eq!(got[1].image_url, "https://pixabay.com/get/g22_1280.jpg");
    assert_eq!((got[1].width, got[1].height), (853, 1280));
}

#[test]
fn pixabay_never_leaks_its_key_in_errors() {
    let key = "12345678-0123456789abcdef01234567";
    let http = FixtureHttp::new(); // every request is a 404 that echoes the URL
    let src = PixabaySource::new(&http, key).unwrap();
    for err in [
        src.candidates(&Search::Query("fog"), &phone()).unwrap_err(),
        src.check().unwrap_err(),
    ] {
        let msg = err.to_string();
        assert!(!msg.contains(key) && !msg.contains("key="), "{msg}");
        assert!(msg.contains("404"), "{msg}");
    }
}
