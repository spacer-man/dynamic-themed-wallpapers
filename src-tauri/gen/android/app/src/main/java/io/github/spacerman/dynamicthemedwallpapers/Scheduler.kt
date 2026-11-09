package io.github.spacerman.dynamicthemedwallpapers

import android.app.AlarmManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import androidx.work.Constraints
import androidx.work.Data
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.OutOfQuotaPolicy
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import java.util.Calendar
import java.util.concurrent.TimeUnit

/**
 * The daily change is a self-rescheduling chain, not a periodic job (a 24 h periodic job drifts when it
 * is re-enqueued and is very inexact in Doze):
 *
 *   AlarmManager.setAndAllowWhileIdle (no exact-alarm permission) -> [DailyAlarmReceiver]
 *     -> arms the next alarm for the next local HH:MM, enqueues an expedited one-time worker
 *     -> [DailyWallpaperWorker] changes the wallpaper and arms the next alarm again.
 *
 * Alarms do not survive a reboot, so [BootReceiver] re-arms them. The prefetch job stays periodic
 * (its exact time does not matter). What the user chose and what happened last is kept in
 * SharedPreferences and shown on the Schedule screen ([status]).
 */
object Scheduler {
    private const val OLD_DAILY = "daily-wallpaper"
    private const val DAILY_NOW = "daily-change"
    private const val PREFETCH = "prefetch-wallpaper"
    private const val ONE_TIME = "one-time-change"

    const val ACTION_DAILY = "io.github.spacerman.dynamicthemedwallpapers.DAILY_ALARM"
    private const val ALARM_REQUEST = 7001

    private const val PREFS = "schedule"
    private const val K_TIME = "time"
    private const val K_WIFI = "wifi_only"
    private const val K_NEXT = "next_at"
    private const val K_LAST_AT = "last_at"
    private const val K_LAST_OUTCOME = "last_outcome"
    private const val K_LAST_MESSAGE = "last_message"
    private const val K_MIGRATED = "migrated_periodic"

    /** Re-arming right after an alarm fired must not pick the same minute again. */
    const val AFTER_FIRING_MS = 60_000L

    private fun prefs(context: Context) = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    /**
     * Idempotent: with the same time and network setting and an alarm already pending, nothing moves,
     * so calling it at every app start and settings save never shifts the next run.
     */
    fun schedule(context: Context, time: String, wifiOnly: Boolean) {
        val p = prefs(context)
        val wm = WorkManager.getInstance(context)
        if (!p.getBoolean(K_MIGRATED, false)) {
            wm.cancelUniqueWork(OLD_DAILY)
            p.edit().putBoolean(K_MIGRATED, true).apply()
        }
        val sameTime = p.getString(K_TIME, null) == time
        val sameWifi = p.getBoolean(K_WIFI, true) == wifiOnly
        p.edit().putString(K_TIME, time).putBoolean(K_WIFI, wifiOnly).apply()
        if (!(sameTime && alarmPending(context))) armNext(context)

        val network = if (wifiOnly) NetworkType.UNMETERED else NetworkType.CONNECTED
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
        // KEEP so re-scheduling does not reset the period; UPDATE only when the network rule changed.
        val policy = if (sameWifi) ExistingPeriodicWorkPolicy.KEEP else ExistingPeriodicWorkPolicy.UPDATE
        wm.enqueueUniquePeriodicWork(PREFETCH, policy, prefetch)
    }

    private fun alarmIntent(context: Context) =
        Intent(context, DailyAlarmReceiver::class.java).setAction(ACTION_DAILY)

    private fun alarmPending(context: Context): Boolean =
        PendingIntent.getBroadcast(
            context,
            ALARM_REQUEST,
            alarmIntent(context),
            PendingIntent.FLAG_NO_CREATE or PendingIntent.FLAG_IMMUTABLE,
        ) != null

    /**
     * Sets the alarm for the next local HH:MM that is at least `afterMs` away. Does nothing when no
     * schedule was ever saved. Replaces a pending alarm.
     */
    fun armNext(context: Context, afterMs: Long = 0) {
        val time = prefs(context).getString(K_TIME, null) ?: return
        val now = Calendar.getInstance().apply { timeInMillis += afterMs }
        val triggerAt = now.timeInMillis + delayUntil(time, now)
        val pending = PendingIntent.getBroadcast(
            context,
            ALARM_REQUEST,
            alarmIntent(context),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val alarms = context.getSystemService(Context.ALARM_SERVICE) as AlarmManager
        alarms.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, triggerAt, pending)
        prefs(context).edit().putLong(K_NEXT, triggerAt).apply()
    }

    /** After a reboot: arm again, and run now if the planned time went by while the phone was off. */
    fun rearmAfterBoot(context: Context) {
        val missed = prefs(context).getLong(K_NEXT, 0).let { it in 1 until System.currentTimeMillis() }
        armNext(context)
        if (missed) enqueueDaily(context)
    }

    /** The daily change itself: an expedited one-time job (plain one if the quota is used up). */
    fun enqueueDaily(context: Context) {
        val wifiOnly = prefs(context).getBoolean(K_WIFI, true)
        val network = if (wifiOnly) NetworkType.UNMETERED else NetworkType.CONNECTED
        val request = OneTimeWorkRequestBuilder<DailyWallpaperWorker>()
            .setExpedited(OutOfQuotaPolicy.RUN_AS_NON_EXPEDITED_WORK_REQUEST)
            .setConstraints(Constraints.Builder().setRequiredNetworkType(network).build())
            .setInputData(
                Data.Builder()
                    .putString(DailyWallpaperWorker.KEY_OP, "change")
                    .putBoolean(DailyWallpaperWorker.KEY_DAILY, true)
                    .build(),
            )
            .build()
        WorkManager.getInstance(context).enqueueUniqueWork(DAILY_NOW, ExistingWorkPolicy.REPLACE, request)
        record(context, "queued", "")
    }

    /** What the daily run did last: outcome is "queued", "changed", "retry" or "failed". */
    fun record(context: Context, outcome: String, message: String) {
        prefs(context).edit()
            .putLong(K_LAST_AT, System.currentTimeMillis())
            .putString(K_LAST_OUTCOME, outcome)
            .putString(K_LAST_MESSAGE, message.take(300))
            .apply()
    }

    class Status(val lastAt: Long, val outcome: String, val message: String, val nextAt: Long)

    fun status(context: Context): Status {
        val p = prefs(context)
        return Status(
            p.getLong(K_LAST_AT, 0),
            p.getString(K_LAST_OUTCOME, "") ?: "",
            p.getString(K_LAST_MESSAGE, "") ?: "",
            p.getLong(K_NEXT, 0),
        )
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

    /** Milliseconds from `now` until the next local "HH:MM" (today if still ahead, else tomorrow). */
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
