package io.github.spacerman.dynamicthemedwallpapers

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.os.Build
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.work.CoroutineWorker
import androidx.work.ForegroundInfo
import androidx.work.WorkerParameters
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Runs in the app process but never starts an Activity or a WebView: it calls the Rust engine over
 * JNI and installs the result with [WallpaperSetter].
 *
 * Input `op`: "change" (take the next wallpaper, install it, then top up the queue),
 * "change:<source id>" (the same, with another source for this one change; used by the buttons of
 * the fallback notification) or "prefetch".
 *
 * Input `daily` marks the run started by the daily alarm: its outcome is recorded for the Schedule
 * screen ([Scheduler.status]) and, once it is over, the next alarm is armed again.
 *
 * When a change fails because of its source (after the last retry), [FallbackNotifier] offers other sources.
 */
class DailyWallpaperWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        val op = inputData.getString(KEY_OP) ?: "change"
        val daily = inputData.getBoolean(KEY_DAILY, false)
        try {
            when (val out = NativeEngine.call(applicationContext, op)) {
                is NativeEngine.Outcome.Changed -> {
                    WallpaperSetter.set(applicationContext, out.path, out.caption, out.credit, out.y, out.align)
                    finish(daily, "changed", "")
                    // Best effort and after the wallpaper is already in place.
                    runCatching { NativeEngine.call(applicationContext, "prefetch") }
                    Result.success()
                }
                is NativeEngine.Outcome.Done -> {
                    finish(daily, "changed", "")
                    Result.success()
                }
                is NativeEngine.Outcome.Retry -> retryOrGiveUp(daily, out.message, out.fallback)
                is NativeEngine.Outcome.Failed -> {
                    Log.w(TAG, "$op failed: ${out.message}")
                    finish(daily, "failed", out.message)
                    offerFallback(out.fallback)
                    Result.failure()
                }
            }
        } catch (e: Exception) {
            Log.w(TAG, "$op crashed", e)
            retryOrGiveUp(daily, e.message ?: e.javaClass.simpleName, null)
        }
    }

    private fun retryOrGiveUp(daily: Boolean, message: String, fallback: NativeEngine.Fallback?): Result {
        Log.i(TAG, "will retry: $message (attempt $runAttemptCount)")
        // WorkManager backs off exponentially; after a few tries wait for tomorrow's run instead.
        if (runAttemptCount < MAX_ATTEMPTS) {
            if (daily) runCatching { Scheduler.record(applicationContext, "retry", message) }
            return Result.retry()
        }
        finish(daily, "failed", message)
        offerFallback(fallback)
        return Result.failure()
    }

    /** A daily run is over (done or given up): remember what happened and make sure tomorrow's alarm is set. */
    private fun finish(daily: Boolean, outcome: String, message: String) {
        if (!daily) return
        runCatching { Scheduler.record(applicationContext, outcome, message) }
        runCatching { Scheduler.armNext(applicationContext, Scheduler.AFTER_FIRING_MS) }
    }

    /** Needed by expedited work on Android 11 and older, where it runs as a foreground service. */
    override suspend fun getForegroundInfo(): ForegroundInfo {
        val context = applicationContext
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(WORKING_CHANNEL, "Wallpaper change", NotificationManager.IMPORTANCE_LOW)
            context.getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
        }
        val notification = NotificationCompat.Builder(context, WORKING_CHANNEL)
            .setSmallIcon(android.R.drawable.ic_menu_gallery)
            .setContentTitle("Changing the wallpaper")
            .setOngoing(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .build()
        return ForegroundInfo(WORKING_NOTIFICATION_ID, notification)
    }

    /** Only the final failure of a change tells the user; `fallback` is null for other operations. */
    private fun offerFallback(fallback: NativeEngine.Fallback?) {
        if (fallback != null) runCatching { FallbackNotifier.show(applicationContext, fallback) }
    }

    companion object {
        const val KEY_OP = "op"
        const val KEY_DAILY = "daily"
        private const val WORKING_CHANNEL = "wallpaper-working"
        private const val WORKING_NOTIFICATION_ID = 4103
        private const val TAG = "DailyWallpaperWorker"
        private const val MAX_ATTEMPTS = 3
    }
}
