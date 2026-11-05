package io.github.spacerman.dynamicthemedwallpapers

import android.content.Context
import android.content.res.Configuration
import org.json.JSONObject

/** The engine inside libdtw_lib.so (the same library Tauri loads), called without any UI. */
object NativeEngine {
    init {
        System.loadLibrary("dtw_lib")
    }

    /** Implemented in `src-tauri/src/worker.rs`. Returns the JSON of `worker::Outcome`. */
    @JvmStatic
    private external fun run(
        op: String,
        dataDir: String,
        cacheDir: String,
        width: Int,
        height: Int,
        systemDark: Boolean,
    ): String

    sealed class Outcome {
        data class Changed(
            val path: String,
            val caption: String,
            val credit: String,
            val y: Int,
            val align: String,
        ) : Outcome()
        data class Done(val added: Int) : Outcome()
        data class Retry(val message: String) : Outcome()
        data class Failed(val message: String) : Outcome()
    }

    /** `op` is "change" or "prefetch". Blocking: call from a background thread. */
    fun call(context: Context, op: String): Outcome {
        val metrics = context.resources.displayMetrics
        val dark = (context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) ==
            Configuration.UI_MODE_NIGHT_YES
        val json = JSONObject(
            run(
                op,
                context.dataDir.absolutePath,
                context.cacheDir.absolutePath,
                metrics.widthPixels,
                metrics.heightPixels,
                dark,
            ),
        )
        return when (json.optString("status")) {
            "changed" -> Outcome.Changed(
                json.getString("path"),
                json.optString("caption"),
                json.optString("credit"),
                json.optInt("y", WallpaperSetter.DEFAULT_Y),
                json.optString("align", "left"),
            )
            "done" -> Outcome.Done(json.optInt("added"))
            "retry" -> Outcome.Retry(json.optString("message"))
            else -> Outcome.Failed(json.optString("message", "unknown error"))
        }
    }
}
