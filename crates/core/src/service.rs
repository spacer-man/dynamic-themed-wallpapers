//! Headless engine: everything the app does, without any UI or platform code.
//!
//! The Tauri commands call it from the UI; the Android background worker calls the same
//! functions over JNI. An [`Engine`] is just configuration — every call opens its own short-lived
//! database connection (SQLite WAL handles the app and the worker running side by side).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::analysis::Swatch;
use crate::error::{Error, Result};
use crate::http::{HttpClient, UreqClient};
use crate::pipeline::{self, Evaluated, Screen};
use crate::place::{Place, PlaceHit, Scope, SubPlace, Wikidata};
use crate::score::{Features, Reject, ToneFilter, EPSILON};
use crate::source::{
    Candidate, CandidateRequest, CommonsSource, Source, SourceKind, UnsplashSource,
};
use crate::store::{self, HistoryEntry, ImageCache, Store, DEFAULT_CACHE_BUDGET_BYTES};
use crate::theme::{self, Theme};

const KEY_SETTINGS: &str = "settings";
const KEY_PLACE: &str = "place";
const KEY_SUB_PLACES: &str = "sub_places";
const SUB_PLACES_TTL_SECS: i64 = 30 * 86_400;
/// How many recent dHashes new photos are compared against.
const RECENT_HASHES: u32 = 90;
/// Fetch → analyse rounds per prefetch before giving up.
const MAX_ROUNDS: u64 = 4;
const ANALYSIS_THREADS: usize = 4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaptionAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Where the caption goes on the wallpaper. `y` is the vertical centre of the text block as a
/// percentage of the screen height (0 = top, 100 = bottom).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptionLayout {
    pub y: u8,
    pub align: CaptionAlign,
}

/// Default vertical position: the bottom edge, as before the position became configurable.
pub const DEFAULT_CAPTION_Y: u8 = 94;
const CAPTION_Y_RANGE: (u8, u8) = (3, 97);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub tone: ToneFilter,
    /// Pick dark wallpapers when the system is dark and light ones when it's light
    /// (overrides `tone`), and make the app UI follow the system as well.
    pub follow_system_theme: bool,
    /// Override the automatic radius (km) for city-sized places.
    pub radius_km: Option<f64>,
    /// Local time of the daily change, "HH:MM".
    pub change_time: String,
    pub wifi_only: bool,
    pub cache_mb: u32,
    pub unsplash_enabled: bool,
    /// "auto", "ru" or "en".
    pub lang: String,
    /// Draw the place name and credit on the wallpaper at all.
    pub caption_enabled: bool,
    /// See [`CaptionLayout::y`]. Lets the user keep the text clear of home screen icons.
    pub caption_y: u8,
    pub caption_align: CaptionAlign,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            tone: ToneFilter::Any,
            follow_system_theme: false,
            radius_km: None,
            change_time: "08:00".into(),
            wifi_only: true,
            cache_mb: (DEFAULT_CACHE_BUDGET_BYTES / 1024 / 1024) as u32,
            unsplash_enabled: false,
            lang: "auto".into(),
            caption_enabled: true,
            caption_y: DEFAULT_CAPTION_Y,
            caption_align: CaptionAlign::Left,
        }
    }
}

impl Settings {
    /// Fix up values from an untrusted source (the UI, an old file).
    pub fn sanitized(mut self) -> Self {
        if parse_hhmm(&self.change_time).is_none() {
            self.change_time = Settings::default().change_time;
        }
        self.cache_mb = self.cache_mb.clamp(20, 2000);
        self.radius_km = self
            .radius_km
            .filter(|r| r.is_finite())
            .map(|r| r.clamp(crate::place::MIN_RADIUS_KM, crate::place::MAX_RADIUS_KM));
        if !["auto", "ru", "en"].contains(&self.lang.as_str()) {
            self.lang = "auto".into();
        }
        self.caption_y = self.caption_y.clamp(CAPTION_Y_RANGE.0, CAPTION_Y_RANGE.1);
        self
    }

