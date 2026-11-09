//! Tauri shell. Every command is a thin wrapper over `wallpaper_core::service::Engine`;
//! blocking work (network, image decoding, SQLite) runs on the blocking thread pool so the UI
//! never freezes.

mod wallpaper_bridge;
pub mod worker;

use base64::Engine as _;
use serde::Serialize;
use tauri::Manager;
use wallpaper_core::analysis::Swatch;
use wallpaper_core::pipeline::Screen;
use wallpaper_core::service::{CacheInfo, Engine, Settings, SourceStatus};
use wallpaper_core::source::SourceKind;
use wallpaper_core::store::HistoryEntry;
use wallpaper_core::theme::Theme;
use wallpaper_core::{Error, Place, PlaceHit, Subjects};

/// Error shape the UI can switch on (and translate) without parsing messages.
#[derive(Debug, Serialize)]
pub struct CmdError {
    kind: &'static str,
    message: String,
}

impl From<Error> for CmdError {
    fn from(e: Error) -> Self {
        let kind = match &e {
            Error::Status { status, .. } if *status != 429 && *status < 500 => "refused",
            Error::Network(_) | Error::Status { .. } => "network",
            Error::NotFound(_) => "not_found",
            Error::Config(_) => "config",
            Error::Parse { .. } => "parse",
            Error::Image(_) => "image",
            Error::Store(_) | Error::Io(_) => "storage",
        };
        CmdError {
            kind,
            message: e.to_string(),
        }
    }
}

type CmdResult<T> = Result<T, CmdError>;

/// Run blocking engine work off the async runtime's threads.
async fn blocking<T, F>(f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, Error> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| CmdError {
            kind: "internal",
            message: e.to_string(),
        })?
        .map_err(CmdError::from)
}

/// Sent to the UI: the rendered JPEG inlined as a data URL (no filesystem access needed).
#[derive(Debug, Serialize)]
struct WallpaperDto {
    entry: HistoryEntry,
    theme: Theme,
    image: Option<String>,
}

