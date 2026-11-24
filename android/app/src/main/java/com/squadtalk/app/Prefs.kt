package com.squadtalk.app

import android.content.Context
import org.json.JSONObject
import java.security.SecureRandom

/** Settings saved on the phone. */
class Prefs(context: Context) {
    private val p = context.getSharedPreferences("squadtalk", Context.MODE_PRIVATE)

    /** Private identity of this phone. Never shown to anyone. */
    val secret: String
        get() {
            val existing = p.getString("secret", null)
            if (existing != null && existing.length >= 16) return existing
            val bytes = ByteArray(16).also { SecureRandom().nextBytes(it) }
            val s = bytes.joinToString("") { "%02x".format(it) }
            p.edit().putString("secret", s).apply()
            return s
        }

    var name: String
        get() = p.getString("name", "") ?: ""
        set(v) = p.edit().putString("name", v).apply()
    var address: String
        get() = p.getString("address", "") ?: ""
        set(v) = p.edit().putString("address", v).apply()
    var port: Int
        get() = p.getInt("port", 9987)
        set(v) = p.edit().putInt("port", v).apply()
    var password: String
        get() = p.getString("password", "") ?: ""
        set(v) = p.edit().putString("password", v).apply()

    var voiceActivation: Boolean
        get() = p.getBoolean("voice_activation", false)
        set(v) = p.edit().putBoolean("voice_activation", v).apply()
    var vaThreshold: Int
        get() = p.getInt("va_threshold", -40)
        set(v) = p.edit().putInt("va_threshold", v).apply()
    var beeps: Boolean
        get() = p.getBoolean("beeps", true)
        set(v) = p.edit().putBoolean("beeps", v).apply()
    var joinSounds: Boolean
        get() = p.getBoolean("join_sounds", true)
        set(v) = p.edit().putBoolean("join_sounds", v).apply()
    var floating: Boolean
        get() = p.getBoolean("floating", false)
        set(v) = p.edit().putBoolean("floating", v).apply()
    var volumeKey: Boolean
        get() = p.getBoolean("volume_key", true)
        set(v) = p.edit().putBoolean("volume_key", v).apply()
    var headsetButton: Boolean
        get() = p.getBoolean("headset_button", true)
        set(v) = p.edit().putBoolean("headset_button", v).apply()
    var bubbleX: Int
        get() = p.getInt("bubble_x", 24)
        set(v) = p.edit().putInt("bubble_x", v).apply()
    var bubbleY: Int
        get() = p.getInt("bubble_y", 300)
        set(v) = p.edit().putInt("bubble_y", v).apply()

    private fun volumes(): JSONObject =
        try { JSONObject(p.getString("volumes", "{}") ?: "{}") } catch (e: Exception) { JSONObject() }

    fun volumeFor(uid: String): Int = volumes().optInt(uid, 100)

    fun setVolume(uid: String, percent: Int) {
        val v = volumes().put(uid, percent)
        p.edit().putString("volumes", v.toString()).apply()
    }
}
