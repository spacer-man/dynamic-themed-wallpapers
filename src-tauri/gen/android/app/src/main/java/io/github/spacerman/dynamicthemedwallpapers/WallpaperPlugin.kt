package io.github.spacerman.dynamicthemedwallpapers

import android.Manifest
import android.app.Activity
import android.os.Build
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class SetWallpaperArgs {
    lateinit var path: String
    var caption: String = ""
    var credit: String = ""
    var y: Int = WallpaperSetter.DEFAULT_Y
    var align: String = "left"
}

@InvokeArg
class ScheduleArgs {
    lateinit var time: String
    var wifiOnly: Boolean = true
}

/** Thin Tauri wrapper; the work is in [WallpaperSetter] so the background worker can share it. */
@TauriPlugin
class WallpaperPlugin(private val activity: Activity) : Plugin(activity) {
    @Command
    fun canSetWallpaper(invoke: Invoke) {
        invoke.resolve(JSObject().put("allowed", WallpaperSetter.canSet(activity)))
    }

    @Command
    fun setWallpaper(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(SetWallpaperArgs::class.java)
            WallpaperSetter.set(activity, args.path, args.caption, args.credit, args.y, args.align)
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message ?: e.javaClass.simpleName)
        }
    }

    @Command
    fun scheduleDaily(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(ScheduleArgs::class.java)
            Scheduler.schedule(activity, args.time, args.wifiOnly)
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message ?: e.javaClass.simpleName)
        }
    }

    /** The screen a notification asked for ("sources"), returned once. */
    @Command
    fun launchRoute(invoke: Invoke) {
        val intent = activity.intent
        val route = intent?.getStringExtra(FallbackNotifier.EXTRA_ROUTE)
        intent?.removeExtra(FallbackNotifier.EXTRA_ROUTE)
        val out = JSObject()
        if (route != null) out.put("route", route)
        invoke.resolve(out)
    }

    @Command
    fun notificationsEnabled(invoke: Invoke) {
        invoke.resolve(JSObject().put("enabled", FallbackNotifier.canNotify(activity)))
    }

    /** Shows the Android 13+ permission dialog if needed; resolves at once with the state before the answer. */
    @Command
    fun requestNotifications(invoke: Invoke) {
        val enabled = FallbackNotifier.canNotify(activity)
        if (!enabled && Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            activity.runOnUiThread {
                activity.requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), REQUEST_NOTIFICATIONS)
            }
        }
        invoke.resolve(JSObject().put("enabled", enabled))
    }

    companion object {
        private const val REQUEST_NOTIFICATIONS = 4102
    }
}
