package com.squadtalk.app

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.AudioTrack
import android.media.MediaRecorder
import android.media.audiofx.AcousticEchoCanceler
import android.os.Handler
import android.os.Looper
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import org.json.JSONObject
import java.io.BufferedReader
import java.io.BufferedWriter
import java.io.IOException
import java.io.InputStreamReader
import java.io.OutputStreamWriter
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetSocketAddress
import java.net.Socket
import java.net.SocketTimeoutException
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger
import kotlin.concurrent.thread
import kotlin.math.log10
import kotlin.math.max

data class RoomInfo(val id: Int, val name: String)

data class UserInfo(
    val session: Int,
    val uid: String,
    val name: String,
    val room: Int,
    val mute: Boolean,
    val deaf: Boolean,
    val serverMute: Boolean,
    val owner: Boolean,
)

enum class Status { DISCONNECTED, CONNECTING, CONNECTED }

private class DeniedException(msg: String) : Exception(msg)

/**
 * The connection to a SquadTalk server and the voice engine.
 * Speaks exactly the same protocol as the PC app (see src/protocol.rs).
 */
object Session {
    const val VERSION = 1
    const val PTT_BUTTON = 1
    const val PTT_BUBBLE = 2
    const val PTT_VOLKEY = 4

    private val main = Handler(Looper.getMainLooper())
    lateinit var app: Context
        private set
    lateinit var prefs: Prefs
        private set

    fun init(context: Context) {
        if (!::app.isInitialized) {
            app = context.applicationContext
            prefs = Prefs(app)
            vaMode = prefs.voiceActivation
        }
    }

    // ---------- State shown on screen (main thread only) ----------
    var status by mutableStateOf(Status.DISCONNECTED)
        private set
    var error by mutableStateOf<String?>(null)
    var rooms by mutableStateOf(listOf<RoomInfo>())
        private set
    var users by mutableStateOf(listOf<UserInfo>())
        private set
    var me by mutableIntStateOf(0)
        private set
    var isOwner by mutableStateOf(false)
        private set
    val chat = mutableStateListOf<String>()
    var speaking by mutableStateOf(setOf<Int>())
        private set
    var selfSpeaking by mutableStateOf(false)
        private set
    var muted by mutableStateOf(false)
        private set
    var deaf by mutableStateOf(false)
        private set
    var pingMs by mutableStateOf<Int?>(null)
        private set
    var micLevelDb by mutableIntStateOf(-100)
        private set
    var headsetLatched by mutableStateOf(false)
        private set
    var vaMode by mutableStateOf(false)

    /** Called when you start/stop talking (used by the floating button). */
    var onSelfSpeaking: ((Boolean) -> Unit)? = null

    // ---------- Shared with background threads ----------
    private val aMuted = AtomicBoolean(false)
    private val aDeaf = AtomicBoolean(false)
    private val aHeadset = AtomicBoolean(false)
    private val pttBits = AtomicInteger(0)
    private val mixer = Mixer()

    private var myRoom: Int? = null
    private var members: Map<Int, String> = emptyMap()

    private class Conn(val host: String, val port: Int) {
        val running = AtomicBoolean(true)
        @Volatile var socket: Socket? = null
        @Volatile var writer: BufferedWriter? = null
        @Volatile var udp: DatagramSocket? = null
        @Volatile var session = 0
        @Volatile var token = 0
        @Volatile var welcomed = false
        private val sendExec: ExecutorService = Executors.newSingleThreadExecutor()

        fun send(o: JSONObject) {
            if (!running.get()) return
            try {
                sendExec.execute {
                    try {
                        writer?.apply {
                            write(o.toString())
                            write("\n")
                            flush()
                        }
                    } catch (_: Exception) {
                    }
                }
            } catch (_: Exception) {
            }
        }

        fun close() {
            running.set(false)
            try { socket?.close() } catch (_: Exception) {}
            try { udp?.close() } catch (_: Exception) {}
            sendExec.shutdown()
        }
    }

    @Volatile private var conn: Conn? = null

    private fun ui(block: () -> Unit) {
        main.post(block)
    }

