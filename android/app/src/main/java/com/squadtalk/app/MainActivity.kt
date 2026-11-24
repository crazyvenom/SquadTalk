package com.squadtalk.app

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts

class MainActivity : ComponentActivity() {
    private var pendingAction: (() -> Unit)? = null

    private val permissionRequest =
        registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { _ ->
            val action = pendingAction
            pendingAction = null
            if (hasMic()) {
                action?.invoke()
            } else {
                Session.error = "SquadTalk needs permission to use the microphone so your friends can hear you.\n\n" +
                    "You can allow it in Android Settings → Apps → SquadTalk → Permissions."
            }
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Session.init(this)
        setContent { SquadTalkApp(this) }
    }

    override fun onResume() {
        super.onResume()
        // Coming back from the "Display over other apps" screen.
        VoiceService.instance?.refreshBubble()
    }

    private fun hasMic() =
        checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED

    /** Asks for microphone (and notification) permission first if needed. */
    fun withPermissions(action: () -> Unit) {
        val need = ArrayList<String>()
        if (!hasMic()) need += Manifest.permission.RECORD_AUDIO
        if (Build.VERSION.SDK_INT >= 33 &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) need += Manifest.permission.POST_NOTIFICATIONS
        if (need.isEmpty()) {
            action()
        } else {
            pendingAction = action
            permissionRequest.launch(need.toTypedArray())
        }
    }

    /** Hold Volume Down to talk while the app is open. */
    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        if (event.keyCode == KeyEvent.KEYCODE_VOLUME_DOWN &&
            Session.status == Status.CONNECTED && Session.prefs.volumeKey && !Session.vaMode
        ) {
            when (event.action) {
                KeyEvent.ACTION_DOWN -> if (event.repeatCount == 0) Session.setPtt(Session.PTT_VOLKEY, true)
                KeyEvent.ACTION_UP -> Session.setPtt(Session.PTT_VOLKEY, false)
            }
            return true
        }
        return super.dispatchKeyEvent(event)
    }
}
