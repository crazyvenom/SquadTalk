# Run your SquadTalk server 24/7 on Oracle Cloud (free)

When you're done, your server runs all the time, even when your PC is off. Friends only install SquadTalk and type your server's address.

**Time needed:** about 30 minutes, once.
**Cost:** free. Oracle's "Always Free" small server is more than enough.

> Oracle changes its website sometimes. If a button has a slightly different name or place, look for the closest match, or send a screenshot to Claude.

---

## Step 0 — Make the SquadTalk project public (30 seconds)

The Oracle server downloads SquadTalk from your GitHub Releases page, so the project must be public.

1. Go to **github.com/crazyvenom/SquadTalk**.
2. Click **Settings**, the gear tab at the top.
3. Scroll to the bottom, to the **Danger Zone**, and click **Change visibility**.
4. Choose **Change to public**, then confirm all the prompts. GitHub may ask you to type the project name.

## Step 1 — Create your Oracle account

1. Go to **oracle.com/cloud/free** and click **Start for free**.
2. Choose **India** as your country, then fill in your name and email and verify the email.
3. **Home Region: choose `India West (Mumbai)`.** This gives you the lowest ping, and it **cannot be changed later**.
4. Add a debit or credit card. It's for identity checks only, and Always Free resources are never charged. You may see a small temporary hold that gets refunded.
5. Wait for the "account ready" email. It can take anywhere from a few minutes to an hour.

## Step 2 — Prepare the setup script

1. Download **setup.sh** from your Releases page (**github.com/crazyvenom/SquadTalk/releases**, under *latest*).
2. Open it with **Notepad** (right-click → Open with → Notepad).
3. Near the top, change these two lines. Use **letters and numbers only**:
   ```
   SERVER_PASSWORD="friends123"     ← the password your friends will type
   OWNER_CODE="MySecretOwner99"     ← only YOU know this one
   ```
   To have no password at all, leave `SERVER_PASSWORD=""`.
4. Keep Notepad open. You'll copy everything from it in Step 3.

## Step 3 — Create the server

1. Sign in at **cloud.oracle.com**.
2. Click the **☰ menu** (top-left), then **Compute**, then **Instances**, then **Create instance**.
3. **Name:** `squadtalk`
4. **Image and shape**, click **Edit**:
   - **Image:** click **Change image**, choose **Ubuntu**, then **Canonical Ubuntu 24.04**, and click **Select image**.
   - **Shape:** click **Change shape**, then **Virtual machine**, then **Specialty and previous generation**, and pick **VM.Standard.E2.1.Micro**. It's marked *Always Free-eligible*. Click **Select shape**.
5. **Networking:** leave the defaults. It creates a new network, and **Assign a public IPv4 address** is on.
6. **SSH keys:** choose **Generate a key pair for me**, then click **Save private key** and keep the file somewhere safe. You won't need it today, but it lets you get into the server later if something goes wrong.
7. Scroll down and click **Show advanced options**, then open the **Management** tab.
   - Under **Initialization script**, choose **Paste cloud-init script**.
   - In Notepad, press Ctrl+A then Ctrl+C. In the Oracle box, press Ctrl+V.
8. Click **Create**. Wait until the status turns green and says **Running**.
9. On the instance page, find **Public IP address** and copy it. It looks like `140.238.x.x`. **This is your server address.**

## Step 4 — Open port 9987 in Oracle's network

Oracle blocks every port except SSH until you allow it.

1. On the instance page, click the **Subnet** link (or **Networking**, then the subnet name).
2. Open **Security Lists** (or the **Security** tab), then click **Default Security List for …**.
3. Click **Add Ingress Rules** and fill in:
   - Source CIDR: `0.0.0.0/0`
   - IP Protocol: **TCP**
   - Destination Port Range: `9987`
4. Click **+ Another Ingress Rule** and add the same rule again, but with IP Protocol **UDP**.
5. Click **Add Ingress Rules**.

## Step 5 — Become the owner

Wait about 3 minutes after the server shows **Running**. The setup script needs a moment to finish.

1. Open **SquadTalk** on your PC.
2. **Server address:** paste the Public IP from Step 3.
3. **Port:** `9987`
4. **Password:** type your **OWNER_CODE**.
5. Click **Join server**.

You'll see `[owner]` next to your name, and the **Owner tools** appear. Add your rooms now.

The server remembers your PC as the owner from now on. Next time you can use the normal server password, and you'll still be the owner.

## Step 6 — Invite friends

Send each friend:
- **SquadTalk.exe**. They can download it from the Releases page now that the project is public.
- The **server address** (Public IP).
- The **server password**. **Never share the owner code.**

They type all three into SquadTalk and click **Join server**. That's all they need to do.

---

## Keeping it free and running

- **Updates are automatic.** Every night the server checks for a new SquadTalk version and installs it. When Claude improves SquadTalk, download the new **SquadTalk.exe** too, so you and your friends stay on the same version.
- **Idle servers can be reclaimed.** Oracle may reclaim an Always Free server that looks idle for 7 days, which a quiet voice server can. To prevent this:
  1. Go to **☰ → Billing & Cost Management → Upgrade and manage payment** and upgrade to **Pay As You Go**.
  2. You still pay **nothing** while you stay within the Always Free limits, which this server does.
  3. Also set a budget alert: **Billing → Budgets → Create budget**, ₹100, email alert. That way you'd know immediately if anything ever cost money.
- **Your data:** rooms, owners and the password are saved on the server in `/opt/squadtalk/squadtalk-server.json`. They survive restarts.

## Something not working?

| Problem | Fix |
|---|---|
| "Nobody answered at …" | Wait 5 minutes after creating the server. Check both rules in Step 4 (TCP **and** UDP, port 9987). |
| Connected, but nobody can hear each other | The **UDP** rule from Step 4 is missing. |
| "Wrong server password" | You typed something different from `SERVER_PASSWORD` in setup.sh. |
| Not shown as owner | Join once with the **OWNER_CODE** as the password. |
| "Out of capacity" when creating the server | Try again later, or pick a different **Availability domain** under Placement. |

If you're stuck, send Claude a screenshot.
