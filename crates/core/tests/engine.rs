//! The whole engine, offline: place → prefetch → change now → feedback → history.

use std::path::PathBuf;
use std::sync::Arc;

use image::{codecs::jpeg::JpegEncoder, DynamicImage, Rgb, RgbImage};
use wallpaper_core::http::fixture::FixtureHttp;
use wallpaper_core::pipeline::{self, Screen};
use wallpaper_core::score::ToneFilter;
use wallpaper_core::service::{CaptionAlign, Engine, Settings, DEFAULT_CAPTION_Y};

const SCREEN: Screen = Screen {
    width: 540,
    height: 1200,
};
const NOW: i64 = 1_800_000_000;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("engine-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Distinct low-frequency pattern per `k`, so the photos don't count as near-duplicates.
fn photo(k: u32, base: u8) -> Vec<u8> {
    let img = RgbImage::from_fn(640, 480, |x, y| {
        let (fx, fy) = (6.0 + k as f32 * 7.0, 9.0 + k as f32 * 5.0);
        let wave = ((x as f32 / fx).sin() * 40.0 + (y as f32 / fy).cos() * 40.0) as i32;
        let n = ((x.wrapping_mul(2654435761) ^ y.wrapping_mul(40503) ^ k) >> 24) as i32 / 16 - 8;
        let v = (base as i32 + wave + n).clamp(0, 255) as u8;
        Rgb([v, (v as i32 * 3 / 4) as u8, 120])
    });
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 90)
        .encode_image(&DynamicImage::ImageRgb8(img))
        .unwrap();
    out
}

fn commons_page(id: u64, name: &str) -> String {
    format!(
        r#"{{"pageid":{id},"ns":6,"title":"File:{name}.jpg","index":{id},
        "imageinfo":[{{"width":3000,"height":4000,
          "thumburl":"https://thumb.wikimedia.org/a/b/{name}.jpg/960px-{name}.jpg",
          "url":"https://upload.wikimedia.org/a/b/{name}.jpg",
          "descriptionurl":"https://commons.wikimedia.org/wiki/File:{name}.jpg",
          "extmetadata":{{"ObjectName":{{"value":"{name}"}},"Artist":{{"value":"Tester"}},
            "LicenseShortName":{{"value":"CC BY 4.0"}},"AttributionRequired":{{"value":"true"}}}},
          "mime":"image/jpeg"}}],
        "coordinates":[{{"lat":35.0,"lon":135.7,"primary":true,"globe":"earth"}}],
        "categories":[{{"ns":14,"title":"Category:Featured pictures on Wikimedia Commons"}}]}}"#
    )
}

fn engine(name: &str) -> (Engine, Arc<FixtureHttp>) {
    engine_with_brightness(name, 90, 100)
}

fn engine_with_brightness(name: &str, a: u8, b: u8) -> (Engine, Arc<FixtureHttp>) {
    let commons = format!(
        r#"{{"batchcomplete":true,"query":{{"pages":[{},{}]}}}}"#,
        commons_page(1, "alpha"),
        commons_page(2, "beta")
    );
    let http = Arc::new(
        FixtureHttp::new()
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
            )
            .route(&["generator=search", "commons.wikimedia.org"], commons)
            .route(&["alpha.jpg"], photo(1, a))
            .route(&["beta.jpg"], photo(2, b)),
    );
    let dir = scratch(name);
    let engine = Engine::with_http(dir.join("app.db"), dir.join("cache"), http.clone());
    (engine, http)
}

