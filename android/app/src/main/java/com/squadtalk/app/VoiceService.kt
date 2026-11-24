package com.squadtalk.app

import android.annotation.SuppressLint
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.graphics.PixelFormat
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.Icon
import android.media.session.MediaSession
import android.media.session.PlaybackState
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import android.provider.Settings
import android.view.Gravity
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.View
import android.view.WindowManager
import android.widget.TextView
import kotlin.math.hypot

/**
 * Keeps voice chat running while you use other apps or the screen is off.
 * Also owns the floating talk button and the earphone-button handling.
 */
class VoiceService : Service() {
    companion object {
        private const val CHANNEL = "voice"
        private const val NOTIFICATION_ID = 1
        const val ACTION_DISCONNECT = "com.squadtalk.app.DISCONNECT"

        @Volatile var instance: VoiceService? = null
            private set

        fun start(ctx: Context) {
            try {
                ctx.startForegroundService(Intent(ctx, VoiceService::class.java))
            } catch (_: Exception) {
            }
        }

        fun stop(ctx: Context) {
            ctx.stopService(Intent(ctx, VoiceService::class.java))
        }
    }

    private var wakeLock: PowerManager.WakeLock? = null
    private var mediaSession: MediaSession? = null
    private var bubble: View? = null
    private var foregroundStarted = false

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        instance = this
        Session.init(this)
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(
            NotificationChannel(CHANNEL, "Voice chat", NotificationManager.IMPORTANCE_LOW).apply {
                description = "Shown while SquadTalk is connected"
            }
        )
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_DISCONNECT) {
            Session.disconnect()
            return START_NOT_STICKY
        }
        if (!foregroundStarted) {
            try {
                if (Build.VERSION.SDK_INT >= 30) {
                    startForeground(NOTIFICATION_ID, buildNotification(), ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE)
                } else {
                    startForeground(NOTIFICATION_ID, buildNotification())
                }
                foregroundStarted = true
            } catch (e: Exception) {
                // Voice still works while the app is open.
            }
        }
        if (wakeLock == null) {
            val pm = getSystemService(PowerManager::class.java)
            wakeLock = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "SquadTalk:voice").apply {
                setReferenceCounted(false)
                acquire(12 * 60 * 60 * 1000L)
            }
        }
        refreshMediaSession()
        refreshBubble()
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        removeBubble()
        mediaSession?.release()
        mediaSession = null
        try { wakeLock?.release() } catch (_: Exception) {}
        wakeLock = null
        instance = null
        super.onDestroy()
    }

    private fun buildNotification(): Notification {
        val open = PendingIntent.getActivity(
            this, 0,
            Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE
        )
        val stop = PendingIntent.getService(
            this, 1,
            Intent(this, VoiceService::class.java).setAction(ACTION_DISCONNECT),
            PendingIntent.FLAG_IMMUTABLE
        )
        return Notification.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_mic)
            .setContentTitle("SquadTalk")
            .setContentText("Voice chat is running")
            .setOngoing(true)
            .setContentIntent(open)
            .addAction(Notification.Action.Builder(null as Icon?, "Disconnect", stop).build())
            .build()
    }

    // ---------- Earphone button: press to start talking, press again to stop ----------

    fun refreshMediaSession() {
        val want = Session.prefs.headsetButton
        if (want && mediaSession == null) {
            val ms = MediaSession(this, "SquadTalk")
            ms.setCallback(object : MediaSession.Callback() {
                override fun onMediaButtonEvent(mediaButtonIntent: Intent): Boolean {
                    val ev: KeyEvent? = if (Build.VERSION.SDK_INT >= 33) {
                        mediaButtonIntent.getParcelableExtra(Intent.EXTRA_KEY_EVENT, KeyEvent::class.java)
                    } else {
                        @Suppress("DEPRECATION")
                        mediaButtonIntent.getParcelableExtra(Intent.EXTRA_KEY_EVENT)
                    }
                    val keys = setOf(
                        KeyEvent.KEYCODE_HEADSETHOOK,
                        KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE,
                        KeyEvent.KEYCODE_MEDIA_PLAY,
                        KeyEvent.KEYCODE_MEDIA_PAUSE,
                    )
                    if (ev != null && ev.keyCode in keys) {
                        if (ev.action == KeyEvent.ACTION_DOWN && ev.repeatCount == 0) Session.toggleHeadsetTalk()
                        return true
                    }
                    return super.onMediaButtonEvent(mediaButtonIntent)
                }
            })
            ms.setPlaybackState(
                PlaybackState.Builder()
                    .setActions(PlaybackState.ACTION_PLAY_PAUSE or PlaybackState.ACTION_PLAY or PlaybackState.ACTION_PAUSE)
                    .setState(PlaybackState.STATE_PLAYING, 0, 1f)
                    .build()
            )
            ms.isActive = true
            mediaSession = ms
        } else if (!want && mediaSession != null) {
            mediaSession?.release()
            mediaSession = null
        }
    }

    // ---------- Floating talk button over other apps ----------

    fun refreshBubble() {
        val want = Session.prefs.floating && Settings.canDrawOverlays(this)
        if (want && bubble == null) addBubble() else if (!want && bubble != null) removeBubble()
    }

    @SuppressLint("ClickableViewAccessibility", "SetTextI18n")
    private fun addBubble() {
        val wm = getSystemService(WindowManager::class.java)
        val density = resources.displayMetrics.density
        val size = (68 * density).toInt()
        val idle = 0xCC2B3A67.toInt()
        val talking = 0xEE3DDC84.toInt()
        val bg = GradientDrawable().apply {
            shape = GradientDrawable.OVAL
            setColor(idle)
            setStroke((2 * density).toInt(), 0xFF7C9CFF.toInt())
        }
        val view = TextView(this).apply {
            text = "🎙"
            textSize = 28f
            gravity = Gravity.CENTER
            background = bg
        }
        val lp = WindowManager.LayoutParams(
            size, size,
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL,
            PixelFormat.TRANSLUCENT
        ).apply {
            gravity = Gravity.TOP or Gravity.START
            x = Session.prefs.bubbleX
            y = Session.prefs.bubbleY
        }

        var downX = 0f
        var downY = 0f
        var startX = 0
        var startY = 0
        var dragging = false
        view.setOnTouchListener { _, e ->
            when (e.actionMasked) {
                MotionEvent.ACTION_DOWN -> {
                    downX = e.rawX; downY = e.rawY
                    startX = lp.x; startY = lp.y
                    dragging = false
                    Session.setPtt(Session.PTT_BUBBLE, true)
                }
                MotionEvent.ACTION_MOVE -> {
                    val dx = e.rawX - downX
                    val dy = e.rawY - downY
                    if (!dragging && hypot(dx, dy) > 24 * density) {
                        dragging = true
                        Session.setPtt(Session.PTT_BUBBLE, false)
                    }
                    if (dragging) {
                        lp.x = startX + dx.toInt()
                        lp.y = startY + dy.toInt()
                        try { wm.updateViewLayout(view, lp) } catch (_: Exception) {}
                    }
                }
                MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                    Session.setPtt(Session.PTT_BUBBLE, false)
                    if (dragging) {
                        Session.prefs.bubbleX = lp.x
                        Session.prefs.bubbleY = lp.y
                    }
                }
            }
            true
        }
        try {
            wm.addView(view, lp)
            bubble = view
            Session.onSelfSpeaking = { on -> bg.setColor(if (on) talking else idle) }
        } catch (_: Exception) {
        }
    }

    private fun removeBubble() {
        val v = bubble ?: return
        bubble = null
        Session.onSelfSpeaking = null
        try { getSystemService(WindowManager::class.java).removeView(v) } catch (_: Exception) {}
    }
}
