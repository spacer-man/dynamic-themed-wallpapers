use std::path::PathBuf;

use wallpaper_core::analysis::{Analysis, CropRect, Swatch, Tone};
use wallpaper_core::pipeline::Evaluated;
use wallpaper_core::score::{Features, Model, Reject, ToneFilter, N_FEATURES, PRIOR_WEIGHTS};
use wallpaper_core::source::{Candidate, Curation, License, SourceKind};
use wallpaper_core::store::{ImageCache, Store};

/// Fresh directory under `target/` (the sandbox can't read back from /tmp).
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("store-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn candidate(id: &str) -> Candidate {
    Candidate {
        source: SourceKind::Commons,
        id: id.into(),
        title: format!("Title of {id}"),
        description: None,
        page_url: "https://commons.wikimedia.org/wiki/File:X.jpg".into(),
        image_url: "https://upload.wikimedia.org/x.jpg".into(),
        preview_url: "https://upload.wikimedia.org/x-960.jpg".into(),
        width: 4000,
        height: 3000,
        author: "Tester".into(),
        author_url: None,
        license: License {
            short_name: "CC BY 4.0".into(),
            url: None,
            attribution_required: true,
        },
        curation: Curation::default(),
        coords: None,
        place_label: "Kyoto".into(),
    }
}

fn features(v: f32) -> Features {
    Features([v; N_FEATURES])
}

fn evaluated(id: &str, score: f32, dhash: u64) -> Evaluated {
    Evaluated {
        candidate: candidate(id),
        analysis: Analysis {
            crop: CropRect::FULL,
            crop_keep: 1.0,
            tone: Tone::Dark,
            luma_median: 0.2,
            luma_p10: 0.1,
            luma_p90: 0.4,
            contrast: 0.2,
            colorfulness: 30.0,
            sharpness: 200.0,
            top_band_busyness: 0.01,
            dock_busyness: 0.01,
            highlight_clip: 0.0,
            shadow_clip: 0.0,
            entropy: 5.0,
            palette: vec![Swatch {
                rgb: [10, 20, 30],
                weight: 1.0,
            }],
            palette_spread: 0.05,
            dhash,
        },
        features: features(0.5),
        verdict: Ok(score),
    }
}

#[test]
fn settings_roundtrip_and_overwrite() {
    let s = Store::open_in_memory().unwrap();
    assert_eq!(s.get_setting("tone").unwrap(), None);
    s.set_setting("tone", "dark").unwrap();
    s.set_setting("tone", "light").unwrap();
    assert_eq!(s.get_setting("tone").unwrap().as_deref(), Some("light"));
    s.set_json("place", &vec![1, 2, 3]).unwrap();
    assert_eq!(
        s.get_json::<Vec<i32>>("place").unwrap(),
        Some(vec![1, 2, 3])
    );
    s.delete_setting("tone").unwrap();
    assert_eq!(s.get_setting("tone").unwrap(), None);
}

#[test]
fn history_order_hashes_and_current() {
    let s = Store::open_in_memory().unwrap();
    for (i, id) in ["a", "b", "c"].iter().enumerate() {
        s.record_shown(
            &candidate(id),
            &features(0.5),
            &[],
            100 + i as u64,
            1000 + i as i64,
        )
        .unwrap();
    }
    assert_eq!(s.history_len().unwrap(), 3);
    assert_eq!(s.recent_hashes(2).unwrap(), [102, 101]);
    assert!(s.was_shown("b").unwrap() && !s.was_shown("zzz").unwrap());
    let h = s.history(10, 0).unwrap();
    assert_eq!(
        h.iter()
            .map(|e| e.candidate.id.as_str())
            .collect::<Vec<_>>(),
        ["c", "b", "a"]
    );
    assert_eq!(s.current().unwrap().unwrap().candidate.id, "c");
    assert_eq!(s.history(1, 1).unwrap()[0].candidate.id, "b");
    // High bit of dHash survives the i64 round trip.
    s.record_shown(&candidate("d"), &features(0.5), &[], u64::MAX - 5, 2000)
        .unwrap();
    assert_eq!(s.recent_hashes(1).unwrap(), [u64::MAX - 5]);
}

#[test]
fn prune_keeps_the_newest_few() {
    let s = Store::open_in_memory().unwrap();
    for i in 0..10 {
        s.record_shown(
            &candidate(&format!("p{i}")),
            &features(0.5),
            &[],
            i,
            i as i64 * 86_400,
        )
        .unwrap();
    }
    // now = day 10; keep 3 days → cutoff day 7, but always keep the newest 5.
    let removed = s.prune_history(3, 5, 10 * 86_400).unwrap();
    assert_eq!(removed, 5);
    assert_eq!(s.history_len().unwrap(), 5);
    assert_eq!(s.current().unwrap().unwrap().candidate.id, "p9");
}

