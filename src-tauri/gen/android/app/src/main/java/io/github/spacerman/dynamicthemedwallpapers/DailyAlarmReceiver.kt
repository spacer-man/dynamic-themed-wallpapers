package io.github.spacerman.dynamicthemedwallpapers

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/**
 * The daily alarm. It arms tomorrow's alarm first (so the chain survives any failure below), then
 * queues the change. It does no work itself, so it returns well inside the receiver time limit.
 */
class DailyAlarmReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != Scheduler.ACTION_DAILY) return
        runCatching { Scheduler.armNext(context, Scheduler.AFTER_FIRING_MS) }
        runCatching { Scheduler.enqueueDaily(context) }
    }
}
