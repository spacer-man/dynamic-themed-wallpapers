package io.github.spacerman.dynamicthemedwallpapers

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import java.util.Locale

/**
 * The notification shown when the scheduled change could not get a photo from the chosen source:
 * one action per other working source (a one-time change with it, see [FallbackReceiver]), plus
 * "Retry later". Opening the body opens the app on the Sources screen.
 */
object FallbackNotifier {
    const val NOTIFICATION_ID = 4101
    const val ACTION_USE_SOURCE = "io.github.spacerman.dynamicthemedwallpapers.USE_SOURCE"
    const val ACTION_RETRY_LATER = "io.github.spacerman.dynamicthemedwallpapers.RETRY_LATER"
    const val EXTRA_SOURCE = "source"
    /** Extra on the intent that opens [MainActivity]; read once by `WallpaperPlugin.launchRoute`. */
    const val EXTRA_ROUTE = "route"
    private const val CHANNEL_ID = "wallpaper-problems"
    private const val REQUEST_OPEN = 1
    private const val REQUEST_RETRY = 2
    private const val REQUEST_SOURCE_BASE = 10

    fun canNotify(context: Context): Boolean = NotificationManagerCompat.from(context).areNotificationsEnabled()

    private fun russian(lang: String): Boolean =
        lang == "ru" || (lang != "en" && Locale.getDefault().language == "ru")

    private fun ensureChannel(context: Context, ru: Boolean) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val channel = NotificationChannel(
            CHANNEL_ID,
            if (ru) "Проблемы со сменой обоев" else "Wallpaper problems",
            NotificationManager.IMPORTANCE_LOW,
        )
        context.getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
    }

    fun show(context: Context, fallback: NativeEngine.Fallback) {
        if (!canNotify(context)) return
        val ru = russian(fallback.lang)
        ensureChannel(context, ru)

        val open = PendingIntent.getActivity(
            context,
            REQUEST_OPEN,
            Intent(context, MainActivity::class.java)
                .putExtra(EXTRA_ROUTE, "sources")
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val title = if (ru) "Не удалось получить фото из ${fallback.sourceName}"
        else "Couldn't get a photo from ${fallback.sourceName}"
        val text = if (fallback.alternatives.isEmpty()) {
            if (ru) "Другие источники сейчас недоступны. Попробуем позже." else "No other source is available right now."
        } else {
            if (ru) "Выберите другой источник, чтобы сменить обои один раз."
            else "Pick another source to change the wallpaper once."
        }
        val builder = NotificationCompat.Builder(context, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.ic_menu_gallery)
            .setContentTitle(title)
            .setContentText(text)
            .setContentIntent(open)
            .setAutoCancel(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
        fallback.alternatives.forEachIndexed { i, alt ->
            val use = PendingIntent.getBroadcast(
                context,
                REQUEST_SOURCE_BASE + i,
                Intent(context, FallbackReceiver::class.java)
                    .setAction(ACTION_USE_SOURCE)
                    .putExtra(EXTRA_SOURCE, alt.id),
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
            builder.addAction(0, if (ru) "Взять из ${alt.name}" else "Use ${alt.name}", use)
        }
        val later = PendingIntent.getBroadcast(
            context,
            REQUEST_RETRY,
            Intent(context, FallbackReceiver::class.java).setAction(ACTION_RETRY_LATER),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        builder.addAction(0, if (ru) "Повторить позже" else "Retry later", later)
        try {
            NotificationManagerCompat.from(context).notify(NOTIFICATION_ID, builder.build())
        } catch (e: SecurityException) {
            // The permission was revoked between the check and now: nothing to show.
        }
    }
}