#[test]
fn feedback_trains_the_model_once_per_verdict() {
    let mut s = Store::open_in_memory().unwrap();
    s.record_shown(&candidate("x"), &features(1.0), &[], 1, 10)
        .unwrap();
    assert_eq!(s.load_model().unwrap(), Model::default());

    let m1 = s.record_feedback("x", true, 20).unwrap();
    assert_eq!(m1.updates, 1);
    assert!(
        m1.weights.iter().zip(PRIOR_WEIGHTS).all(|(w, p)| *w > p),
        "liking an all-ones photo raises weights"
    );
    // Double tap: no second update.
    assert_eq!(s.record_feedback("x", true, 21).unwrap().updates, 1);
    // Changing the verdict is a new lesson.
    assert_eq!(s.record_feedback("x", false, 22).unwrap().updates, 2);
    assert_eq!(s.load_model().unwrap().updates, 2);
    assert_eq!(s.feedback_counts().unwrap(), (0, 1));
    assert_eq!(s.history(1, 0).unwrap()[0].liked, Some(false));

    assert!(s.record_feedback("never-shown", true, 30).is_err());
    s.reset_model().unwrap();
    assert_eq!(s.load_model().unwrap(), Model::default());
}

#[test]
fn unreadable_model_falls_back_to_prior() {
    let s = Store::open_in_memory().unwrap();
    wallpaper_core::store::testing::corrupt_model(&s);
    assert_eq!(s.load_model().unwrap(), Model::default());
}

#[test]
fn file_database_persists_and_migrates() {
    let dir = scratch("persist");
    let path = dir.join("app.db");
    {
        let mut s = Store::open(&path).unwrap();
        s.set_setting("k", "v").unwrap();
        s.record_shown(&candidate("a"), &features(0.5), &[], 7, 1)
            .unwrap();
        s.record_feedback("a", true, 2).unwrap();
    }
    let s = Store::open(&path).unwrap();
    assert_eq!(s.get_setting("k").unwrap().as_deref(), Some("v"));
    assert_eq!(s.load_model().unwrap().updates, 1);
    assert_eq!(s.history_len().unwrap(), 1);
}

#[test]
fn newer_schema_is_refused() {
    let dir = scratch("newer");
    let path = dir.join("app.db");
    Store::open(&path).unwrap();
    wallpaper_core::store::testing::set_user_version(
        &path,
        wallpaper_core::store::SCHEMA_VERSION + 1,
    );
    let err = Store::open(&path).err().expect("must refuse");
    assert!(err.to_string().contains("newer"), "{err}");
}

#[test]
fn cache_hands_out_best_ready_first_and_marks_used() {
    let dir = scratch("ready");
    let s = Store::open_in_memory().unwrap();
    let cache = ImageCache::new(&s, dir.join("img"), 10_000).unwrap();
    cache
        .put_ready(&evaluated("low", 0.5, 1), b"jpeg-low", 100)
        .unwrap();
    cache
        .put_ready(&evaluated("high", 0.9, 2), b"jpeg-high", 101)
        .unwrap();
    assert_eq!(cache.ready_count().unwrap(), 2);
    assert_eq!(cache.total_bytes().unwrap(), 17); // 8 + 9 bytes

    let first = cache
        .take_next_ready(200, ToneFilter::Any)
        .unwrap()
        .unwrap();
    assert_eq!(first.candidate.id, "high");
    assert_eq!(first.dhash, 2);
    assert_eq!(first.palette.len(), 1);
    assert_eq!(std::fs::read(&first.path).unwrap(), b"jpeg-high");
    assert_eq!(cache.ready_count().unwrap(), 1);
    assert_eq!(
        cache
            .take_next_ready(201, ToneFilter::Any)
            .unwrap()
            .unwrap()
            .candidate
            .id,
        "low"
    );
    assert!(cache
        .take_next_ready(202, ToneFilter::Any)
        .unwrap()
        .is_none());
}

#[test]
fn rejected_photos_are_never_cached() {
    let dir = scratch("reject");
    let s = Store::open_in_memory().unwrap();
    let cache = ImageCache::new(&s, dir.join("img"), 10_000).unwrap();
    let mut e = evaluated("bad", 0.9, 1);
    e.verdict = Err(Reject::Blurry);
    assert!(cache.put_ready(&e, b"x", 1).is_err());
    assert_eq!(cache.total_bytes().unwrap(), 0);
}