fn to_dto(cur: wallpaper_core::service::CurrentWallpaper) -> Result<WallpaperDto, Error> {
    let image = match &cur.image_path {
        Some(p) => {
            let bytes = std::fs::read(p)?;
            Some(format!(
                "data:image/jpeg;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            ))
        }
        None => None,
    };
    Ok(WallpaperDto {
        entry: cur.entry,
        theme: cur.theme,
        image,
    })
}

#[derive(Debug, serde::Deserialize)]
struct ScreenDto {
    width: u32,
    height: u32,
}

impl ScreenDto {
    /// Portrait, within sane bounds. A desktop window reports a landscape monitor; wallpapers
    /// for phones are portrait either way.
    fn to_screen(&self) -> Screen {
        let (a, b) = (self.width.clamp(320, 8192), self.height.clamp(320, 8192));
        Screen {
            width: a.min(b),
            height: a.max(b),
        }
    }
}

#[tauri::command]
async fn get_settings(engine: tauri::State<'_, Engine>) -> CmdResult<Settings> {
    let e = engine.inner().clone();
    blocking(move || e.settings()).await
}

#[tauri::command]
async fn save_settings(
    app: tauri::AppHandle,
    engine: tauri::State<'_, Engine>,
    settings: Settings,
) -> CmdResult<Settings> {
    let e = engine.inner().clone();
    blocking(move || {
        let saved = e.save_settings(settings)?;
        app.state::<wallpaper_bridge::Bridge>().schedule(
            &saved.change_time,
            saved.wifi_only,
            saved.schedule_enabled,
        )?;
        Ok(saved)
    })
    .await
}

#[tauri::command]
async fn selected_place(engine: tauri::State<'_, Engine>) -> CmdResult<Option<Place>> {
    let e = engine.inner().clone();
    blocking(move || e.selected_place()).await
}

#[tauri::command]
async fn search_places(
    engine: tauri::State<'_, Engine>,
    query: String,
    lang: String,
) -> CmdResult<Vec<PlaceHit>> {
    let e = engine.inner().clone();
    blocking(move || e.search_places(&query, &lang)).await
}

#[tauri::command]
async fn select_place(
    engine: tauri::State<'_, Engine>,
    qid: String,
    lang: String,
) -> CmdResult<Place> {
    let e = engine.inner().clone();
    blocking(move || e.select_place(&qid, &lang)).await
}

#[tauri::command]
async fn subjects(engine: tauri::State<'_, Engine>) -> CmdResult<Subjects> {
    let e = engine.inner().clone();
    blocking(move || e.subjects()).await
}

#[tauri::command]
async fn add_query_subject(engine: tauri::State<'_, Engine>, text: String) -> CmdResult<Subjects> {
    let e = engine.inner().clone();
    blocking(move || e.add_query(&text)).await
}

/// Save a Pinterest board from the address the user pasted; selects Pinterest as the source.
#[tauri::command]
async fn add_board_subject(engine: tauri::State<'_, Engine>, url: String) -> CmdResult<Subjects> {
    let e = engine.inner().clone();
    blocking(move || e.add_board(&url)).await
}

#[tauri::command]
async fn update_query_subject(
    engine: tauri::State<'_, Engine>,
    id: String,
    text: String,
) -> CmdResult<Subjects> {
    let e = engine.inner().clone();
    blocking(move || e.update_query(&id, &text)).await
}

#[tauri::command]
async fn select_subject(engine: tauri::State<'_, Engine>, id: String) -> CmdResult<Subjects> {
    let e = engine.inner().clone();
    blocking(move || e.select_subject(&id)).await
}

#[tauri::command]
async fn delete_subject(engine: tauri::State<'_, Engine>, id: String) -> CmdResult<Subjects> {
    let e = engine.inner().clone();
    blocking(move || e.delete_subject(&id)).await
}

/// What is known about each source without touching the network.
#[tauri::command]
async fn source_statuses(engine: tauri::State<'_, Engine>) -> CmdResult<Vec<SourceStatus>> {
    let e = engine.inner().clone();
    blocking(move || e.source_statuses()).await
}

/// One lightweight request to the source (short timeout) and its updated status.
#[tauri::command]
async fn check_source(
    engine: tauri::State<'_, Engine>,
    source: SourceKind,
) -> CmdResult<SourceStatus> {
    let e = engine.inner().clone();
    blocking(move || e.check_source(source, Engine::now())).await
}

/// Save (or, with no key, remove) the user's API key for a source. Never echoed back.
#[tauri::command]
async fn set_source_key(
    engine: tauri::State<'_, Engine>,
    source: SourceKind,
    key: Option<String>,
) -> CmdResult<()> {
    let e = engine.inner().clone();
    blocking(move || e.set_source_key(source, key)).await
}

#[tauri::command]
async fn current_wallpaper(
    engine: tauri::State<'_, Engine>,
    dark: bool,
) -> CmdResult<Option<WallpaperDto>> {
    let e = engine.inner().clone();
    blocking(move || e.current_wallpaper(dark)?.map(to_dto).transpose()).await
}

#[tauri::command]
async fn change_now(
    app: tauri::AppHandle,
    engine: tauri::State<'_, Engine>,
    system_dark: bool,
    screen: ScreenDto,
    source: Option<SourceKind>,
) -> CmdResult<Option<WallpaperDto>> {
    let e = engine.inner().clone();
    blocking(move || {
        // `source` is a one-time fallback chosen by the user after a failure; it is not saved.
        let shown =
            e.change_now_with_source(screen.to_screen(), system_dark, Engine::now(), source)?;
        app.state::<wallpaper_bridge::Bridge>()
            .set_wallpaper(&shown)?;
        e.current_wallpaper(system_dark)?.map(to_dto).transpose()
    })
    .await
}

/// Install the current photo again, e.g. after the caption position changed. No new photo is taken.
#[tauri::command]
async fn reapply_wallpaper(
    app: tauri::AppHandle,
    engine: tauri::State<'_, Engine>,
) -> CmdResult<bool> {
    let e = engine.inner().clone();
    blocking(move || match e.current_shown()? {
        Some(shown) => {
            app.state::<wallpaper_bridge::Bridge>()
                .set_wallpaper(&shown)?;
            Ok(true)
        }
        None => Ok(false),
    })
    .await
}

/// Set a photo from the history as the wallpaper again (cached render, or re-downloaded).
#[tauri::command]
async fn apply_from_history(
    app: tauri::AppHandle,
    engine: tauri::State<'_, Engine>,
    history_id: i64,
    system_dark: bool,
    screen: ScreenDto,
) -> CmdResult<Option<WallpaperDto>> {
    let e = engine.inner().clone();
    blocking(move || {
        let shown = e.apply_from_history(history_id, screen.to_screen(), Engine::now())?;
        app.state::<wallpaper_bridge::Bridge>()
            .set_wallpaper(&shown)?;
        e.current_wallpaper(system_dark)?.map(to_dto).transpose()
    })
    .await
}

#[tauri::command]
async fn feedback(
    engine: tauri::State<'_, Engine>,
    candidate_id: String,
    liked: bool,
) -> CmdResult<()> {
    let e = engine.inner().clone();
    blocking(move || e.feedback(&candidate_id, liked, Engine::now())).await
}

#[tauri::command]
async fn history(
    engine: tauri::State<'_, Engine>,
    limit: u32,
    offset: u32,
) -> CmdResult<Vec<HistoryEntry>> {
    let e = engine.inner().clone();
    blocking(move || e.history(limit, offset)).await
}

#[tauri::command]
async fn cache_info(engine: tauri::State<'_, Engine>) -> CmdResult<CacheInfo> {
    let e = engine.inner().clone();
    blocking(move || e.cache_info()).await
}

#[tauri::command]
async fn clear_cache(engine: tauri::State<'_, Engine>) -> CmdResult<()> {
    let e = engine.inner().clone();
    blocking(move || e.clear_cache()).await
}

#[tauri::command]
async fn forget_all(engine: tauri::State<'_, Engine>) -> CmdResult<()> {
    let e = engine.inner().clone();
    blocking(move || e.forget_all()).await
}

/// The screen a notification asked for (`"sources"`), consumed on read.
#[tauri::command]
async fn launch_route(app: tauri::AppHandle) -> CmdResult<Option<String>> {
    blocking(move || app.state::<wallpaper_bridge::Bridge>().launch_route()).await
}

#[tauri::command]
async fn notifications_enabled(app: tauri::AppHandle) -> CmdResult<bool> {
    blocking(move || {
        app.state::<wallpaper_bridge::Bridge>()
            .notifications_enabled()
    })
    .await
}

/// Called when the user turns the schedule on, never at app start.
#[tauri::command]
async fn request_notifications(app: tauri::AppHandle) -> CmdResult<bool> {
    blocking(move || {
        app.state::<wallpaper_bridge::Bridge>()
            .request_notifications()
    })
    .await
}

#[tauri::command]
async fn schedule_status(app: tauri::AppHandle) -> CmdResult<wallpaper_bridge::ScheduleStatus> {
    blocking(move || app.state::<wallpaper_bridge::Bridge>().schedule_status()).await
}

#[tauri::command]
async fn open_battery_settings(app: tauri::AppHandle) -> CmdResult<()> {
    blocking(move || {
        app.state::<wallpaper_bridge::Bridge>()
            .open_battery_settings()
    })
    .await
}

#[tauri::command]
fn theme_for(engine: tauri::State<'_, Engine>, palette: Vec<Swatch>, dark: bool) -> Theme {
    engine.theme_for(&palette, dark)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(wallpaper_bridge::plugin())
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            let cache = app.path().app_cache_dir()?;
            let engine = worker::engine_at(&data, &cache);
            // Make sure the daily alarm exists (first launch, or after the OS cleared it). Idempotent.
            if let Ok(s) = engine.settings() {
                let _ = app.state::<wallpaper_bridge::Bridge>().schedule(
                    &s.change_time,
                    s.wifi_only,
                    s.schedule_enabled,
                );
            }
            app.manage(engine);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            selected_place,
            search_places,
            select_place,
            subjects,
            add_query_subject,
            add_board_subject,
            update_query_subject,
            select_subject,
            delete_subject,
            source_statuses,
            check_source,
            set_source_key,
            current_wallpaper,
            change_now,
            reapply_wallpaper,
            apply_from_history,
            feedback,
            history,
            cache_info,
            clear_cache,
            forget_all,
            launch_route,
            notifications_enabled,
            request_notifications,
            schedule_status,
            open_battery_settings,
            theme_for,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the application");
}
