# SquadTalk

Lightweight voice chat for gaming that you host yourself on your own PC. It's built to use as little memory and CPU as possible.

**Features:** push-to-talk on any keyboard key or mouse button, voice-activated mode, rooms that the owner creates, mute and deafen, volume per person, a server password, owner tools (kick, move, server-mute), room text chat, and talk/join/leave sounds.

---

## 1. Get the app

1. Open this project on GitHub and click **Releases** on the right side.
2. Under **SquadTalk — latest build**, click **SquadTalk.exe** to download it.
3. Put it anywhere you like, for example on your Desktop. There's nothing to install.

> **Windows may warn you the first time** ("Windows protected your PC"). This happens because the app is new and unsigned. Click **More info**, then **Run anyway**.

**Giving it to friends:** this project is private, so friends can't open the Releases page. Send them the `SquadTalk.exe` file directly, using WhatsApp, Google Drive or a pen drive.

## Android app

**SquadTalk.apk** on the Releases page runs on Android 10 and newer, and joins the same server and rooms. Setup and tips are in **[android/GUIDE.md](android/GUIDE.md)**.

## 24/7 server on Oracle Cloud (recommended)

Run the server on a free Oracle Cloud machine, so it's always online and friends install nothing but SquadTalk. Follow **[oracle/GUIDE.md](oracle/GUIDE.md)**. The steps below are for hosting on your own PC instead.

## 2. Set up Tailscale (once, everyone)

Tailscale creates a private, secure network between your PCs, so friends can reach your server without any router settings.

1. Everyone installs Tailscale from **tailscale.com/download** and signs in.
2. **Host:** in the Tailscale admin page (login.tailscale.com), go to **Users** and invite your friends. Alternatively, use **Share** on your PC's machine page to share just your PC with them.
3. When Tailscale is running, your PC's Tailscale address looks like `100.x.y.z`. You can see it by clicking the Tailscale icon in the taskbar.

## 3. Host the server (owner)

1. Open SquadTalk, type your name, and (optionally) type a password.
2. Click **Host a server on this PC**.
3. If Windows Firewall asks, tick **both Private and Public** and click **Allow access**.
4. The chat box shows the address your friends should use, for example `100.101.102.103`.

You're the owner. Use **Owner tools** to add or delete rooms and to kick, move or server-mute people.

## 4. Join (friends)

1. Open SquadTalk and type your name.
2. Type the host's Tailscale address and the password, then click **Join server**.

## 5. Talking

- The default push-to-talk key is **`** (left of 1). Change it in **Settings → Change key…**, and press any key or mouse button, including the side buttons.
- Or pick **Voice activation** in Settings and adjust the level until it only picks up your voice.
- Double-click a room to move into it. Select a person to change their volume.
- **If push-to-talk doesn't work inside a game:** some games run "as administrator". Right-click SquadTalk.exe, choose **Run as administrator**, and try again.

Settings are saved in `%APPDATA%\SquadTalk\config.json`.

---

## For the technically curious

- Written in Rust with native Win32 controls: no browser engine and no GPU use.
- Voice is Opus at 32 kbps, 20 ms frames. The server relays packets without decoding them.
- TCP port 9987 carries control and chat, and UDP port 9987 carries voice.
- Every push to `main` is built on GitHub's Windows machines and published to Releases.
