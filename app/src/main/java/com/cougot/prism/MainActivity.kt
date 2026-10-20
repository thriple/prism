package com.cougot.prism

import android.content.Intent
import android.content.pm.ActivityInfo
import android.net.Uri
import android.os.Bundle
import android.provider.OpenableColumns
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.setValue
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.lifecycle.lifecycleScope
import com.cougot.prism.ui.App

class MainActivity : ComponentActivity() {

    lateinit var ctrl: PlayerController
    lateinit var recents: Recents

    /** 0 = auto (sensor), 1 = landscape, 2 = portrait */
    var rotationMode by mutableIntStateOf(0)
        private set

    private val picker = registerForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) openUri(uri, persist = true)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        recents = Recents(this)
        ctrl = PlayerController(this, lifecycleScope)
        setRotation(recents.rotation)
        setContent { App(this) }
        handleIntent(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleIntent(intent)
    }

    private fun handleIntent(intent: Intent?) {
        if (intent?.action == Intent.ACTION_VIEW) {
            intent.data?.let { openUri(it, persist = false) }
        }
    }

    fun pickFile() {
        picker.launch(
            arrayOf(
                "video/*", "application/x-matroska", "application/mp4", "application/x-mpegts",
                "application/octet-stream", "audio/*",
            )
        )
    }

    fun openUri(uri: Uri, persist: Boolean) {
        if (persist) {
            try {
                contentResolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
            } catch (_: Exception) {
            }
        }
        saveProgress()
        val prev = recents.find(uri.toString())
        val name = prev?.name?.takeIf { it.isNotBlank() } ?: displayName(uri)
        val resume = prev?.let { if (it.durationMs > 0 && it.positionMs < it.durationMs - 5000) it.positionMs else 0L } ?: 0L
        ctrl.open(uri, name, resume)
        recents.upsert(RecentItem(uri.toString(), name, prev?.durationMs ?: 0, resume, prev?.info ?: "", false))
    }

    fun openRecent(item: RecentItem) = openUri(Uri.parse(item.uri), persist = false)

    fun closePlayer() {
        saveProgress()
        ctrl.close()
        setImmersive(false)
    }

    fun saveProgress() {
        val u = ctrl.uri ?: return
        val prev = recents.find(u.toString())
        recents.upsert(
            RecentItem(
                u.toString(), ctrl.name, ctrl.duration, ctrl.position,
                ctrl.info.ifBlank { prev?.info ?: "" },
                ctrl.damaged.isNotEmpty() || prev?.damaged == true,
            )
        )
    }

    private fun displayName(uri: Uri): String {
        try {
            contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
                if (c.moveToFirst()) {
                    val s = c.getString(0)
                    if (!s.isNullOrBlank()) return s
                }
            }
        } catch (_: Exception) {
        }
        return uri.lastPathSegment?.substringAfterLast('/') ?: "Video"
    }

    fun setRotation(mode: Int) {
        rotationMode = mode
        recents.rotation = mode
        requestedOrientation = when (mode) {
            1 -> ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
            2 -> ActivityInfo.SCREEN_ORIENTATION_SENSOR_PORTRAIT
            else -> ActivityInfo.SCREEN_ORIENTATION_FULL_SENSOR
        }
    }

    fun cycleRotation() = setRotation((rotationMode + 1) % 3)

    fun setImmersive(on: Boolean) {
        val c = WindowCompat.getInsetsController(window, window.decorView)
        if (on) {
            c.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            c.hide(WindowInsetsCompat.Type.systemBars())
        } else {
            c.show(WindowInsetsCompat.Type.systemBars())
        }
    }

    override fun onStop() {
        super.onStop()
        saveProgress()
        ctrl.pause()
    }

    override fun onDestroy() {
        super.onDestroy()
        ctrl.release()
    }
}
