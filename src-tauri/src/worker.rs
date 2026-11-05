//! Entry points for the background worker (`DailyWallpaperWorker`, Kotlin).
//!
//! WorkManager runs the worker in this app's process but without the UI: no Activity, no WebView.
//! The worker calls straight into the engine over JNI, gets back where the prepared JPEG is and
//! what to write on it, and installs it with `WallpaperSetter`. Keeping this path free of Tauri
//! types is the point: it is what keeps the daily change cheap on the battery.

use serde::Serialize;
use std::path::Path;
use wallpaper_core::pipeline::Screen;
use wallpaper_core::service::Engine;

/// The engine used by both the UI process and the worker. On Android `data` is the app's data
/// directory (`Context.getDataDir()`) and `cache` is `Context.getCacheDir()`: the same
/// directories Tauri hands to `app_data_dir()` / `app_cache_dir()`, so both see one database.
pub fn engine_at(data: &Path, cache: &Path) -> Engine {
    Engine::new(data.join("app.db"), cache.join("wallpapers"))
}

/// What the worker needs to know after a call. Serialised to JSON for the JNI boundary.
#[derive(Debug, Serialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Outcome {
    /// A wallpaper was chosen: install it.
    Changed {
        path: String,
        caption: String,
        credit: String,
        /// Vertical centre of the text block, percent of the screen height.
        y: u8,
        /// "left", "center" or "right".
        align: wallpaper_core::service::CaptionAlign,
    },
    /// Nothing to install (prefetch ran, or the queue was topped up).
    Done { added: u32 },
    /// Worth retrying later (network, rate limit).
    Retry { message: String },
    /// Retrying will not help until something changes (settings, empty result).
    Failed { message: String },
}

/// `op` is `"change"` (take the next wallpaper) or `"prefetch"` (top up the queue if it is low).
pub fn run(
    op: &str,
    data: &Path,
    cache: &Path,
    width: u32,
    height: u32,
    system_dark: bool,
) -> Outcome {
    let engine = engine_at(data, cache);
    let screen = Screen { width, height };
    let now = Engine::now();
    match op {
        "change" => match engine.change_now(screen, system_dark, now) {
            Ok(shown) => Outcome::Changed {
                path: shown.image_path.to_string_lossy().into_owned(),
                caption: shown.caption,
                credit: shown.credit,
                y: shown.layout.y,
                align: shown.layout.align,
            },
            Err(e) => classify(e),
        },
        "prefetch" => match engine.prefetch_if_low(screen, system_dark, now) {
            Ok(report) => Outcome::Done {
                added: report.map_or(0, |r| r.added),
            },
            Err(e) => classify(e),
        },
        other => Outcome::Failed {
            message: format!("unknown operation {other:?}"),
        },
    }
}

fn classify(e: wallpaper_core::Error) -> Outcome {
    use wallpaper_core::Error::*;
    let message = e.to_string();
    match e {
        Network(_) | Status { .. } => Outcome::Retry { message },
        _ => Outcome::Failed { message },
    }
}

#[cfg(target_os = "android")]
mod jni_entry {
    use super::*;
    use jni::objects::{JObject, JString};
    use jni::sys::{jboolean, jint, jstring};
    use jni::JNIEnv;

    fn read(env: &mut JNIEnv, s: &JString) -> Option<String> {
        env.get_string(s).ok().map(|s| s.into())
    }

    /// Kotlin: `object NativeEngine { external fun run(op, dataDir, cacheDir, width, height, systemDark): String }`
    #[no_mangle]
    pub extern "system" fn Java_app_dynamicthemedwallpapers_NativeEngine_run(
        mut env: JNIEnv,
        _this: JObject,
        op: JString,
        data_dir: JString,
        cache_dir: JString,
        width: jint,
        height: jint,
        system_dark: jboolean,
    ) -> jstring {
        let outcome = match (
            read(&mut env, &op),
            read(&mut env, &data_dir),
            read(&mut env, &cache_dir),
        ) {
            (Some(op), Some(data), Some(cache)) => {
                // A panic must not unwind across the JNI boundary.
                std::panic::catch_unwind(|| {
                    run(
                        &op,
                        Path::new(&data),
                        Path::new(&cache),
                        width.max(1) as u32,
                        height.max(1) as u32,
                        system_dark != 0,
                    )
                })
                .unwrap_or_else(|_| Outcome::Failed {
                    message: "engine panicked".into(),
                })
            }
            _ => Outcome::Failed {
                message: "invalid arguments".into(),
            },
        };
        let json = serde_json::to_string(&outcome).unwrap_or_else(|_| {
            r#"{"status":"failed","message":"cannot serialise the result"}"#.into()
        });
        match env.new_string(json) {
            Ok(s) => s.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_operation_fails_without_touching_the_disk() {
        let dir = std::env::temp_dir().join("dtw-worker-test-unused");
        let out = run("explode", &dir, &dir, 1080, 2400, false);
        assert!(matches!(out, Outcome::Failed { .. }));
        assert!(!dir.exists());
    }

    #[test]
    fn network_errors_are_retried_and_others_are_not() {
        use wallpaper_core::Error;
        assert!(matches!(
            classify(Error::Network("timeout".into())),
            Outcome::Retry { .. }
        ));
        assert!(matches!(
            classify(Error::Status {
                status: 503,
                url: "x".into()
            }),
            Outcome::Retry { .. }
        ));
        assert!(matches!(
            classify(Error::NotFound("nothing".into())),
            Outcome::Failed { .. }
        ));
    }

    #[test]
    fn outcome_json_is_tagged_for_kotlin() {
        let json = serde_json::to_string(&Outcome::Changed {
            path: "/a.jpg".into(),
            caption: "Kyoto".into(),
            credit: "Someone, CC BY".into(),
            y: 94,
            align: wallpaper_core::service::CaptionAlign::Left,
        })
        .unwrap();
        assert!(json.contains(r#""status":"changed""#));
        assert!(json.contains(r#""path":"/a.jpg""#));
        assert!(json.contains(r#""y":94"#) && json.contains(r#""align":"left""#));
    }
}