    private fun addChat(line: String) {
        chat.add(line)
        while (chat.size > 300) chat.removeAt(0)
    }

    // ---------- Connecting ----------

    fun connect(host: String, port: Int, name: String, password: String) {
        if (status != Status.DISCONNECTED) return
        error = null
        chat.clear()
        rooms = emptyList()
        users = emptyList()
        myRoom = null
        members = emptyMap()
        muted = false
        deaf = false
        headsetLatched = false
        pingMs = null
        aMuted.set(false)
        aDeaf.set(false)
        aHeadset.set(false)
        pttBits.set(0)
        mixer.clear()
        status = Status.CONNECTING
        val c = Conn(host, port)
        conn = c
        VoiceService.start(app)
        thread(name = "squadtalk-tcp") { runConnection(c, name, password) }
    }

    fun disconnect() {
        val c = conn
        conn = null
        c?.close()
        resetAfterDisconnect()
    }

    private fun resetAfterDisconnect() {
        status = Status.DISCONNECTED
        selfSpeaking = false
        speaking = emptySet()
        isOwner = false
        pttBits.set(0)
        aHeadset.set(false)
        headsetLatched = false
        mixer.clear()
        VoiceService.stop(app)
    }

    private fun fail(c: Conn, message: String) {
        c.close()
        ui {
            if (conn === c) {
                conn = null
                resetAfterDisconnect()
                error = message
            }
        }
    }

    private fun runConnection(c: Conn, name: String, password: String) {
        try {
            val s = Socket()
            c.socket = s
            s.tcpNoDelay = true
            s.connect(InetSocketAddress(c.host, c.port), 6000)
            s.soTimeout = 10000
            val reader = BufferedReader(InputStreamReader(s.getInputStream(), Charsets.UTF_8))
            c.writer = BufferedWriter(OutputStreamWriter(s.getOutputStream(), Charsets.UTF_8))
            c.send(
                JSONObject()
                    .put("t", "Hello")
                    .put("version", VERSION)
                    .put("name", name)
                    .put("secret", prefs.secret)
                    .put("password", password)
            )
            val first = reader.readLine() ?: throw IOException("The server did not reply.")
            val w = JSONObject(first)
            when (w.optString("t")) {
                "Welcome" -> {
                    c.session = w.getLong("session").toInt()
                    c.token = w.getLong("udp_token").toInt()
                    val owner = w.optBoolean("owner")
                    val session = c.session
                    ui {
                        if (conn === c) {
                            me = session
                            isOwner = owner
                        }
                    }
                }
                "Denied" -> throw DeniedException(w.optString("reason"))
                else -> throw IOException("The server sent something unexpected.")
            }
            c.welcomed = true

            val u = DatagramSocket()
            u.connect(s.inetAddress, c.port)
            u.soTimeout = 500
            c.udp = u

            ui {
                if (conn === c) {
                    status = Status.CONNECTED
                    addChat("Connected to the server.")
                    if (prefs.voiceActivation) addChat("Voice activation is on: just talk.")
                    else addChat("Hold the big button to talk (or use the floating button / Volume Down).")
                    main.post(ticker)
                }
            }
            thread(name = "squadtalk-udp") { udpLoop(c) }
            thread(name = "squadtalk-ping") { pingLoop(c) }
            thread(name = "squadtalk-mic") { captureLoop(c) }
            thread(name = "squadtalk-speaker") { playbackLoop(c) }

            // Server answers our pings every 5 s, so 20 s of silence means it's gone.
            s.soTimeout = 20000
            while (c.running.get()) {
                val line = reader.readLine() ?: break
                handleLine(c, line)
            }
            if (c.running.get()) fail(c, "Lost the connection to the server.")
        } catch (e: DeniedException) {
            fail(c, "The server did not let you in:\n\n${e.message}")
        } catch (e: Exception) {
            if (c.running.get()) {
                if (c.welcomed) {
                    fail(c, "Lost the connection to the server.")
                } else {
                    fail(
                        c,
                        "Could not connect to ${c.host} (port ${c.port}).\n\n" +
                            "Check the address, and that the server is running.\n\n(${e.message ?: e.javaClass.simpleName})"
                    )
                }
            }
        }
    }

