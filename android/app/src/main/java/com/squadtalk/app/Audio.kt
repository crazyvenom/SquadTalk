package com.squadtalk.app

import android.os.SystemClock
import kotlin.math.PI
import kotlin.math.min
import kotlin.math.sin

const val RATE = 48000
const val FRAME = 960 // 20 ms

/** Short sounds: talk beeps and join/leave chimes. */
object Sounds {
    private fun tones(vararg parts: Pair<Double, Double>): ShortArray {
        val out = ArrayList<Short>()
        for ((freq, secs) in parts) {
            val n = (secs * RATE).toInt()
            for (i in 0 until n) {
                val fade = (min(i, n - i) / 240.0).coerceAtMost(1.0)
                val v = sin(2 * PI * freq * i / RATE) * 0.18 * fade
                out.add((v * 32767).toInt().toShort())
            }
        }
        return out.toShortArray()
    }

    val beepOn = tones(880.0 to 0.06)
    val beepOff = tones(587.0 to 0.06)
    val join = tones(659.0 to 0.07, 988.0 to 0.09)
    val leave = tones(988.0 to 0.07, 659.0 to 0.09)
}

/** Mixes everyone's voice (plus sounds) into one stream for the speaker. */
class Mixer {
    private class Remote {
        val frames = ArrayDeque<ShortArray>()
        var primed = false
        var lastHeard = 0L
    }

    private val remotes = HashMap<Int, Remote>()
    private val volumes = HashMap<Int, Float>()
    private val effects = ArrayDeque<ShortArray>()
    private val acc = IntArray(FRAME)

    @Volatile var deaf = false

    @Synchronized
    fun push(session: Int, pcm: ShortArray, n: Int) {
        if (deaf) return
        val r = remotes.getOrPut(session) { Remote() }
        r.frames.addLast(pcm.copyOf(n))
        r.lastHeard = SystemClock.elapsedRealtime()
        // Never let delay build up beyond ~200 ms.
        while (r.frames.size > 10) r.frames.removeFirst()
    }

    @Synchronized
    fun mix(out: ShortArray) {
        java.util.Arrays.fill(acc, 0)
        if (!deaf) {
            for ((session, r) in remotes) {
                if (!r.primed) {
                    // Wait for 40 ms of audio to smooth out network jitter.
                    if (r.frames.size >= 2) r.primed = true else continue
                }
                val f = r.frames.removeFirstOrNull()
                if (f == null) {
                    r.primed = false
                    continue
                }
                val v = volumes[session] ?: 1f
                for (i in 0 until min(f.size, FRAME)) acc[i] += (f[i] * v).toInt()
            }
        }
        effects.removeFirstOrNull()?.let { e ->
            for (i in 0 until min(e.size, FRAME)) acc[i] += e[i].toInt()
        }
        for (i in 0 until FRAME) out[i] = acc[i].coerceIn(-32768, 32767).toShort()
    }

    @Synchronized
    fun speaking(): Set<Int> {
        val now = SystemClock.elapsedRealtime()
        return remotes.filter { now - it.value.lastHeard < 300 }.keys.toSet()
    }

    @Synchronized
    fun setVolumes(map: Map<Int, Float>) {
        volumes.clear()
        volumes.putAll(map)
    }

    @Synchronized
    fun setVolume(session: Int, v: Float) {
        volumes[session] = v
    }

    @Synchronized
    fun retain(keep: Set<Int>) {
        remotes.keys.retainAll(keep)
    }

    @Synchronized
    fun effect(samples: ShortArray) {
        var i = 0
        while (i < samples.size) {
            effects.addLast(samples.copyOfRange(i, min(i + FRAME, samples.size)))
            i += FRAME
        }
        while (effects.size > 50) effects.removeFirst()
    }

    @Synchronized
    fun clear() {
        remotes.clear()
        effects.clear()
        deaf = false
    }
}