    pub fn caption_layout(&self) -> CaptionLayout {
        CaptionLayout {
            y: self.caption_y,
            align: self.caption_align,
        }
    }

    pub fn effective_tone(&self, system_dark: bool) -> ToneFilter {
        if self.follow_system_theme {
            if system_dark {
                ToneFilter::Dark
            } else {
                ToneFilter::Light
            }
        } else {
            self.tone
        }
    }
}

/// "HH:MM" → (hour, minute).
pub fn parse_hhmm(s: &str) -> Option<(u8, u8)> {
    let (h, m) = s.split_once(':')?;
    let (h, m): (u8, u8) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60 && s.len() == 5).then_some((h, m))
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PrefetchReport {
    pub added: u32,
    pub considered: u32,
    pub rejected: Vec<(Reject, u32)>,
    pub failed_downloads: u32,
}

impl PrefetchReport {
    fn reject(&mut self, r: Reject) {
        match self.rejected.iter_mut().find(|(k, _)| *k == r) {
            Some((_, n)) => *n += 1,
            None => self.rejected.push((r, 1)),
        }
    }
}

/// What the "Today" screen shows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CurrentWallpaper {
    pub entry: HistoryEntry,
    pub theme: Theme,
    /// Rendered JPEG in the cache, if it is still there.
    pub image_path: Option<PathBuf>,
}

/// A wallpaper that was just chosen; the platform layer installs `image_path`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shown {
    pub entry: HistoryEntry,
    pub image_path: PathBuf,
    /// "<place>" caption line and credit, ready to draw on the wallpaper.
    pub caption: String,
    pub credit: String,
    /// Where to draw them. Both strings are empty when the user turned the caption off.
    pub layout: CaptionLayout,
}

