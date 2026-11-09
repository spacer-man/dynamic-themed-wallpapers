package io.github.spacerman.dynamicthemedwallpapers

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/**
 * Alarms are lost on reboot, and a changed clock or time zone moves "08:00". The manifest exports this
 * receiver only so the system can deliver these broadcasts; any other action is ignored and nothing in
 * the intent is read.
 */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        when (intent.action) {
            Intent.ACTION_BOOT_COMPLETED -> runCatching { Scheduler.rearmAfterBoot(context) }
            Intent.ACTION_MY_PACKAGE_REPLACED,
            Intent.ACTION_TIMEZONE_CHANGED,
            Intent.ACTION_TIME_CHANGED,
            -> runCatching { Scheduler.armNext(context) }
        }
    }
}