    private fun handleLine(c: Conn, line: String) {
        val o = try { JSONObject(line) } catch (e: Exception) { return }
        when (o.optString("t")) {
            "State" -> {
                val ra = o.optJSONArray("rooms")
                val ua = o.optJSONArray("users")
                val newRooms = ArrayList<RoomInfo>()
                if (ra != null) for (i in 0 until ra.length()) {
                    val r = ra.getJSONObject(i)
                    newRooms.add(RoomInfo(r.getInt("id"), r.optString("name")))
                }
                val newUsers = ArrayList<UserInfo>()
                if (ua != null) for (i in 0 until ua.length()) {
                    val x = ua.getJSONObject(i)
                    newUsers.add(
                        UserInfo(
                            session = x.getLong("session").toInt(),
                            uid = x.optString("uid"),
                            name = x.optString("name"),
                            room = x.getInt("room"),
                            mute = x.optBoolean("mute"),
                            deaf = x.optBoolean("deaf"),
                            serverMute = x.optBoolean("server_mute"),
                            owner = x.optBoolean("owner"),
                        )
                    )
                }
                ui { if (conn === c) applyState(newRooms, newUsers) }
            }
            "Chat" -> {
                val n = o.optString("name")
                val t = o.optString("text")
                ui { if (conn === c) addChat("$n: $t") }
            }
            "Kicked" -> fail(c, o.optString("reason", "You were removed from the server."))
            else -> {}
        }
    }

    private fun applyState(newRooms: List<RoomInfo>, newUsers: List<UserInfo>) {
        rooms = newRooms
        users = newUsers
        mixer.setVolumes(newUsers.associate { it.session to prefs.volumeFor(it.uid) / 100f })
        mixer.retain(newUsers.map { it.session }.toSet())

        val mine = newUsers.firstOrNull { it.session == me }?.room
        val now = newUsers.filter { it.room == mine && it.session != me }.associate { it.session to it.name }
        if (mine != null) {
            if (mine != myRoom) {
                val rn = newRooms.firstOrNull { it.id == mine }?.name ?: ""
                addChat("— You are in room \"$rn\" —")
            } else {
                var joined = false
                var left = false
                for ((s, n) in now) if (s !in members) {
                    addChat("$n joined the room.")
                    joined = true
                }
                for ((s, n) in members) if (s !in now) {
                    addChat("$n left the room.")
                    left = true
                }
                if (prefs.joinSounds) {
                    if (joined) mixer.effect(Sounds.join)
                    if (left) mixer.effect(Sounds.leave)
                }
            }
        }
        myRoom = mine
        members = now
    }

    private val ticker = object : Runnable {
        override fun run() {
            if (status == Status.CONNECTED) {
                val s = mixer.speaking()
                if (s != speaking) speaking = s
                main.postDelayed(this, 200)
            }
        }
    }

    // ---------- Network threads ----------

    private fun header(c: Conn, kind: Int, extra: Int): ByteBuffer =
        ByteBuffer.allocate(9 + extra).order(ByteOrder.LITTLE_ENDIAN)
            .put(kind.toByte()).putInt(c.session).putInt(c.token)

    private fun sendUdp(c: Conn, bb: ByteBuffer) {
        val u = c.udp ?: return
        try {
            u.send(DatagramPacket(bb.array(), bb.position()))
        } catch (_: Exception) {
        }
    }

    private fun pingLoop(c: Conn) {
        var i = 0
        while (c.running.get()) {
            sendUdp(c, header(c, 2, 8).putLong(System.currentTimeMillis()))
            if (i % 5 == 0) c.send(JSONObject().put("t", "Ping"))
            i++
            try { Thread.sleep(1000) } catch (_: InterruptedException) { break }
        }
    }

