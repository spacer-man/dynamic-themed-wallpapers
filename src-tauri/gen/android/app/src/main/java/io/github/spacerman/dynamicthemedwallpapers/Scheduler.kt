package io.github.spacerman.dynamicthemedwallpapers

import android.content.Context
import androidx.work.Constraints
import androidx.work.Data
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import java.util.Calendar
import java.util.concurrent.TimeUnit

/**
 * Two periodic jobs. WorkManager persists them across reboots and app updates by itself, which is
 * why there is no BootReceiver.
 *  - daily: change the wallpaper around the chosen local time;
 *  - prefetch: refill the queue while on an unmetered network, charging and with a healthy battery.
 */
object Scheduler {
    private const val DAILY = "daily-wallpaper"
    private const val PREFETCH = "prefetch-wallpaper"
    private const val ONE_TIME = "one-time-change"

    fun schedule(context: Context, time: String, wifiOnly: Boolean) {
        val wm = WorkManager.getInstance(context)
        val network = if (wifiOnly) NetworkType.UNMETERED else NetworkType.CONNECTED

        val daily = PeriodicWorkRequestBuilder<DailyWallpaperWorker>(24, TimeUnit.HOURS)
            .setInitialDelay(delayUntil(time), TimeUnit.MILLISECONDS)
            .setConstraints(
                Constraints.Builder().setRequiredNetworkType(network).setRequiresBatteryNotLow(true).build(),
            )
            .setInputData(Data.Builder().putString(DailyWallpaperWorker.KEY_OP, "change").build())
            .build()
        wm.enqueueUniquePeriodicWork(DAILY, ExistingPeriodicWorkPolicy.UPDATE, daily)

        val prefetch = PeriodicWorkRequestBuilder<DailyWallpaperWorker>(12, TimeUnit.HOURS)
            .setConstraints(
                Constraints.Builder()
                    .setRequiredNetworkType(network)
                    .setRequiresCharging(true)
                    .setRequiresBatteryNotLow(true)
                    .build(),
            )
            .setInputData(Data.Builder().putString(DailyWallpaperWorker.KEY_OP, "prefetch").build())
            .build()
        wm.enqueueUniquePeriodicWork(PREFETCH, ExistingPeriodicWorkPolicy.UPDATE, prefetch)
    }

    /**
     * One change now (or after `delayMinutes`): `op` is "change", or "change:<source id>" to use
     * another source for this change only. A newer request replaces a pending one.
     */
    fun changeOnce(context: Context, op: String, delayMinutes: Long) {
        val request = OneTimeWorkRequestBuilder<DailyWallpaperWorker>()
            .setInitialDelay(delayMinutes, TimeUnit.MINUTES)
            .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
            .setInputData(Data.Builder().putString(DailyWallpaperWorker.KEY_OP, op).build())
            .build()
        WorkManager.getInstance(context).enqueueUniqueWork(ONE_TIME, ExistingWorkPolicy.REPLACE, request)
    }

    fun cancel(context: Context) {
        val wm = WorkManager.getInstance(context)
        wm.cancelUniqueWork(DAILY)
        wm.cancelUniqueWork(PREFETCH)
    }

    /** Milliseconds from now until the next local "HH:MM" (today if still ahead, else tomorrow). */
    internal fun delayUntil(time: String, now: Calendar = Calendar.getInstance()): Long {
        val (h, m) = time.split(":").map { it.toInt() }
        val next = (now.clone() as Calendar).apply {
            set(Calendar.HOUR_OF_DAY, h)
            set(Calendar.MINUTE, m)
            set(Calendar.SECOND, 0)
            set(Calendar.MILLISECOND, 0)
            if (!after(now)) add(Calendar.DAY_OF_YEAR, 1)
        }
        return next.timeInMillis - now.timeInMillis
    }
}
