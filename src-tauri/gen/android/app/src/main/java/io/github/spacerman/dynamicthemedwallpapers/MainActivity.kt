package io.github.spacerman.dynamicthemedwallpapers

import android.content.Intent
import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }

  // singleTask: a tap on the fallback notification arrives here when the app is already running.
  // Keep its extras so that WallpaperPlugin.launchRoute can read them.
  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    setIntent(intent)
  }
}
