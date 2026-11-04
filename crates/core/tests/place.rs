use wallpaper_core::http::fixture::FixtureHttp;
use wallpaper_core::{GeoPoint, PlaceKind, Scope, Wikidata};

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn search_returns_hits_with_descriptions() {
    let http = FixtureHttp::new().route(
        &["wbsearchentities", "search=Киото"],
        fixture("wikidata/search_kyoto_ru.json"),
    );
    let hits = Wikidata::new(&http, "ru").search("  Киото ", 10).unwrap();
    assert_eq!(hits.len(), 3);
    assert_eq!(hits[0].qid, "Q34600");
    assert_eq!(hits[0].label, "Киото");
    assert_eq!(hits[1].description.as_deref(), Some("префектура Японии"));
    assert_eq!(hits[2].description, None);
}

#[test]
fn search_falls_back_to_english() {
    let http = FixtureHttp::new()
        .route(&["wbsearchentities", "language=ru"], r#"{"search":[]}"#)
        .route(
            &["wbsearchentities", "language=en"],
            fixture("wikidata/search_kyoto_ru.json"),
        );
    let hits = Wikidata::new(&http, "ru").search("Kyoto", 10).unwrap();
    assert_eq!(hits[0].qid, "Q34600");
    assert_eq!(http.requests().len(), 2);
}

#[test]
fn empty_search_makes_no_requests() {
    let http = FixtureHttp::new();
    assert!(Wikidata::new(&http, "ru")
        .search("   ", 10)
        .unwrap()
        .is_empty());
    assert!(http.requests().is_empty());
}

#[test]
fn resolve_kyoto() {
    let http = FixtureHttp::new()
        .route(&["titles=Q34600"], fixture("wikidata/terms_Q34600.json"))
        .route(
            &["wbgetclaims", "property=P31"],
            fixture("wikidata/claims_Q34600_P31.json"),
        )
        .route(
            &["wbgetclaims", "property=P2046"],
            fixture("wikidata/claims_Q34600_P2046.json"),
        )
        .route(
            &["wbgetclaims", "property=P373"],
            fixture("wikidata/claims_Q34600_P373.json"),
        );
    let place = Wikidata::new(&http, "ru").resolve("Q34600").unwrap();
    assert_eq!(place.label, "Киото");
    assert_eq!(
        place.description.as_deref(),
        Some("город в префектуре Киото")
    );
    assert_eq!(place.kind, PlaceKind::City);
    assert_eq!(place.area_km2, Some(827.83));
    assert_eq!(place.commons_category.as_deref(), Some("Kyoto"));
    let c = place.coords.unwrap();
    assert!((c.lat - 35.0116).abs() < 1e-3 && (c.lon - 135.7681).abs() < 1e-3);
    assert!(matches!(place.default_scope(), Scope::Radius { .. }));
}

#[test]
fn resolve_rejects_garbage_ids() {
    let http = FixtureHttp::new();
    assert!(Wikidata::new(&http, "en").resolve("Q1|Q2").is_err());
    assert!(http.requests().is_empty());
}

#[test]
fn sub_places_of_a_country_skip_items_without_coordinates() {
    let http = FixtureHttp::new().route(
        &["generator=search", "haswbstatement:P17=Q17"],
        fixture("wikidata/subplaces_Q17.json"),
    );
    let japan = wallpaper_core::Place {
        qid: "Q17".into(),
        label: "Япония".into(),
        description: None,
        coords: Some(GeoPoint::new(35.0, 136.0)),
        kind: PlaceKind::Country,
        area_km2: Some(377_975.0),
        commons_category: Some("Japan".into()),
    };
    let subs = Wikidata::new(&http, "ru").sub_places(&japan, 20).unwrap();
    assert_eq!(subs.len(), 2);
    assert_eq!(subs[0].label, "Фудзияма");
    assert_eq!(subs[1].qid, "Q261932");
}