    private fun udpLoop(c: Conn) {
        val u = c.udp ?: return
        val buf = ByteArray(1500)
        val pkt = DatagramPacket(buf, buf.size)
        val pcm = ShortArray(5760)
        val decoders = HashMap<Int, Long>()
        try {
            while (c.running.get()) {
                try {
                    pkt.length = buf.size
                    u.receive(pkt)
                } catch (_: SocketTimeoutException) {
                    continue
                } catch (e: Exception) {
                    if (!c.running.get()) break else continue
                }
                val n = pkt.length
                if (n < 1) continue
                val bb = ByteBuffer.wrap(buf, 0, n).order(ByteOrder.LITTLE_ENDIAN)
                when (buf[0].toInt()) {
                    2 -> if (n >= 9) {
                        val sent = bb.getLong(1)
                        val rtt = (System.currentTimeMillis() - sent).coerceIn(0, 9999).toInt()
                        ui { if (conn === c) pingMs = rtt }
                    }
                    1 -> if (n > 7 && !aDeaf.get()) {
                        val sender = bb.getInt(1)
                        var d = decoders[sender]
                        if (d == null) {
                            d = Opus.decoderCreate()
                            if (d == 0L) continue
                            decoders[sender] = d
                        }
                        val samples = Opus.decode(d, buf, 7, n - 7, pcm)
                        if (samples > 0) mixer.push(sender, pcm, samples)
                    }
                }
            }
        } finally {
            decoders.values.forEach { Opus.decoderDestroy(it) }
        }
    }

    // ---------- Audio threads ----------

    private fun captureLoop(c: Conn) {
        val minBuf = AudioRecord.getMinBufferSize(RATE, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT)
        val rec = try {
            AudioRecord(
                MediaRecorder.AudioSource.VOICE_COMMUNICATION,
                RATE,
                AudioFormat.CHANNEL_IN_MONO,
                AudioFormat.ENCODING_PCM_16BIT,
                max(minBuf, FRAME * 2 * 4)
            )
        } catch (e: SecurityException) {
            ui { addChat("⚠ Microphone permission is missing. Allow it in Android settings.") }
            return
        } catch (e: Exception) {
            ui { addChat("⚠ Could not open the microphone: ${e.message}") }
            return
        }
        if (rec.state != AudioRecord.STATE_INITIALIZED) {
            ui { addChat("⚠ Could not open the microphone.") }
            rec.release()
            return
        }
        val aec = if (AcousticEchoCanceler.isAvailable()) {
            try { AcousticEchoCanceler.create(rec.audioSessionId)?.apply { enabled = true } } catch (e: Exception) { null }
        } else null
        val enc = Opus.encoderCreate(32000)
        val frame = ShortArray(FRAME)
        val out = ByteArray(1500)
        var seq = 0
        var tx = false
        var hang = 0
        var lastLevel = 0L
        try {
            rec.startRecording()
            while (c.running.get()) {
                var got = 0
                while (got < FRAME && c.running.get()) {
                    val r = rec.read(frame, got, FRAME - got)
                    if (r <= 0) break
                    got += r
                }
                if (got < FRAME) {
                    try { Thread.sleep(10) } catch (_: InterruptedException) { break }
                    continue
                }
                var energy = 0.0
                for (v in frame) energy += v.toDouble() * v
                energy /= FRAME * 32768.0 * 32768.0
                val db = if (energy > 1e-12) (10 * log10(energy)).toInt() else -100
                val now = System.currentTimeMillis()
                if (now - lastLevel > 200) {
                    lastLevel = now
                    ui { micLevelDb = db }
                }

                val va = prefs.voiceActivation
                val want = when {
                    aMuted.get() || aDeaf.get() -> { hang = 0; false }
                    va -> when {
                        db > prefs.vaThreshold -> { hang = 20; true } // keep talking 400 ms after you stop
                        hang > 0 -> { hang--; true }
                        else -> false
                    }
                    else -> pttBits.get() != 0 || aHeadset.get()
                }
                if (want != tx) {
                    tx = want
                    ui {
                        selfSpeaking = want
                        onSelfSpeaking?.invoke(want)
                    }
                    if (!va && prefs.beeps) mixer.effect(if (want) Sounds.beepOn else Sounds.beepOff)
                }
                if (tx && enc != 0L) {
                    val n = Opus.encode(enc, frame, out)
                    if (n > 0) {
                        val bb = header(c, 1, 2 + n).putShort(seq.toShort()).put(out, 0, n)
                        sendUdp(c, bb)
                        seq = (seq + 1) and 0xFFFF
                    }
                }
            }
        } catch (e: Exception) {
            ui { addChat("⚠ Microphone stopped: ${e.message}") }
        } finally {
            try { rec.stop() } catch (_: Exception) {}
            rec.release()
            aec?.release()
            Opus.encoderDestroy(enc)
            ui {
                selfSpeaking = false
                onSelfSpeaking?.invoke(false)
            }
        }
    }

