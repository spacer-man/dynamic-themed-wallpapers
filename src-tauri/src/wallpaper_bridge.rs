//! Bridge to the Kotlin `WallpaperPlugin`. On desktop there is no wallpaper to set, so the
//! bridge reports "unsupported" and the rest of the app works unchanged.

use std::path::Path;
use tauri::plugin::{Builder, TauriPlugin};
use tauri::Manager;
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
}

impl Bridge {
    /// Install `image` as home and lock screen wallpaper, drawing the caption on it.
    /// Blocking: call from the blocking pool, not from the UI thread.
    #[cfg(target_os = "android")]
    pub fn set_wallpaper(&self, image: &Path, caption: &str, credit: &str) -> Result<(), Error> {
        let path = image
            .to_str()
            .ok_or_else(|| Error::Config("wallpaper path is not valid UTF-8".into()))?;
        self.0
            .run_mobile_plugin::<serde_json::Value>(
                "setWallpaper",
                SetArgs {
                    path,
                    caption,
                    credit,
                },
            )
            .map(|_| ())
            .map_err(|e| Error::Config(format!("setting the wallpaper failed: {e}")))
    }

    #[cfg(not(target_os = "android"))]
    pub fn set_wallpaper(&self, _image: &Path, _caption: &str, _credit: &str) -> Result<(), Error> {
        Ok(())
    }
}

pub fn plugin() -> TauriPlugin<tauri::Wry> {
    Builder::new("wallpaper")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            {
                let handle =
                    api.register_android_plugin("app.dynamicthemedwallpapers", "WallpaperPlugin")?;
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
