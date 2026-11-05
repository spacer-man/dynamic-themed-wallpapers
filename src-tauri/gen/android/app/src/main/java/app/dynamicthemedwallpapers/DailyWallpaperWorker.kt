package app.dynamicthemedwallpapers

import android.content.Context
import android.util.Log
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Runs in the app process but never starts an Activity or a WebView: it calls the Rust engine over
 * JNI and installs the result with [WallpaperSetter].
 *
 * Input `op`: "change" (take the next wallpaper, install it, then top up the queue) or "prefetch".
 */
class DailyWallpaperWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        val op = inputData.getString(KEY_OP) ?: "change"
        try {
            when (val out = NativeEngine.call(applicationContext, op)) {
                is NativeEngine.Outcome.Changed -> {
                    WallpaperSetter.set(applicationContext, out.path, out.caption, out.credit, out.y, out.align)
                    // Best effort and after the wallpaper is already in place.
                    runCatching { NativeEngine.call(applicationContext, "prefetch") }
                    Result.success()
                }
                is NativeEngine.Outcome.Done -> Result.success()
                is NativeEngine.Outcome.Retry -> retryOrGiveUp(out.message)
                is NativeEngine.Outcome.Failed -> {
                    Log.w(TAG, "$op failed: ${out.message}")
                    Result.failure()
                }
            }
        } catch (e: Exception) {
            Log.w(TAG, "$op crashed", e)
            retryOrGiveUp(e.message ?: e.javaClass.simpleName)
        }
    }

    private fun retryOrGiveUp(message: String): Result {
        Log.i(TAG, "will retry: $message (attempt $runAttemptCount)")
        // WorkManager backs off exponentially; after a few tries wait for tomorrow's run instead.
        return if (runAttemptCount < MAX_ATTEMPTS) Result.retry() else Result.failure()
    }

    companion object {
        const val KEY_OP = "op"
        private const val TAG = "DailyWallpaperWorker"
        private const val MAX_ATTEMPTS = 3
    }
}