impl Shown {
    fn new(settings: &Settings, entry: HistoryEntry, image_path: PathBuf) -> Self {
        let (caption, credit) = if settings.caption_enabled {
            (caption_for(&entry.candidate), entry.candidate.credit_line())
        } else {
            (String::new(), String::new())
        };
        Self {
            entry,
            image_path,
            caption,
            credit,
            layout: settings.caption_layout(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CacheInfo {
    pub bytes: u64,
    pub budget_bytes: u64,
    pub ready: u32,
}

#[derive(Serialize, Deserialize)]
struct StoredSubPlaces {
    qid: String,
    fetched_at: i64,
    places: Vec<SubPlace>,
}

#[derive(Clone)]
pub struct Engine {
    db_path: PathBuf,
    cache_dir: PathBuf,
    http: Arc<dyn HttpClient>,
    unsplash_key: Option<String>,
}

impl Engine {
    pub fn new(db_path: impl Into<PathBuf>, cache_dir: impl Into<PathBuf>) -> Self {
        Self::with_http(db_path, cache_dir, Arc::new(UreqClient::new()))
    }

    pub fn with_http(
        db_path: impl Into<PathBuf>,
        cache_dir: impl Into<PathBuf>,
        http: Arc<dyn HttpClient>,
    ) -> Self {
        Self {
            db_path: db_path.into(),
            cache_dir: cache_dir.into(),
            http,
            unsplash_key: None,
        }
    }

    /// The Unsplash key is held in memory only; the platform layer keeps it in secure storage.
    pub fn set_unsplash_key(&mut self, key: Option<String>) {
        self.unsplash_key = key.filter(|k| !k.trim().is_empty());
    }

    pub fn store(&self) -> Result<Store> {
        if let Some(dir) = self.db_path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Store::open(&self.db_path)
    }

    fn cache<'a>(&self, store: &'a Store, settings: &Settings) -> Result<ImageCache<'a>> {
        ImageCache::new(
            store,
            &self.cache_dir,
            u64::from(settings.cache_mb) * 1024 * 1024,
        )
    }

    // --- settings & place -----------------------------------------------------------------

    pub fn settings(&self) -> Result<Settings> {
        Ok(self
            .store()?
            .get_json::<Settings>(KEY_SETTINGS)?
            .unwrap_or_default()
            .sanitized())
    }

    pub fn save_settings(&self, settings: Settings) -> Result<Settings> {
        let settings = settings.sanitized();
        let old = self.settings()?;
        let store = self.store()?;
        store.set_json(KEY_SETTINGS, &settings)?;
        // The queue was assembled under the old search conditions. (Tone is not listed: it is
        // re-checked when a wallpaper is taken from the queue.)
        if old.radius_km != settings.radius_km || old.unsplash_enabled != settings.unsplash_enabled
        {
            self.cache(&store, &settings)?.drop_ready()?;
        }
        Ok(settings)
    }

    pub fn selected_place(&self) -> Result<Option<Place>> {
        self.store()?.get_json(KEY_PLACE)
    }

    pub fn search_places(&self, query: &str, lang: &str) -> Result<Vec<PlaceHit>> {
        Wikidata::new(self.http.as_ref(), lang).search(query, 10)
    }

    /// Resolve and remember the place. Photos from the previous place stay cached but the
    /// prefetched queue is cleared — it would show the wrong place.
    pub fn select_place(&self, qid: &str, lang: &str) -> Result<Place> {
        let place = Wikidata::new(self.http.as_ref(), lang).resolve(qid)?;
        let store = self.store()?;
        store.set_json(KEY_PLACE, &place)?;
        let settings = self.settings()?;
        self.cache(&store, &settings)?.drop_ready()?;
        Ok(place)
    }

    fn scope_for(&self, place: &Place, settings: &Settings) -> Scope {
        match (settings.radius_km, place.coords) {
            (Some(km), Some(center)) if place.kind != crate::place::PlaceKind::Country => {
                Scope::Radius { center, km }
            }
            _ => place.default_scope(),
        }
    }

    fn sub_places(
        &self,
        store: &Store,
        place: &Place,
        lang: &str,
        now: i64,
    ) -> Result<Vec<SubPlace>> {
        let cached: Option<StoredSubPlaces> = store.get_json(KEY_SUB_PLACES)?;
        if let Some(c) = cached {
            if c.qid == place.qid
                && now - c.fetched_at < SUB_PLACES_TTL_SECS
                && !c.places.is_empty()
            {
                return Ok(c.places);
            }
        }
        let places = Wikidata::new(self.http.as_ref(), lang).sub_places(place, 40)?;
        store.set_json(
            KEY_SUB_PLACES,
            &StoredSubPlaces {
                qid: place.qid.clone(),
                fetched_at: now,
                places: places.clone(),
            },
        )?;
        Ok(places)
    }

    // --- picking --------------------------------------------------------------------------

    /// Fill the cache so that at least `target_ready` wallpapers are waiting.
    pub fn prefetch(
        &self,
        screen: Screen,
        system_dark: bool,
        target_ready: u32,
        now: i64,
    ) -> Result<PrefetchReport> {
        let store = self.store()?;
        let settings = self.settings()?;
        let place = self
            .selected_place()?
            .ok_or_else(|| Error::Config("no place selected yet".into()))?;
        let cache = self.cache(&store, &settings)?;
        cache.reconcile()?;
        let lang = if settings.lang == "auto" {
            "en"
        } else {
            settings.lang.as_str()
        };
        let scope = self.scope_for(&place, &settings);
        let tone = settings.effective_tone(system_dark);
        let model = store.load_model()?;
        let recent = store.recent_hashes(RECENT_HASHES)?;

        let commons = CommonsSource::new(self.http.as_ref());
        let unsplash = match (&self.unsplash_key, settings.unsplash_enabled) {
            (Some(key), true) => Some(UnsplashSource::new(self.http.as_ref(), key)?),
            _ => None,
        };

        let mut req = CandidateRequest::new(screen.width, screen.height);
        req.limit = 30;
        if scope == Scope::Hierarchy {
            req.sub_places = self.sub_places(&store, &place, lang, now)?;
        }

        let mut report = PrefetchReport::default();
        let mut last_error = None;
        let mut rng = fastrand::Rng::with_seed(now as u64);
        for round in 0..MAX_ROUNDS {
            if cache.ready_count_for(tone)? >= target_ready {
                break;
            }
            req.seed = (now as u64).wrapping_add(round);
            let mut candidates = Vec::new();
            for source in [
                Some(&commons as &dyn Source),
                unsplash.as_ref().map(|u| u as &dyn Source),
            ]
            .into_iter()
            .flatten()
            {
                match source.candidates(&place, &scope, &req) {
                    Ok(c) => candidates.extend(c),
                    Err(e) => last_error = Some(e),
                }
            }
            // Never re-pick what was shown or is already waiting.
            candidates.retain(|c| {
                !store.was_shown(&c.id).unwrap_or(false) && !cache.has(&c.id).unwrap_or(false)
            });
            if candidates.is_empty() {
                continue;
            }
            report.considered += candidates.len() as u32;
            let (analysed, failed) =
                pipeline::analyse_all(self.http.as_ref(), candidates, screen, ANALYSIS_THREADS);
            report.failed_downloads += failed.len() as u32;
            let mut evaluated = pipeline::evaluate(analysed, &model, screen, tone, &recent);
            for e in &evaluated {
                if let Err(r) = e.verdict {
                    report.reject(r);
                }
            }
            // Within one batch, also avoid near-identical frames of the same shoot.
            let mut taken: Vec<u64> = Vec::new();
            while cache.ready_count_for(tone)? < target_ready {
                let Some(i) = pipeline::choose(&evaluated, EPSILON, &mut rng) else {
                    break;
                };
                let e = evaluated.swap_remove(i);
                if taken
                    .iter()
                    .any(|&h| crate::analysis::hash::is_near_duplicate(h, e.analysis.dhash))
                {
                    continue;
                }
                match self.render_into(&cache, &commons, unsplash.as_ref(), &e, screen, now) {
                    Ok(()) => {
                        taken.push(e.analysis.dhash);
                        report.added += 1;
                    }
                    Err(err) => last_error = Some(err),
                }
            }
        }
        if report.added == 0 && cache.ready_count_for(tone)? == 0 {
            return Err(last_error.unwrap_or_else(|| {
                Error::NotFound("no suitable photo found for this place and filters".into())
            }));
        }
        Ok(report)
    }

    fn render_into(
        &self,
        cache: &ImageCache,
        commons: &CommonsSource,
        unsplash: Option<&UnsplashSource>,
        e: &Evaluated,
        screen: Screen,
        now: i64,
    ) -> Result<()> {
        let source: &dyn Source = match e.candidate.source {
            SourceKind::Commons => commons,
            SourceKind::Unsplash => {
                unsplash.ok_or_else(|| Error::Config("Unsplash is off".into()))?
            }
        };
        let jpeg = pipeline::render(self.http.as_ref(), source, e, screen)?;
        cache.put_ready(e, &jpeg, now)?;
        Ok(())
    }

    /// Take the next wallpaper (fetching if the queue is empty) and record it as shown.
    pub fn change_now(&self, screen: Screen, system_dark: bool, now: i64) -> Result<Shown> {
        let store = self.store()?;
        let settings = self.settings()?;
        let tone = settings.effective_tone(system_dark);
        {
            let cache = self.cache(&store, &settings)?;
            if cache.ready_count_for(tone)? == 0 {
                drop(cache);
                self.prefetch(screen, system_dark, 1, now)?;
            }
        }
        let cache = self.cache(&store, &settings)?;
        let ready = cache
            .take_next_ready(now, tone)?
            .ok_or_else(|| Error::NotFound("no wallpaper available".into()))?;
        self.finish_shown(
            &store,
            &settings,
            &ready.candidate,
            &ready.features,
            &ready.palette,
            ready.dhash,
            ready.path,
            now,
        )
    }

    /// Put a photo from the history back on the screen: same bookkeeping as a normal change
    /// (new history row, Unsplash ping), same sizing and caption. Uses the cached render when
    /// it is still on disk, otherwise downloads and renders the photo again from its stored URL.
    pub fn apply_from_history(&self, history_id: i64, screen: Screen, now: i64) -> Result<Shown> {
        let store = self.store()?;
        let settings = self.settings()?;
        let (entry, features, dhash) = store
            .history_item(history_id)?
            .ok_or_else(|| Error::NotFound("that photo is no longer in the history".into()))?;
        let cache = self.cache(&store, &settings)?;
        let path = match cache.path_of(&entry.candidate.id)? {
            Some(p) => p,
            None => {
                let commons = CommonsSource::new(self.http.as_ref());
                let unsplash = match (&self.unsplash_key, settings.unsplash_enabled) {
                    (Some(key), true) => Some(UnsplashSource::new(self.http.as_ref(), key)?),
                    _ => None,
                };
                // Only the crop plan is needed, so analyse the preview again; the verdict is
                // irrelevant for a photo the user picked by hand.
                let analysis = pipeline::analyse(self.http.as_ref(), &entry.candidate, screen)?;
                let e = Evaluated {
                    candidate: entry.candidate.clone(),
                    analysis,
                    features,
                    verdict: Ok(0.0),
                };
                self.render_into(&cache, &commons, unsplash.as_ref(), &e, screen, now)?;
                cache.path_of(&entry.candidate.id)?.ok_or_else(|| {
                    Error::Store("rendered wallpaper is missing from the cache".into())
                })?
            }
        };
        cache.mark_used(&entry.candidate.id, now)?;
        self.finish_shown(
            &store,
            &settings,
            &entry.candidate,
            &features,
            &entry.palette,
            dhash,
            path,
            now,
        )
    }

    /// Record `candidate` as the current wallpaper and build what the platform layer needs.
    #[allow(clippy::too_many_arguments)]
    fn finish_shown(
        &self,
        store: &Store,
        settings: &Settings,
        candidate: &Candidate,
        features: &Features,
        palette: &[Swatch],
        dhash: u64,
        path: PathBuf,
        now: i64,
    ) -> Result<Shown> {
        store.record_shown(candidate, features, palette, dhash, now)?;
        let entry = store
            .current()?
            .ok_or_else(|| Error::Store("history is empty".into()))?;

        // Unsplash API terms: tell them when a photo is actually used. Best effort.
        if candidate.source == SourceKind::Unsplash {
            if let (Some(key), true) = (&self.unsplash_key, settings.unsplash_enabled) {
                if let Ok(u) = UnsplashSource::new(self.http.as_ref(), key) {
                    let _ = u.report_used(candidate);
                }
            }
        }
        Ok(Shown::new(settings, entry, path))
    }

    /// The wallpaper that is currently set, ready to be installed again (for example after the
    /// caption position changed). `None` when nothing was shown yet or its file is gone.
    pub fn current_shown(&self) -> Result<Option<Shown>> {
        let store = self.store()?;
        let Some(entry) = store.current()? else {
            return Ok(None);
        };
        let settings = self.settings()?;
        let path = self
            .cache(&store, &settings)?
            .path_of(&entry.candidate.id)?;
        Ok(path.map(|p| Shown::new(&settings, entry, p)))
    }

    /// Top up the queue only if it runs low (called by the background job on Wi-Fi + charging).
    pub fn prefetch_if_low(
        &self,
        screen: Screen,
        system_dark: bool,
        now: i64,
    ) -> Result<Option<PrefetchReport>> {
        let store = self.store()?;
        let settings = self.settings()?;
        let ready = self.cache(&store, &settings)?.ready_count()?;
        if ready >= 3 {
            return Ok(None);
        }
        self.prefetch(screen, system_dark, 7, now).map(Some)
    }

    // --- reading state --------------------------------------------------------------------

    /// `dark`: whether the UI is dark (decides the theme variant).
    pub fn current_wallpaper(&self, dark: bool) -> Result<Option<CurrentWallpaper>> {
        let store = self.store()?;
        let Some(entry) = store.current()? else {
            return Ok(None);
        };
        let settings = self.settings()?;
        let image_path = self
            .cache(&store, &settings)?
            .path_of(&entry.candidate.id)?;
        let theme = theme::from_palette(&entry.palette, dark);
        Ok(Some(CurrentWallpaper {
            entry,
            theme,
            image_path,
        }))
    }

    /// Theme for an arbitrary palette (used by the UI before any photo exists).
    pub fn theme_for(&self, palette: &[Swatch], dark: bool) -> Theme {
        theme::from_palette(palette, dark)
    }

    pub fn feedback(&self, candidate_id: &str, liked: bool, now: i64) -> Result<()> {
        self.store()?.record_feedback(candidate_id, liked, now)?;
        Ok(())
    }

    pub fn history(&self, limit: u32, offset: u32) -> Result<Vec<HistoryEntry>> {
        self.store()?.history(limit.clamp(1, 200), offset)
    }

    pub fn cache_info(&self) -> Result<CacheInfo> {
        let store = self.store()?;
        let settings = self.settings()?;
        let cache = self.cache(&store, &settings)?;
        Ok(CacheInfo {
            bytes: cache.total_bytes()?,
            budget_bytes: u64::from(settings.cache_mb) * 1024 * 1024,
            ready: cache.ready_count()?,
        })
    }

    pub fn clear_cache(&self) -> Result<()> {
        let store = self.store()?;
        let settings = self.settings()?;
        self.cache(&store, &settings)?.clear()
    }

    /// Wipe history, feedback, the learned model and the cache. Settings and place stay.
    pub fn forget_all(&self) -> Result<()> {
        let store = self.store()?;
        let settings = self.settings()?;
        self.cache(&store, &settings)?.clear()?;
        store.forget_all()
    }

    pub fn now() -> i64 {
        store::now()
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }
}

/// Text drawn on the wallpaper: the place, plus the photo's own title when it adds something.
pub fn caption_for(c: &Candidate) -> String {
    let place = c.place_label.trim();
    let title = c.title.trim();
    if title.is_empty() || place.eq_ignore_ascii_case(title) {
        place.to_owned()
    } else {
        format!("{place} · {}", crate::text::truncate(title, 60))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hhmm() {
        assert_eq!(parse_hhmm("08:05"), Some((8, 5)));
        assert_eq!(parse_hhmm("24:00"), None);
        assert_eq!(parse_hhmm("8:05"), None);
        assert_eq!(parse_hhmm("nope"), None);
    }

    #[test]
    fn settings_are_sanitized() {
        let s = Settings {
            change_time: "99:99".into(),
            cache_mb: 5,
            radius_km: Some(9999.0),
            lang: "xx".into(),
            ..Default::default()
        }
        .sanitized();
        assert_eq!(s.change_time, "08:00");
        assert_eq!(s.cache_mb, 20);
        assert_eq!(s.radius_km, Some(crate::place::MAX_RADIUS_KM));
        assert_eq!(s.lang, "auto");
        assert_eq!(
            Settings {
                radius_km: Some(f64::NAN),
                ..Default::default()
            }
            .sanitized()
            .radius_km,
            None
        );
    }

    #[test]
    fn follow_system_overrides_tone() {
        let s = Settings {
            tone: ToneFilter::Light,
            follow_system_theme: true,
            ..Default::default()
        };
        assert_eq!(s.effective_tone(true), ToneFilter::Dark);
        assert_eq!(s.effective_tone(false), ToneFilter::Light);
        let s = Settings {
            tone: ToneFilter::Light,
            ..Default::default()
        };
        assert_eq!(s.effective_tone(true), ToneFilter::Light);
    }

    #[test]
    fn partial_settings_json_uses_defaults() {
        let s: Settings = serde_json::from_str(r#"{"tone":"dark"}"#).unwrap();
        assert_eq!(s.tone, ToneFilter::Dark);
        assert_eq!(s.change_time, "08:00");
    }
}
