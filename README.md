# SquadTalk

Lightweight voice chat for gaming, on your own 24/7 server. It's built to use as little memory and CPU as possible.

**Features:**
- Push-to-talk on any keyboard key or mouse button, or a voice-activated mode
- Rooms that the owner creates
- Mute and deafen, plus volume for each person
- A server password
- Owner tools: kick, move, server-mute
- Text chat for each room
- Talk, join and leave sounds

**Apps:** Windows (`SquadTalk.exe`) and Android (`SquadTalk.apk`). Everyone joins the same server and rooms.

---

## How it works

```
 Your friends (PC or phone)  ──►  Your SquadTalk server on Oracle Cloud (free, 24/7)
```

- The **server owner** sets up the server once. Follow **[oracle/GUIDE.md](oracle/GUIDE.md)**.
- **Everyone else** installs SquadTalk and types the server's **address** and **password**. Nothing else is needed.

## Get the app

Go to the **[Releases page](https://github.com/crazyvenom/SquadTalk/releases)**, open **latest**, then download:

- **SquadTalk.exe** for Windows. There's nothing to install: put it on your Desktop and double-click it.
  > The first time, Windows may say "Windows protected your PC". Click **More info**, then **Run anyway**. This happens because the app is new and unsigned.
- **SquadTalk.apk** for Android 10 and newer. Install steps and phone tips are in **[android/GUIDE.md](android/GUIDE.md)**.

## Join the server

1. Open SquadTalk and type your name.
2. Type the **server address** and the **password** that the owner gave you. Leave the port as **9987**.
3. Click **Join server**.

**Owner:** the first time, type your **owner code** as the password. The server then remembers your device as the owner.

## Talking (PC)

- **Push-to-talk:** the default key is **`** (left of 1). To change it, go to **Settings**, click **Change key…**, and press any key or mouse button, including the side buttons.
- **Voice activation:** pick it in **Settings**, then adjust the level until it only picks up your voice.
- **Rooms and volume:** double-click a room to move into it. Select a person to change their volume.
- **If push-to-talk doesn't work inside a game:** some games run "as administrator". Right-click SquadTalk.exe, choose **Run as administrator**, and try again.

Settings are saved in `%APPDATA%\SquadTalk\config.json`.

## Advanced: temporary server on your own PC

The Windows app also has **Host on this PC (same Wi-Fi only)**, for quick tests with people on the same home network. Friends elsewhere should use the Oracle server.

---

## For the technically curious

- The Windows app is written in Rust with native Win32 controls: no browser engine and no GPU use.
- The Android app is Kotlin and Jetpack Compose, using the same Opus codec through a small native library.
- Voice is Opus at 32 kbps with 20 ms frames, and the server relays packets without decoding them.
- TCP port 9987 carries control and chat, and UDP port 9987 carries voice.
- Every push to `main` is built on GitHub (Windows app, Linux server, Android app), tested, and published to Releases.
