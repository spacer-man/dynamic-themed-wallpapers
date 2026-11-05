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
use wallpaper_core::service::{CacheInfo, Engine, Settings};
use wallpaper_core::store::HistoryEntry;
use wallpaper_core::theme::Theme;
use wallpaper_core::{Error, Place, PlaceHit};

/// Error shape the UI can switch on (and translate) without parsing messages.
#[derive(Debug, Serialize)]
pub struct CmdError {
    kind: &'static str,
    message: String,
}

impl From<Error> for CmdError {
    fn from(e: Error) -> Self {
        let kind = match &e {
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
        app.state::<wallpaper_bridge::Bridge>()
            .schedule(&saved.change_time, saved.wifi_only)?;
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
) -> CmdResult<Option<WallpaperDto>> {
    let e = engine.inner().clone();
    blocking(move || {
        let shown = e.change_now(screen.to_screen(), system_dark, Engine::now())?;
        app.state::<wallpaper_bridge::Bridge>().set_wallpaper(
            &shown.image_path,
            &shown.caption,
            &shown.credit,
        )?;
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
            // Make sure the daily job exists (first launch, or after the OS cleared it).
            if let Ok(s) = engine.settings() {
                let _ = app
                    .state::<wallpaper_bridge::Bridge>()
                    .schedule(&s.change_time, s.wifi_only);
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
            current_wallpaper,
            change_now,
            feedback,
            history,
            cache_info,
            clear_cache,
            forget_all,
            theme_for,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the application");
}
