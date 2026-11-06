//! Bridge to the Kotlin `WallpaperPlugin`. On desktop there is no wallpaper to set, so the
//! bridge reports "unsupported" and the rest of the app works unchanged.

use tauri::plugin::{Builder, TauriPlugin};
use tauri::Manager;
use wallpaper_core::service::Shown;
use wallpaper_core::Error;

#[cfg(target_os = "android")]
pub struct Bridge(tauri::plugin::PluginHandle<tauri::Wry>);
#[cfg(not(target_os = "android"))]
pub struct Bridge;

#[cfg(target_os = "android")]
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SetArgs<'a> {
    path: &'a str,
    caption: &'a str,
    credit: &'a str,
    y: u8,
    align: wallpaper_core::service::CaptionAlign,
}

impl Bridge {
    /// Install the photo as home and lock screen wallpaper, drawing the caption where the
    /// user's layout says. Blocking: call from the blocking pool, not from the UI thread.
    #[cfg(target_os = "android")]
    pub fn set_wallpaper(&self, shown: &Shown) -> Result<(), Error> {
        let path = shown
            .image_path
            .to_str()
            .ok_or_else(|| Error::Config("wallpaper path is not valid UTF-8".into()))?;
        self.0
            .run_mobile_plugin::<serde_json::Value>(
                "setWallpaper",
                SetArgs {
                    path,
                    caption: &shown.caption,
                    credit: &shown.credit,
                    y: shown.layout.y,
                    align: shown.layout.align,
                },
            )
            .map(|_| ())
            .map_err(|e| Error::Config(format!("setting the wallpaper failed: {e}")))
    }

    #[cfg(not(target_os = "android"))]
    pub fn set_wallpaper(&self, _shown: &Shown) -> Result<(), Error> {
        Ok(())
    }

    /// (Re)schedule the daily change and the prefetch job. Safe to call repeatedly:
    /// WorkManager replaces the existing jobs.
    #[cfg(target_os = "android")]
    pub fn schedule(&self, time: &str, wifi_only: bool) -> Result<(), Error> {
        self.0
            .run_mobile_plugin::<serde_json::Value>(
                "scheduleDaily",
                ScheduleArgs { time, wifi_only },
            )
            .map(|_| ())
            .map_err(|e| Error::Config(format!("scheduling failed: {e}")))
    }

    #[cfg(not(target_os = "android"))]
    pub fn schedule(&self, _time: &str, _wifi_only: bool) -> Result<(), Error> {
        Ok(())
    }

    /// The screen a notification asked the app to open (`"sources"`), once; `None` otherwise.
    #[cfg(target_os = "android")]
    pub fn launch_route(&self) -> Result<Option<String>, Error> {
        self.0
            .run_mobile_plugin::<RouteReply>("launchRoute", NoArgs {})
            .map(|r| r.route)
            .map_err(|e| Error::Config(format!("reading the launch screen failed: {e}")))
    }

    #[cfg(not(target_os = "android"))]
    pub fn launch_route(&self) -> Result<Option<String>, Error> {
        Ok(None)
    }

    /// Whether notifications can be shown. Without them the fallback notification of a failed
    /// background change is silently skipped.
    #[cfg(target_os = "android")]
    pub fn notifications_enabled(&self) -> Result<bool, Error> {
        self.0
            .run_mobile_plugin::<EnabledReply>("notificationsEnabled", NoArgs {})
            .map(|r| r.enabled)
            .map_err(|e| Error::Config(format!("reading the notification state failed: {e}")))
    }

    #[cfg(not(target_os = "android"))]
    pub fn notifications_enabled(&self) -> Result<bool, Error> {
        Ok(true)
    }

    /// Ask for the notification permission (Android 13+ shows a system dialog; older versions
    /// and desktop do nothing). Returns the state *before* the user answers.
    #[cfg(target_os = "android")]
    pub fn request_notifications(&self) -> Result<bool, Error> {
        self.0
            .run_mobile_plugin::<EnabledReply>("requestNotifications", NoArgs {})
            .map(|r| r.enabled)
            .map_err(|e| Error::Config(format!("asking for notifications failed: {e}")))
    }

    #[cfg(not(target_os = "android"))]
    pub fn request_notifications(&self) -> Result<bool, Error> {
        Ok(true)
    }
}

#[cfg(target_os = "android")]
#[derive(serde::Serialize)]
struct NoArgs {}

#[cfg(target_os = "android")]
#[derive(serde::Deserialize)]
struct RouteReply {
    #[serde(default)]
    route: Option<String>,
}

#[cfg(target_os = "android")]
#[derive(serde::Deserialize)]
struct EnabledReply {
    enabled: bool,
}

#[cfg(target_os = "android")]
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ScheduleArgs<'a> {
    time: &'a str,
    wifi_only: bool,
}

pub fn plugin() -> TauriPlugin<tauri::Wry> {
    Builder::new("wallpaper")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            {
                let handle = api.register_android_plugin(
                    "io.github.spacerman.dynamicthemedwallpapers",
                    "WallpaperPlugin",
                )?;
                app.manage(Bridge(handle));
            }
            #[cfg(not(target_os = "android"))]
            {
                let _ = api;
                app.manage(Bridge);
            }
            Ok(())
        })
        .build()
}