#[test]
fn eviction_drops_used_first_then_lowest_score_and_keeps_current() {
    let dir = scratch("evict");
    let s = Store::open_in_memory().unwrap();
    // Budget fits exactly three 10-byte files.
    let cache = ImageCache::new(&s, dir.join("img"), 30).unwrap();
    let jpeg = [0u8; 10];
    cache
        .put_ready(&evaluated("old-used", 0.9, 1), &jpeg, 1)
        .unwrap();
    cache.take_next_ready(2, ToneFilter::Any).unwrap(); // old-used → used @2
    cache
        .put_ready(&evaluated("current", 0.9, 2), &jpeg, 3)
        .unwrap();
    cache.take_next_ready(4, ToneFilter::Any).unwrap(); // current → used @4
    cache
        .put_ready(&evaluated("ready-low", 0.4, 3), &jpeg, 5)
        .unwrap();
    assert_eq!(cache.total_bytes().unwrap(), 30);

    // One more prefetch → over budget → the old used file goes, not the current one.
    cache
        .put_ready(&evaluated("ready-high", 0.8, 4), &jpeg, 6)
        .unwrap();
    assert_eq!(cache.total_bytes().unwrap(), 30);
    // Next prefetch: no other used file left except current (protected) → lowest-scored ready goes.
    cache
        .put_ready(&evaluated("ready-mid", 0.6, 5), &jpeg, 7)
        .unwrap();
    assert_eq!(cache.total_bytes().unwrap(), 30);
    let order: Vec<_> = std::iter::from_fn(|| cache.take_next_ready(10, ToneFilter::Any).unwrap())
        .map(|r| r.candidate.id)
        .collect();
    assert_eq!(order, ["ready-high", "ready-mid"], "ready-low was evicted");
    assert_eq!(
        std::fs::read_dir(cache.dir()).unwrap().count(),
        3,
        "files match the index"
    );
}

#[test]
fn reconcile_repairs_index_and_orphans() {
    let dir = scratch("reconcile");
    let s = Store::open_in_memory().unwrap();
    let cache = ImageCache::new(&s, dir.join("img"), 10_000).unwrap();
    let kept = cache
        .put_ready(&evaluated("kept", 0.9, 1), b"k", 1)
        .unwrap();
    let lost = cache
        .put_ready(&evaluated("lost", 0.8, 2), b"l", 1)
        .unwrap();
    std::fs::remove_file(&lost).unwrap(); // the OS cleared one file
    std::fs::write(cache.dir().join("stray.jpg"), b"?").unwrap();

    assert_eq!(cache.reconcile().unwrap(), 2);
    assert!(kept.exists() && !cache.dir().join("stray.jpg").exists());
    assert_eq!(cache.ready_count().unwrap(), 1);
    // A ready row whose file vanished later is skipped when taking.
    std::fs::remove_file(&kept).unwrap();
    assert!(cache.take_next_ready(5, ToneFilter::Any).unwrap().is_none());
}

#[test]
fn clear_and_forget() {
    let dir = scratch("clear");
    let mut s = Store::open_in_memory().unwrap();
    s.set_setting("keep", "me").unwrap();
    s.record_shown(&candidate("a"), &features(0.5), &[], 1, 1)
        .unwrap();
    s.record_feedback("a", true, 2).unwrap();
    {
        let cache = ImageCache::new(&s, dir.join("img"), 10_000).unwrap();
        cache.put_ready(&evaluated("z", 0.9, 1), b"z", 1).unwrap();
        cache.clear().unwrap();
        assert_eq!(cache.total_bytes().unwrap(), 0);
        assert_eq!(std::fs::read_dir(cache.dir()).unwrap().count(), 0);
    }
    s.forget_all().unwrap();
    assert_eq!(s.history_len().unwrap(), 0);
    assert_eq!(s.feedback_counts().unwrap(), (0, 0));
    assert_eq!(s.load_model().unwrap().updates, 0);
    assert_eq!(s.get_setting("keep").unwrap().as_deref(), Some("me"));
}

#[test]
fn queue_respects_the_tone_filter_when_taking() {
    let dir = scratch("tone");
    let s = Store::open_in_memory().unwrap();
    let cache = ImageCache::new(&s, dir.join("img"), 10_000).unwrap();
    let mut dark = evaluated("dark", 0.5, 1);
    dark.analysis.tone = Tone::Dark;
    let mut light = evaluated("light", 0.9, 2);
    light.analysis.tone = Tone::Light;
    cache.put_ready(&dark, b"d", 1).unwrap();
    cache.put_ready(&light, b"l", 2).unwrap();

    assert_eq!(cache.ready_count().unwrap(), 2);
    assert_eq!(cache.ready_count_for(ToneFilter::Dark).unwrap(), 1);
    assert_eq!(cache.ready_count_for(ToneFilter::Light).unwrap(), 1);

    // The better-scored light one must not be handed out under a dark filter.
    let got = cache.take_next_ready(5, ToneFilter::Dark).unwrap().unwrap();
    assert_eq!(got.candidate.id, "dark");
    assert!(cache
        .take_next_ready(6, ToneFilter::Dark)
        .unwrap()
        .is_none());
    assert_eq!(
        cache
            .take_next_ready(7, ToneFilter::Any)
            .unwrap()
            .unwrap()
            .candidate
            .id,
        "light"
    );
}
