package io.github.spacerman.dynamicthemedwallpapers

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationManagerCompat

/** Handles the buttons of the fallback notification by queuing one change; it does no work itself. */
class FallbackReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        NotificationManagerCompat.from(context).cancel(FallbackNotifier.NOTIFICATION_ID)
        when (intent.action) {
            FallbackNotifier.ACTION_USE_SOURCE -> {
                val id = intent.getStringExtra(FallbackNotifier.EXTRA_SOURCE) ?: return
                // Source ids are lower-case words; refuse anything else before it reaches Rust.
                if (!id.matches(Regex("[a-z]{1,20}"))) return
                Scheduler.changeOnce(context, "change:$id", 0)
            }
            FallbackNotifier.ACTION_RETRY_LATER -> Scheduler.changeOnce(context, "change", RETRY_DELAY_MINUTES)
        }
    }

    companion object {
        private const val RETRY_DELAY_MINUTES = 60L
    }
}
