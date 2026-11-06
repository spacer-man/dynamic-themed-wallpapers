# Release builds run R8 (optimization { enable = true } in build.gradle.kts). Everything reached by name
# from Rust (JNI / Tauri plugin loader) or by reflection (Tauri @Command / @InvokeArg) must be kept, or the
# app dies at launch: `register_android_plugin` fails in Rust setup when WallpaperPlugin is renamed/removed.
# This file is committed (gen/android is tracked); keep it in sync if you add Kotlin classes used from Rust.

-keepattributes SourceFile,LineNumberTable,*Annotation*,Signature,InnerClasses,EnclosingMethod
-renamesourcefileattribute SourceFile

# Tauri plugin looked up by class name from Rust (wallpaper_bridge.rs register_android_plugin),
# with commands invoked reflectively by name.
-keep class io.github.spacerman.dynamicthemedwallpapers.WallpaperPlugin {
    <init>(...);
    @app.tauri.annotation.Command <methods>;
}

# Command argument classes are filled reflectively by Tauri (invoke.parseArgs).
-keep @app.tauri.annotation.InvokeArg class * { *; }
-keep class io.github.spacerman.dynamicthemedwallpapers.SetWallpaperArgs { *; }
-keep class io.github.spacerman.dynamicthemedwallpapers.ScheduleArgs { *; }

# JNI: Rust exports Java_io_github_spacerman_dynamicthemedwallpapers_NativeEngine_run, so class, object
# INSTANCE, method name and signature must not change.
-keep class io.github.spacerman.dynamicthemedwallpapers.NativeEngine {
    public static final ** INSTANCE;
    *;
}
-keepclasseswithmembernames,includedescriptorclasses class * {
    native <methods>;
}

# Instantiated by WorkManager from the class name stored in its database.
-keep class io.github.spacerman.dynamicthemedwallpapers.DailyWallpaperWorker { <init>(...); }

# Activity named in the manifest, and the Tauri/wry runtime (also covered by the generated proguard-tauri.pro).
-keep class io.github.spacerman.dynamicthemedwallpapers.MainActivity { *; }
-keep class app.tauri.** { *; }
-keep class * extends app.tauri.plugin.Plugin { *; }
-keep class kotlin.Metadata { *; }

# WebView <-> Rust bridges: Wry's generated classes are called from native code.
-keepclassmembers class * {
    @android.webkit.JavascriptInterface <methods>;
}
-keep class io.github.spacerman.dynamicthemedwallpapers.RustWebView { *; }
-keep class io.github.spacerman.dynamicthemedwallpapers.RustWebViewClient { *; }
-keep class io.github.spacerman.dynamicthemedwallpapers.RustWebChromeClient { *; }
-keep class io.github.spacerman.dynamicthemedwallpapers.Ipc { *; }
-keep class io.github.spacerman.dynamicthemedwallpapers.TauriActivity { *; }
-keep class io.github.spacerman.dynamicthemedwallpapers.WryActivity { *; }
-keep class io.github.spacerman.dynamicthemedwallpapers.Logger { *; }
-keep class io.github.spacerman.dynamicthemedwallpapers.PermissionHelper { *; }
-dontwarn io.github.spacerman.dynamicthemedwallpapers.**
