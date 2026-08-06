package dev.harmonia.player

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat

class MainActivity : TauriActivity() {
  private val permissionLauncher =
    registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) {
      // Result is intentionally ignored: the app works without a grant
      // (library folders can be added via the system folder picker), and
      // users can always enable access later in system settings.
    }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    requestMediaPermissions()
  }

  /**
   * Requests access to the shared music library (scoped storage) and
   * track-change notifications on first launch. Harmonia degrades gracefully
   * when the user declines.
   */
  private fun requestMediaPermissions() {
    val wanted = buildList {
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        add(Manifest.permission.READ_MEDIA_AUDIO)
        add(Manifest.permission.POST_NOTIFICATIONS)
      } else {
        add(Manifest.permission.READ_EXTERNAL_STORAGE)
      }
    }
    val needed = wanted.filter {
      ContextCompat.checkSelfPermission(this, it) != PackageManager.PERMISSION_GRANTED
    }
    if (needed.isNotEmpty()) {
      permissionLauncher.launch(needed.toTypedArray())
    }
  }
}