    private fun playbackLoop(c: Conn) {
        val minBuf = AudioTrack.getMinBufferSize(RATE, AudioFormat.CHANNEL_OUT_MONO, AudioFormat.ENCODING_PCM_16BIT)
        val track = try {
            AudioTrack.Builder()
                .setAudioAttributes(
                    AudioAttributes.Builder()
                        .setUsage(AudioAttributes.USAGE_MEDIA)
                        .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                        .build()
                )
                .setAudioFormat(
                    AudioFormat.Builder()
                        .setSampleRate(RATE)
                        .setChannelMask(AudioFormat.CHANNEL_OUT_MONO)
                        .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                        .build()
                )
                .setBufferSizeInBytes(max(minBuf, FRAME * 2 * 3))
                .setTransferMode(AudioTrack.MODE_STREAM)
                .setPerformanceMode(AudioTrack.PERFORMANCE_MODE_LOW_LATENCY)
                .build()
        } catch (e: Exception) {
            ui { addChat("⚠ Could not open the speaker: ${e.message}") }
            return
        }
        val out = ShortArray(FRAME)
        try {
            track.play()
            while (c.running.get()) {
                mixer.mix(out)
                track.write(out, 0, FRAME)
            }
        } catch (e: Exception) {
            ui { addChat("⚠ Speaker stopped: ${e.message}") }
        } finally {
            try { track.stop() } catch (_: Exception) {}
            track.release()
        }
    }

    // ---------- Actions from the screen ----------

    fun setPtt(bit: Int, on: Boolean) {
        if (on) pttBits.updateAndGet { it or bit } else pttBits.updateAndGet { it and bit.inv() }
    }

    fun toggleHeadsetTalk() {
        if (status != Status.CONNECTED || !prefs.headsetButton) return
        val v = !aHeadset.get()
        aHeadset.set(v)
        headsetLatched = v
    }

    fun toggleMute() {
        muted = !muted
        aMuted.set(muted)
        conn?.send(JSONObject().put("t", "SetState").put("mute", muted).put("deaf", deaf))
    }

    fun toggleDeaf() {
        deaf = !deaf
        aDeaf.set(deaf)
        mixer.deaf = deaf
        conn?.send(JSONObject().put("t", "SetState").put("mute", muted).put("deaf", deaf))
    }

    fun joinRoom(id: Int) {
        conn?.send(JSONObject().put("t", "Join").put("room", id))
    }

    fun sendChat(text: String) {
        conn?.send(JSONObject().put("t", "Chat").put("text", text))
    }

    fun kick(session: Int) {
        conn?.send(JSONObject().put("t", "Kick").put("session", session.toLong() and 0xFFFFFFFFL))
    }

    fun serverMute(session: Int, muted: Boolean) {
        conn?.send(JSONObject().put("t", "ServerMute").put("session", session.toLong() and 0xFFFFFFFFL).put("muted", muted))
    }

    fun move(session: Int, room: Int) {
        conn?.send(JSONObject().put("t", "Move").put("session", session.toLong() and 0xFFFFFFFFL).put("room", room))
    }

    fun addRoom(name: String) {
        conn?.send(JSONObject().put("t", "AddRoom").put("name", name))
    }

    fun deleteRoom(id: Int) {
        conn?.send(JSONObject().put("t", "DeleteRoom").put("room", id))
    }

    fun setUserVolume(u: UserInfo, percent: Int) {
        prefs.setVolume(u.uid, percent)
        mixer.setVolume(u.session, percent / 100f)
    }
}
