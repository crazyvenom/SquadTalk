package com.squadtalk.app

import android.content.Intent
import android.net.Uri
import android.provider.Settings
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

private val Accent = Color(0xFF7C9CFF)
private val Talking = Color(0xFF3DDC84)

@Composable
fun SquadTalkApp(act: MainActivity) {
    MaterialTheme(colorScheme = darkColorScheme(primary = Accent, secondary = Talking)) {
        var showSettings by remember { mutableStateOf(false) }
        Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
            Box(Modifier.fillMaxSize().safeDrawingPadding()) {
                when {
                    showSettings -> SettingsScreen(act) { showSettings = false }
                    Session.status == Status.CONNECTED -> MainScreen { showSettings = true }
                    else -> ConnectScreen(act) { showSettings = true }
                }
            }
        }
        val err = Session.error
        if (err != null) {
            AlertDialog(
                onDismissRequest = { Session.error = null },
                confirmButton = { TextButton(onClick = { Session.error = null }) { Text("OK") } },
                title = { Text("SquadTalk") },
                text = { Text(err) },
            )
        }
    }
}

// ---------------- Connect ----------------

@Composable
fun ConnectScreen(act: MainActivity, onSettings: () -> Unit) {
    val p = Session.prefs
    var name by remember { mutableStateOf(p.name) }
    var address by remember { mutableStateOf(p.address) }
    var port by remember { mutableStateOf(p.port.toString()) }
    var password by remember { mutableStateOf(p.password) }
    val connecting = Session.status == Status.CONNECTING

    Column(
        Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text("SquadTalk", style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.Bold)
        Text("Lightweight voice chat for gaming", color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(8.dp))
        OutlinedTextField(
            value = name, onValueChange = { name = it.take(24) },
            label = { Text("Your name") }, singleLine = true, modifier = Modifier.fillMaxWidth(),
        )
        OutlinedTextField(
            value = address, onValueChange = { address = it.trim() },
            label = { Text("Server address") }, placeholder = { Text("e.g. 140.238.12.34") },
            singleLine = true, modifier = Modifier.fillMaxWidth(),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
        )
        OutlinedTextField(
            value = port, onValueChange = { port = it.filter(Char::isDigit).take(5) },
            label = { Text("Port") }, singleLine = true, modifier = Modifier.fillMaxWidth(),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
        )
        OutlinedTextField(
            value = password, onValueChange = { password = it },
            label = { Text("Server password") }, singleLine = true, modifier = Modifier.fillMaxWidth(),
            visualTransformation = PasswordVisualTransformation(),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
        )
        Button(
            onClick = {
                val pt = port.toIntOrNull() ?: 0
                val problem = when {
                    name.isBlank() -> "Please type your name first."
                    address.isBlank() -> "Type the server address first."
                    pt !in 1..65535 -> "The port must be a number like 9987."
                    else -> null
                }
                if (problem != null) {
                    Session.error = problem
                } else {
                    p.name = name.trim()
                    p.address = address
                    p.port = pt
                    p.password = password
                    act.withPermissions { Session.connect(address, pt, name.trim(), password) }
                }
            },
            enabled = !connecting,
            modifier = Modifier.fillMaxWidth().height(54.dp),
        ) { Text(if (connecting) "Connecting…" else "Join server", fontSize = 17.sp) }
        if (connecting) {
            TextButton(onClick = { Session.disconnect() }, modifier = Modifier.fillMaxWidth()) { Text("Cancel") }
        }
        OutlinedButton(onClick = onSettings, modifier = Modifier.fillMaxWidth()) { Text("Talk settings") }
        Text(
            "Ask the server owner for the address and password.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

// ---------------- Main ----------------

@Composable
fun MainScreen(onSettings: () -> Unit) {
    var tab by remember { mutableIntStateOf(0) }
    var selectedUser by remember { mutableStateOf<Int?>(null) }
    var roomToDelete by remember { mutableStateOf<RoomInfo?>(null) }

    Column(Modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text("SquadTalk", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold)
                val ping = Session.pingMs
                Text(
                    if (ping != null) "Connected · ping $ping ms" else "Connected",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            TextButton(onClick = onSettings) { Text("Settings") }
            TextButton(onClick = { Session.disconnect() }) { Text("Leave") }
        }
        TabRow(selectedTabIndex = tab) {
            Tab(selected = tab == 0, onClick = { tab = 0 }, text = { Text("Rooms") })
            Tab(selected = tab == 1, onClick = { tab = 1 }, text = { Text("Chat") })
        }
        Box(Modifier.weight(1f).fillMaxWidth()) {
            if (tab == 0) RoomsList(onUser = { selectedUser = it }, onDeleteRoom = { roomToDelete = it })
            else ChatPane()
        }
        TalkBar()
    }

    selectedUser?.let { s -> UserDialog(s) { selectedUser = null } }
    roomToDelete?.let { r ->
        AlertDialog(
            onDismissRequest = { roomToDelete = null },
            confirmButton = {
                TextButton(onClick = { Session.deleteRoom(r.id); roomToDelete = null }) { Text("Delete") }
            },
            dismissButton = { TextButton(onClick = { roomToDelete = null }) { Text("Cancel") } },
            title = { Text("Delete \"${r.name}\"?") },
            text = { Text("People in this room will move to the first room.") },
        )
    }
}

@Composable
fun RoomsList(onUser: (Int) -> Unit, onDeleteRoom: (RoomInfo) -> Unit) {
    val rooms = Session.rooms
    val users = Session.users
    val speaking = Session.speaking
    val me = Session.me
    val selfSpeaking = Session.selfSpeaking
    val owner = Session.isOwner
    var newRoom by remember { mutableStateOf("") }

    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(12.dp)) {
        rooms.forEachIndexed { index, r ->
            val people = users.filter { it.room == r.id }.sortedBy { it.name.lowercase() }
            item(key = "room-${r.id}") {
                Row(
                    Modifier.fillMaxWidth()
                        .clip(RoundedCornerShape(10.dp))
                        .background(MaterialTheme.colorScheme.surfaceVariant)
                        .clickable { Session.joinRoom(r.id) }
                        .padding(horizontal = 14.dp, vertical = 10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(r.name, Modifier.weight(1f), fontWeight = FontWeight.SemiBold)
                    Text("${people.size}", color = MaterialTheme.colorScheme.onSurfaceVariant)
                    if (owner && index > 0) {
                        TextButton(onClick = { onDeleteRoom(r) }) { Text("Delete") }
                    }
                }
            }
            items(people, key = { "user-${it.session}" }) { u ->
                val talking = if (u.session == me) selfSpeaking else u.session in speaking
                Row(
                    Modifier.fillMaxWidth()
                        .clickable { onUser(u.session) }
                        .padding(start = 26.dp, top = 9.dp, bottom = 9.dp, end = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Box(
                        Modifier.size(12.dp).clip(CircleShape)
                            .background(if (talking) Talking else Color.Gray.copy(alpha = 0.4f))
                    )
                    Spacer(Modifier.width(10.dp))
                    Text(buildString {
                        append(u.name)
                        if (u.session == me) append("  (you)")
                        if (u.owner) append("  ★")
                        if (u.serverMute) append("  [server-muted]") else if (u.mute) append("  [mic off]")
                        if (u.deaf) append("  [deafened]")
                    })
                }
            }
            item(key = "gap-${r.id}") { Spacer(Modifier.height(8.dp)) }
        }
        if (owner) {
            item(key = "add-room") {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    OutlinedTextField(
                        value = newRoom, onValueChange = { newRoom = it.take(40) },
                        label = { Text("New room name") }, singleLine = true, modifier = Modifier.weight(1f),
                    )
                    Spacer(Modifier.width(8.dp))
                    Button(onClick = {
                        if (newRoom.isNotBlank()) {
                            Session.addRoom(newRoom.trim())
                            newRoom = ""
                        }
                    }) { Text("Add") }
                }
            }
        }
        item(key = "hint") {
            Text(
                if (owner) "Tap a room to join it. Tap a person for volume and owner tools."
                else "Tap a room to join it. Tap a person to change their volume.",
                Modifier.padding(top = 12.dp),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
fun UserDialog(session: Int, onClose: () -> Unit) {
    val u = Session.users.firstOrNull { it.session == session }
    if (u == null) {
        LaunchedEffect(Unit) { onClose() }
        return
    }
    val isMe = u.session == Session.me
    var vol by remember(u.uid) { mutableFloatStateOf(Session.prefs.volumeFor(u.uid).toFloat()) }

    AlertDialog(
        onDismissRequest = onClose,
        confirmButton = { TextButton(onClick = onClose) { Text("Close") } },
        title = { Text(u.name) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                if (isMe) {
                    Text("This is you.")
                } else {
                    Text("Volume: ${vol.toInt()}%")
                    Slider(
                        value = vol,
                        onValueChange = { vol = it; Session.setUserVolume(u, it.toInt()) },
                        valueRange = 0f..200f,
                    )
                }
                if (Session.isOwner) {
                    HorizontalDivider()
                    Text("Owner tools", fontWeight = FontWeight.SemiBold)
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        if (!isMe) {
                            OutlinedButton(onClick = { Session.kick(u.session); onClose() }) { Text("Kick") }
                        }
                        OutlinedButton(onClick = { Session.serverMute(u.session, !u.serverMute) }) {
                            Text(if (u.serverMute) "Unmute" else "Server-mute")
                        }
                    }
                    val others = Session.rooms.filter { it.id != u.room }
                    if (others.isNotEmpty()) {
                        Text("Move to room:")
                        others.forEach { r ->
                            TextButton(onClick = { Session.move(u.session, r.id) }) { Text(r.name) }
                        }
                    }
                }
            }
        },
    )
}

@Composable
fun ChatPane() {
    var text by remember { mutableStateOf("") }
    val listState = rememberLazyListState()
    val count = Session.chat.size
    LaunchedEffect(count) { if (count > 0) listState.animateScrollToItem(count - 1) }

    Column(Modifier.fillMaxSize()) {
        LazyColumn(Modifier.weight(1f).fillMaxWidth(), state = listState, contentPadding = PaddingValues(12.dp)) {
            items(Session.chat) { line -> Text(line, Modifier.padding(vertical = 3.dp)) }
        }
        Row(Modifier.padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(
                value = text, onValueChange = { text = it.take(500) },
                placeholder = { Text("Message your room") }, singleLine = true, modifier = Modifier.weight(1f),
            )
            Spacer(Modifier.width(8.dp))
            Button(onClick = {
                if (text.isNotBlank()) {
                    Session.sendChat(text.trim())
                    text = ""
                }
            }) { Text("Send") }
        }
    }
}

@Composable
fun TalkBar() {
    Column(
        Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.surfaceVariant).padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = { Session.toggleMute() }, modifier = Modifier.weight(1f)) {
                Text(if (Session.muted) "Unmute mic" else "Mute mic")
            }
            OutlinedButton(onClick = { Session.toggleDeaf() }, modifier = Modifier.weight(1f)) {
                Text(if (Session.deaf) "Undeafen" else "Deafen")
            }
        }
        val talking = Session.selfSpeaking
        if (Session.vaMode) {
            Text(
                if (talking) "● Talking" else "Voice activation is on: just talk.",
                Modifier.fillMaxWidth(), textAlign = TextAlign.Center,
                color = if (talking) Talking else MaterialTheme.colorScheme.onSurfaceVariant,
            )
        } else {
            Box(
                Modifier.fillMaxWidth().height(86.dp)
                    .clip(RoundedCornerShape(43.dp))
                    .background(if (talking) Talking else Accent)
                    .pointerInput(Unit) {
                        detectTapGestures(onPress = {
                            Session.setPtt(Session.PTT_BUTTON, true)
                            try {
                                tryAwaitRelease()
                            } finally {
                                Session.setPtt(Session.PTT_BUTTON, false)
                            }
                        })
                    },
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    if (talking) "Talking…" else "Hold to talk",
                    color = Color(0xFF0B1020), fontSize = 21.sp, fontWeight = FontWeight.Bold,
                )
            }
            if (Session.headsetLatched) {
                Text(
                    "Earphone button: you're talking. Press it again to stop.",
                    Modifier.fillMaxWidth(), textAlign = TextAlign.Center, color = Talking,
                    style = MaterialTheme.typography.bodySmall,
                )
            }
        }
    }
}

// ---------------- Settings ----------------

@Composable
fun SettingsScreen(act: MainActivity, onBack: () -> Unit) {
    BackHandler(onBack = onBack)
    val p = Session.prefs
    var va by remember { mutableStateOf(p.voiceActivation) }
    var threshold by remember { mutableFloatStateOf(p.vaThreshold.toFloat()) }
    var beeps by remember { mutableStateOf(p.beeps) }
    var joinSounds by remember { mutableStateOf(p.joinSounds) }
    var floating by remember { mutableStateOf(p.floating) }
    var volumeKey by remember { mutableStateOf(p.volumeKey) }
    var headset by remember { mutableStateOf(p.headsetButton) }

    fun setVa(on: Boolean) {
        va = on
        p.voiceActivation = on
        Session.vaMode = on
    }

    Column(
        Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            TextButton(onClick = onBack) { Text("← Back") }
            Text("Talk settings", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold)
        }

        Text("How do you want to talk?", fontWeight = FontWeight.SemiBold)
        Row(Modifier.fillMaxWidth().clickable { setVa(false) }, verticalAlignment = Alignment.CenterVertically) {
            RadioButton(selected = !va, onClick = { setVa(false) })
            Text("Push-to-talk: hold a button to talk")
        }
        Row(Modifier.fillMaxWidth().clickable { setVa(true) }, verticalAlignment = Alignment.CenterVertically) {
            RadioButton(selected = va, onClick = { setVa(true) })
            Text("Voice activation: talks when you speak")
        }
        if (va) {
            Text("Sensitivity: ${threshold.toInt()} dB (move left to pick up quieter speech)")
            Slider(
                value = threshold,
                onValueChange = { threshold = it; p.vaThreshold = it.toInt() },
                valueRange = -70f..-10f,
            )
            Text(
                if (Session.status == Status.CONNECTED) "Your mic right now: ${Session.micLevelDb} dB"
                else "Join a server to see your mic level here.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }

        HorizontalDivider()
        Text("Push-to-talk buttons", fontWeight = FontWeight.SemiBold)
        Text(
            "The big \"Hold to talk\" button inside the app always works.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        SettingSwitch(
            "Floating talk button over games",
            "A bubble that stays on top of other apps. Hold it to talk, drag it to move it. " +
                "Android will ask you to allow \"Display over other apps\".",
            floating,
        ) { on ->
            floating = on
            p.floating = on
            if (on && !Settings.canDrawOverlays(act)) {
                act.startActivity(
                    Intent(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, Uri.parse("package:" + act.packageName))
                )
            }
            VoiceService.instance?.refreshBubble()
        }
        SettingSwitch(
            "Volume Down = talk",
            "Hold Volume Down to talk while SquadTalk is open on screen.",
            volumeKey,
        ) { on -> volumeKey = on; p.volumeKey = on }
        SettingSwitch(
            "Earphone button = talk on/off",
            "Press the button on wired or Bluetooth earphones to start talking, press again to stop. " +
                "Works while you're in a game (unless the game takes over the button).",
            headset,
        ) { on ->
            headset = on
            p.headsetButton = on
            VoiceService.instance?.refreshMediaSession()
        }

        HorizontalDivider()
        Text("Sounds", fontWeight = FontWeight.SemiBold)
        SettingSwitch("Beep when you start and stop talking", "", beeps) { on -> beeps = on; p.beeps = on }
        SettingSwitch("Sound when people join or leave your room", "", joinSounds) { on ->
            joinSounds = on; p.joinSounds = on
        }
        Spacer(Modifier.height(20.dp))
    }
}

@Composable
fun SettingSwitch(title: String, description: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f).padding(end = 12.dp)) {
            Text(title)
            if (description.isNotEmpty()) {
                Text(
                    description,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        Switch(checked = checked, onCheckedChange = onChange)
    }
}
