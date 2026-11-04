//! End-to-end selection on synthetic photos served from an offline fixture client.

use image::{codecs::jpeg::JpegEncoder, DynamicImage, Rgb, RgbImage};
use wallpaper_core::http::fixture::FixtureHttp;
use wallpaper_core::pipeline::{self, Screen};
use wallpaper_core::score::{Model, Reject, ToneFilter};
use wallpaper_core::source::{Candidate, CommonsSource, Curation, License, SourceKind};

const SCREEN: Screen = Screen {
    width: 540,
    height: 1200,
};

fn noise(x: u32, y: u32, seed: u32) -> u8 {
    let mut v =
        x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263) ^ seed.wrapping_mul(2246822519);
    v = (v ^ (v >> 13)).wrapping_mul(1274126177);
    (v >> 24) as u8
}

/// Smooth gradient with a bit of texture, around `base` brightness.
fn photo(w: u32, h: u32, base: u8, seed: u32) -> Vec<u8> {
    let img = RgbImage::from_fn(w, h, |x, y| {
        let wave = ((x as f32 / 23.0).sin() * 30.0 + (y as f32 / 31.0).cos() * 30.0) as i32;
        let n = noise(x, y, seed) as i32 / 8 - 16;
        let v = (base as i32 + wave + n).clamp(0, 255) as u8;
        Rgb([
            v,
            (v as i32 * 3 / 4) as u8,
            (v as i32 / 2 + 40).min(255) as u8,
        ])
    });
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 90)
        .encode_image(&DynamicImage::ImageRgb8(img))
        .unwrap();
    out
}

fn candidate(id: &str, preview: &str, curated: bool) -> Candidate {
    Candidate {
        source: SourceKind::Commons,
        id: id.into(),
        title: id.into(),
        description: None,
        page_url: format!("https://commons.wikimedia.org/wiki/{id}"),
        image_url: format!("https://upload.wikimedia.org/{id}.jpg"),
        preview_url: preview.into(),
        width: 1600,
        height: 1200,
        author: "Tester".into(),
        author_url: None,
        license: License {
            short_name: "CC0".into(),
            url: None,
            attribution_required: false,
        },
        curation: Curation {
            featured: curated,
            ..Default::default()
        },
        coords: None,
        place_label: "Test place".into(),
    }
}

#[test]
fn tone_filter_never_lets_the_wrong_tone_through() {
    let http = FixtureHttp::new()
        .route(&["dark1"], photo(640, 480, 45, 1))
        .route(&["dark2"], photo(640, 480, 55, 2))
        .route(&["light1"], photo(640, 480, 210, 3))
        .route(&["light2"], photo(640, 480, 200, 4));
    let cands = vec![
        candidate("dark1", "https://x/dark1.jpg", true),
        candidate("light1", "https://x/light1.jpg", true),
        candidate("dark2", "https://x/dark2.jpg", false),
        candidate("light2", "https://x/light2.jpg", false),
    ];
    let (analysed, failed) = pipeline::analyse_all(&http, cands, SCREEN, 3);
    assert!(failed.is_empty(), "{failed:?}");
    assert_eq!(analysed.len(), 4);
    // Input order is preserved even with several threads.
    assert_eq!(analysed[0].0.id, "dark1");
    assert_eq!(analysed[3].0.id, "light2");

    for (filter, wrong, right) in [
        (ToneFilter::Dark, ["light1", "light2"], ["dark1", "dark2"]),
        (ToneFilter::Light, ["dark1", "dark2"], ["light1", "light2"]),
    ] {
        let ev = pipeline::evaluate(analysed.clone(), &Model::default(), SCREEN, filter, &[]);
        for e in &ev {
            let id = e.candidate.id.as_str();
            if wrong.contains(&id) {
                assert_eq!(e.verdict, Err(Reject::WrongTone), "{id} under {filter:?}");
            }
            if right.contains(&id) {
                assert!(e.verdict.is_ok(), "{id}: {:?}", e.verdict);
            }
        }
        // 20 draws with exploration: the wrong tone is never picked.
        for seed in 0..20 {
            let mut rng = fastrand::Rng::with_seed(seed);
            let i = pipeline::choose(&ev, 0.15, &mut rng).unwrap();
            assert!(right.contains(&ev[i].candidate.id.as_str()));
        }
    }
}

#[test]
fn curated_photo_wins_when_everything_else_is_equal() {
    let http = FixtureHttp::new()
        .route(&["a.jpg"], photo(640, 480, 60, 7))
        .route(&["b.jpg"], photo(640, 480, 60, 7));
    let cands = vec![
        candidate("plain", "https://x/a.jpg", false),
        candidate("featured", "https://x/b.jpg", true),
    ];
    let (analysed, _) = pipeline::analyse_all(&http, cands, SCREEN, 1);
    let ev = pipeline::evaluate(analysed, &Model::default(), SCREEN, ToneFilter::Any, &[]);
    let mut rng = fastrand::Rng::with_seed(1);
    let i = pipeline::choose(&ev, 0.0, &mut rng).unwrap();
    assert_eq!(ev[i].candidate.id, "featured");
}

#[test]
fn duplicates_of_recent_wallpapers_are_rejected() {
    let http = FixtureHttp::new().route(&["a.jpg"], photo(640, 480, 60, 9));
    let (analysed, _) = pipeline::analyse_all(
        &http,
        vec![candidate("a", "https://x/a.jpg", true)],
        SCREEN,
        1,
    );
    let hash = analysed[0].1.dhash;
    let ev = pipeline::evaluate(
        analysed,
        &Model::default(),
        SCREEN,
        ToneFilter::Any,
        &[hash],
    );
    assert_eq!(ev[0].verdict, Err(Reject::Duplicate));
}

#[test]
fn broken_downloads_are_reported_not_fatal() {
    let http = FixtureHttp::new()
        .route(&["ok.jpg"], photo(640, 480, 60, 3))
        .route(&["bad.jpg"], b"<html>captcha</html>".to_vec());
    let cands = vec![
        candidate("ok", "https://x/ok.jpg", true),
        candidate("bad", "https://x/bad.jpg", true),
        candidate("missing", "https://x/missing.jpg", true),
    ];
    let (analysed, failed) = pipeline::analyse_all(&http, cands, SCREEN, 2);
    assert_eq!(analysed.len(), 1);
    let ids: Vec<_> = failed.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(ids, ["bad", "missing"]);
}

#[test]
fn render_produces_a_screen_sized_jpeg() {
    let http = FixtureHttp::new()
        .route(&["prev.jpg"], photo(640, 480, 60, 5))
        // Commons thumbs: preview is "960px-…", the render asks for a bigger step.
        .route(&["px-prev.jpg"], photo(1600, 1200, 60, 5));
    let mut c = candidate("c", "https://thumb.example/a/960px-prev.jpg", true);
    c.image_url = "https://upload.example/prev.jpg".into();
    let (analysed, _) = pipeline::analyse_all(&http, vec![c], SCREEN, 1);
    let ev = pipeline::evaluate(analysed, &Model::default(), SCREEN, ToneFilter::Any, &[]);
    let src = CommonsSource::new(&http);
    let jpeg = pipeline::render(&http, &src, &ev[0], SCREEN).unwrap();
    let out = pipeline::decode(&jpeg).unwrap();
    assert_eq!((out.width(), out.height()), (540, 1200));
}
