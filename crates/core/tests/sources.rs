use wallpaper_core::http::fixture::FixtureHttp;
use wallpaper_core::source::{CandidateRequest, CommonsSource, Source, SourceKind, UnsplashSource};
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
        .candidates(&place, &scope, &req)
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
            &place,
            &place.default_scope(),
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
            &place,
            &place.default_scope(),
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
            &place,
            &place.default_scope(),
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
        .candidates(&place, &Scope::Hierarchy, &req)
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
            &place,
            &place.default_scope(),
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
