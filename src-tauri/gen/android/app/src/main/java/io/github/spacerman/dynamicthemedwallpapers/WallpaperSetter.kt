package io.github.spacerman.dynamicthemedwallpapers

import android.app.WallpaperManager
import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Typeface

/**
 * Installs a prepared JPEG as the wallpaper and draws the caption on it.
 *
 * Deliberately free of Tauri/WebView types: the background worker (no UI process) uses the same code.
 */
object WallpaperSetter {
    fun canSet(context: Context): Boolean {
        val wm = WallpaperManager.getInstance(context)
        return wm.isWallpaperSupported && wm.isSetWallpaperAllowed
    }

    /** Throws on any failure; the caller reports it. */
    fun set(
        context: Context,
        path: String,
        caption: String,
        credit: String,
        y: Int = DEFAULT_Y,
        align: String = "left",
    ) {
        val wm = WallpaperManager.getInstance(context)
        check(wm.isWallpaperSupported && wm.isSetWallpaperAllowed) { "wallpaper changes are not allowed on this device" }

        val opts = BitmapFactory.Options().apply { inMutable = true }
        val bmp = BitmapFactory.decodeFile(path, opts) ?: error("cannot decode $path")
        try {
            drawCaption(bmp, caption, credit, y, align)
            // No crop hint: the Rust side already cropped the photo to the screen.
            wm.setBitmap(bmp, null, true, WallpaperManager.FLAG_SYSTEM or WallpaperManager.FLAG_LOCK)
        } finally {
            bmp.recycle()
        }
    }

    /**
     * Small text block whose vertical centre sits at [centerPercent] of the screen height
     * (the user moves it clear of home screen icons) and which is aligned left, centre or right.
     * The colour follows the luminance of the pixels underneath, with a soft shadow, so it stays
     * readable on any photo.
     */
    private fun drawCaption(bmp: Bitmap, caption: String, credit: String, centerPercent: Int, align: String) {
        if (caption.isBlank() && credit.isBlank()) return
        val w = bmp.width
        val h = bmp.height
        val unit = w / 100f // 1% of the width: scales with the screen
        val margin = 5f * unit

        val lines = listOfNotNull(
            caption.takeIf { it.isNotBlank() }?.let { it to 3.6f * unit },
            credit.takeIf { it.isNotBlank() }?.let { it to 2.6f * unit },
        )
        val total = lines.sumOf { (it.second * 1.35f).toDouble() }.toFloat()
        // Keep the whole block on the screen whatever the percentage is.
        val top = (h * centerPercent.coerceIn(0, 100) / 100f - total / 2f)
            .coerceIn(margin, (h - margin - total).coerceAtLeast(margin))
        val bottom = top + total
        val dark = isDark(bmp, margin.toInt(), top.toInt(), (w - margin).toInt(), bottom.toInt())
        val ink = if (dark) Color.WHITE else Color.BLACK
        val shadow = if (dark) Color.argb(160, 0, 0, 0) else Color.argb(140, 255, 255, 255)

        val (x, textAlign) = when (align) {
            "center" -> w / 2f to Paint.Align.CENTER
            "right" -> (w - margin) to Paint.Align.RIGHT
            else -> margin to Paint.Align.LEFT
        }

        val canvas = Canvas(bmp)
        var y = top
        for ((i, line) in lines.withIndex()) {
            val (text, size) = line
            val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                color = ink
                textSize = size
                this.textAlign = textAlign
                typeface = if (i == 0) Typeface.create(Typeface.DEFAULT, Typeface.BOLD) else Typeface.DEFAULT
                alpha = if (i == 0) 255 else 200
                setShadowLayer(size * 0.35f, 0f, size * 0.08f, shadow)
            }
            y += size * 1.1f
            canvas.drawText(ellipsize(text, paint, w - 2 * margin), x, y, paint)
            y += size * 0.25f
        }
    }

    /** Default vertical position, percent of the screen height (matches the Rust default). */
    const val DEFAULT_Y = 94

    private fun ellipsize(text: String, paint: Paint, maxWidth: Float): String {
        if (paint.measureText(text) <= maxWidth) return text
        var n = text.length
        while (n > 1 && paint.measureText(text, 0, n) + paint.measureText("…") > maxWidth) n--
        return text.substring(0, n).trimEnd() + "…"
    }

    /** Sample the area on a coarse grid; true when it is mostly dark. */
    private fun isDark(bmp: Bitmap, l: Int, t: Int, r: Int, b: Int): Boolean {
        val left = l.coerceIn(0, bmp.width - 1)
        val right = r.coerceIn(left + 1, bmp.width)
        val top = t.coerceIn(0, bmp.height - 1)
        val bottom = b.coerceIn(top + 1, bmp.height)
        val stepX = maxOf(1, (right - left) / 24)
        val stepY = maxOf(1, (bottom - top) / 6)
        var sum = 0.0
        var n = 0
        var y = top
        while (y < bottom) {
            var x = left
            while (x < right) {
                val c = bmp.getPixel(x, y)
                sum += 0.2126 * Color.red(c) + 0.7152 * Color.green(c) + 0.0722 * Color.blue(c)
                n++
                x += stepX
            }
            y += stepY
        }
        return n == 0 || sum / n < 128
    }
}