#[test]
fn settings_default_and_persist() {
    let (engine, _) = engine("settings");
    assert_eq!(engine.settings().unwrap(), Settings::default());
    let saved = engine
        .save_settings(Settings {
            tone: ToneFilter::Dark,
            cache_mb: 1,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(saved.cache_mb, 20, "sanitised");
    assert_eq!(engine.settings().unwrap().tone, ToneFilter::Dark);
}

#[test]
fn nothing_works_before_a_place_is_chosen() {
    let (engine, _) = engine("noplace");
    assert!(engine.selected_place().unwrap().is_none());
    assert!(engine.change_now(SCREEN, false, NOW).is_err());
    assert!(engine.current_wallpaper(true).unwrap().is_none());
}

#[test]
fn full_day_cycle() {
    let (engine, http) = engine("cycle");
    let place = engine.select_place("Q34600", "en").unwrap();
    assert_eq!(engine.selected_place().unwrap().unwrap(), place);

    // First wallpaper: queue is empty, so it prefetches one and takes it.
    let shown = engine.change_now(SCREEN, false, NOW).unwrap();
    assert!(shown.caption.starts_with("Киото · "), "{}", shown.caption);
    assert_eq!(shown.credit, "Photo: Tester, CC BY 4.0");
    let rendered = pipeline::decode(&std::fs::read(&shown.image_path).unwrap()).unwrap();
    assert_eq!((rendered.width(), rendered.height()), (540, 1200));

    // The UI can read it back, themed.
    let cur = engine.current_wallpaper(true).unwrap().unwrap();
    assert_eq!(cur.entry.candidate.id, shown.entry.candidate.id);
    assert!(cur.theme.dark && cur.image_path.is_some());
    assert!(!engine.current_wallpaper(false).unwrap().unwrap().theme.dark);

    // Feedback is stored and trains the model exactly once.
    engine
        .feedback(&shown.entry.candidate.id, true, NOW + 1)
        .unwrap();
    engine
        .feedback(&shown.entry.candidate.id, true, NOW + 2)
        .unwrap();
    let hist = engine.history(10, 0).unwrap();
    assert_eq!(hist.len(), 1);
    assert_eq!(hist[0].liked, Some(true));
    assert_eq!(engine.store().unwrap().load_model().unwrap().updates, 1);

    // Second day: the other photo; shown ones are never picked again.
    let second = engine.change_now(SCREEN, false, NOW + 86_400).unwrap();
    assert_ne!(second.entry.candidate.id, shown.entry.candidate.id);
    assert_eq!(engine.history(10, 0).unwrap().len(), 2);
    assert_eq!(
        engine
            .current_wallpaper(true)
            .unwrap()
            .unwrap()
            .entry
            .candidate
            .id,
        second.entry.candidate.id
    );

    // Third day: nothing left in this tiny world → a clear error, not a repeat.
    let err = engine
        .change_now(SCREEN, false, NOW + 2 * 86_400)
        .unwrap_err();
    assert!(err.to_string().contains("no suitable photo"), "{err}");
    assert_eq!(engine.history(10, 0).unwrap().len(), 2);

    // Every network request stayed on the documented hosts.
    for url in http.requests() {
        let host = url.split('/').nth(2).unwrap();
        assert!(
            [
                "www.wikidata.org",
                "commons.wikimedia.org",
                "thumb.wikimedia.org",
                "upload.wikimedia.org"
            ]
            .contains(&host),
            "unexpected host {host}"
        );
    }
}

#[test]
fn tone_filter_applies_to_the_queue_and_follow_system_overrides_it() {
    // Clearly dark photos (luma ≈ 0.15).
    let (engine, _) = engine_with_brightness("tone", 35, 45);
    engine.select_place("Q34600", "en").unwrap();

    let with = |s: Settings| engine.save_settings(s).unwrap();
    // A light filter must refuse dark photos — nothing is shown, nothing is recorded.
    with(Settings {
        tone: ToneFilter::Light,
        ..Default::default()
    });
    assert!(engine.change_now(SCREEN, false, NOW).is_err());
    assert!(engine.history(10, 0).unwrap().is_empty());
    // "Follow system" overrides the manual tone: light system → light photos → none here…
    with(Settings {
        tone: ToneFilter::Dark,
        follow_system_theme: true,
        ..Default::default()
    });
    assert!(engine.change_now(SCREEN, false, NOW).is_err());
    // …dark system → dark photos → found.
    assert!(engine.change_now(SCREEN, true, NOW).is_ok());
    // And the manual dark filter works on its own.
    with(Settings {
        tone: ToneFilter::Dark,
        ..Default::default()
    });
    assert!(engine.change_now(SCREEN, false, NOW + 1).is_ok());
}

#[test]
fn changing_place_drops_the_prefetched_queue_and_forget_wipes_learning() {
    let (engine, _) = engine("reset");
    engine.select_place("Q34600", "en").unwrap();
    engine.prefetch(SCREEN, false, 2, NOW).unwrap();
    assert_eq!(engine.cache_info().unwrap().ready, 2);
    engine.select_place("Q34600", "en").unwrap();
    assert_eq!(engine.cache_info().unwrap().ready, 0);

    let shown = engine.change_now(SCREEN, false, NOW).unwrap();
    engine
        .feedback(&shown.entry.candidate.id, false, NOW)
        .unwrap();
    engine.forget_all().unwrap();
    assert!(engine.history(10, 0).unwrap().is_empty());
    assert_eq!(engine.store().unwrap().load_model().unwrap().updates, 0);
    assert_eq!(engine.cache_info().unwrap().bytes, 0);
    assert!(engine.selected_place().unwrap().is_some(), "place survives");
}

#[test]
fn prefetch_if_low_only_works_when_needed() {
    let (engine, http) = engine("low");
    engine.select_place("Q34600", "en").unwrap();
    let first = engine.prefetch_if_low(SCREEN, false, NOW).unwrap();
    assert_eq!(first.unwrap().added, 2);
    let before = http.requests().len();
    // 2 ready is < 3, so it tries again, finds nothing new but must not fail while some are ready.
    let again = engine.prefetch_if_low(SCREEN, false, NOW + 1).unwrap();
    assert_eq!(again.unwrap().added, 0);
    assert!(http.requests().len() > before);
}

#[test]
fn a_queue_filled_under_one_filter_is_filtered_again_when_taken() {
    // alpha is dark, beta is light.
    let (engine, _) = engine_with_brightness("queue-tone", 35, 215);
    engine.select_place("Q34600", "en").unwrap();
    engine.prefetch(SCREEN, false, 2, NOW).unwrap();
    assert_eq!(engine.cache_info().unwrap().ready, 2);

    let with = |s: Settings| engine.save_settings(s).unwrap();
    with(Settings {
        tone: ToneFilter::Light,
        ..Default::default()
    });
    let shown = engine.change_now(SCREEN, false, NOW).unwrap();
    assert_eq!(
        shown.entry.candidate.title, "beta",
        "the light photo, straight from the queue"
    );

    // Now the user switches to dark: the queued dark photo is used, no new search needed.
    with(Settings {
        tone: ToneFilter::Dark,
        ..Default::default()
    });
    let shown = engine.change_now(SCREEN, false, NOW + 1).unwrap();
    assert_eq!(shown.entry.candidate.title, "alpha");
}

#[test]
fn changing_the_radius_drops_the_queue_but_changing_tone_does_not() {
    let (engine, _) = engine("radius-queue");
    engine.select_place("Q34600", "en").unwrap();
    engine.prefetch(SCREEN, false, 2, NOW).unwrap();
    assert_eq!(engine.cache_info().unwrap().ready, 2);
    engine
        .save_settings(Settings {
            tone: ToneFilter::Dark,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(engine.cache_info().unwrap().ready, 2);
    engine
        .save_settings(Settings {
            radius_km: Some(5.0),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(engine.cache_info().unwrap().ready, 0);
}

#[test]
fn caption_layout_is_configurable_sanitised_and_reapplied_without_a_new_photo() {
    let (engine, _) = engine("caption");
    engine.select_place("Q34600", "en").unwrap();
    assert!(
        engine.current_shown().unwrap().is_none(),
        "nothing shown yet"
    );

    let first = engine.change_now(SCREEN, false, NOW).unwrap();
    assert_eq!(first.layout.y, DEFAULT_CAPTION_Y);
    assert_eq!(first.layout.align, CaptionAlign::Left);
    assert!(!first.caption.is_empty());

    // Move the caption to the top, centred: re-applying shows the same photo, new layout.
    let saved = engine
        .save_settings(Settings {
            caption_y: 6,
            caption_align: CaptionAlign::Center,
            ..engine.settings().unwrap()
        })
        .unwrap();
    assert_eq!(saved.caption_y, 6);
    let again = engine.current_shown().unwrap().unwrap();
    assert_eq!(again.entry.candidate.id, first.entry.candidate.id);
    assert_eq!(again.image_path, first.image_path);
    assert_eq!(
        (again.layout.y, again.layout.align),
        (6, CaptionAlign::Center)
    );
    assert_eq!(again.caption, first.caption);

    // Turning the caption off blanks the text but keeps the photo.
    engine
        .save_settings(Settings {
            caption_enabled: false,
            ..engine.settings().unwrap()
        })
        .unwrap();
    let bare = engine.current_shown().unwrap().unwrap();
    assert!(bare.caption.is_empty() && bare.credit.is_empty());

    // Out-of-range values from the UI are pulled back, so the text cannot leave the screen.
    let wild = engine
        .save_settings(Settings {
            caption_y: 255,
            ..engine.settings().unwrap()
        })
        .unwrap();
    assert_eq!(wild.caption_y, 97);
    let wild = engine
        .save_settings(Settings {
            caption_y: 0,
            ..engine.settings().unwrap()
        })
        .unwrap();
    assert_eq!(wild.caption_y, 3);
}

#[test]
fn a_history_photo_can_be_set_again_from_cache_or_from_the_network() {
    let (engine, http) = engine("reapply");
    engine.select_place("Q34600", "en").unwrap();
    let first = engine.change_now(SCREEN, false, NOW).unwrap();
    let second = engine.change_now(SCREEN, false, NOW + 86_400).unwrap();
    assert_ne!(first.entry.candidate.id, second.entry.candidate.id);

    // Cached render still on disk: no network at all, but a fresh history row.
    let before = http.requests().len();
    let again = engine
        .apply_from_history(first.entry.id, SCREEN, NOW + 2 * 86_400)
        .unwrap();
    assert_eq!(http.requests().len(), before, "served from the cache");
    assert_eq!(again.entry.candidate.id, first.entry.candidate.id);
    assert_ne!(again.entry.id, first.entry.id, "recorded as a new change");
    assert_eq!(again.image_path, first.image_path);
    assert!(!again.caption.is_empty());
    assert_eq!(engine.history(10, 0).unwrap().len(), 3);
    assert_eq!(
        engine.current_shown().unwrap().unwrap().entry.id,
        again.entry.id
    );

    // Cache gone: the photo is downloaded and rendered again at the screen size.
    engine.clear_cache().unwrap();
    assert!(engine.current_shown().unwrap().is_none());
    let before = http.requests().len();
    let rebuilt = engine
        .apply_from_history(second.entry.id, SCREEN, NOW + 3 * 86_400)
        .unwrap();
    assert!(http.requests().len() > before, "downloaded again");
    let img = pipeline::decode(&std::fs::read(&rebuilt.image_path).unwrap()).unwrap();
    assert_eq!((img.width(), img.height()), (540, 1200));
    assert_eq!(engine.history(10, 0).unwrap().len(), 4);

    // Unknown history id: a clear error, nothing recorded.
    let err = engine.apply_from_history(9999, SCREEN, NOW).unwrap_err();
    assert!(err.to_string().contains("no longer"), "{err}");
    assert_eq!(engine.history(10, 0).unwrap().len(), 4);
}

#[test]
fn setting_a_history_photo_offline_fails_cleanly_when_the_file_is_gone() {
    let (engine, _) = engine("reapply-offline");
    engine.select_place("Q34600", "en").unwrap();
    let first = engine.change_now(SCREEN, false, NOW).unwrap();
    engine.clear_cache().unwrap();

    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("engine-reapply-offline-{}", std::process::id()));
    let offline = Engine::with_http(
        dir.join("app.db"),
        dir.join("cache"),
        Arc::new(FixtureHttp::new()),
    );
    assert!(offline
        .apply_from_history(first.entry.id, SCREEN, NOW + 1)
        .is_err());
    assert_eq!(offline.history(10, 0).unwrap().len(), 1, "nothing recorded");
}
