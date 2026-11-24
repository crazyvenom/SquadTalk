#!/bin/bash
# =====================================================================
#  SquadTalk server - automatic setup for Oracle Cloud (Ubuntu, x86).
#  Paste this whole file into "Cloud-init script" when creating the
#  server. It installs SquadTalk, opens the firewall, starts it on every
#  boot, and checks for updates every night.
# =====================================================================

# ---- CHANGE THESE TWO LINES (letters and numbers only) --------------
SERVER_PASSWORD=""            # friends type this to join ("" = no password)
OWNER_CODE="ChangeMe12345"    # YOU type this once as the password to become owner
# ---------------------------------------------------------------------

PORT=9987
DOWNLOAD="https://github.com/crazyvenom/SquadTalk/releases/download/latest/squadtalk-server-linux-x64"
DIR=/opt/squadtalk

set -e
exec > >(tee -a /var/log/squadtalk-setup.log) 2>&1
echo "=== SquadTalk setup started $(date -u) ==="

if [ "$(uname -m)" != "x86_64" ]; then
  echo "This setup needs an AMD/Intel (x86_64) server, e.g. VM.Standard.E2.1.Micro."
  exit 1
fi

# Program
id squadtalk >/dev/null 2>&1 || useradd --system --home-dir "$DIR" --shell /usr/sbin/nologin squadtalk
mkdir -p "$DIR"
for i in 1 2 3 4 5; do
  curl -fsSL "$DOWNLOAD" -o "$DIR/squadtalk-server.new" && break
  echo "Download failed, retrying in 10s..."; sleep 10
done
chmod +x "$DIR/squadtalk-server.new"
mv "$DIR/squadtalk-server.new" "$DIR/squadtalk-server"
cd "$DIR"
"$DIR/squadtalk-server" --setup --port "$PORT" --password "$SERVER_PASSWORD" --owner-code "$OWNER_CODE"
chown -R squadtalk:squadtalk "$DIR"
chmod 600 "$DIR/squadtalk-server.json"

# Start automatically, restart if it ever stops
cat > /etc/systemd/system/squadtalk.service <<EOF
[Unit]
Description=SquadTalk voice server
After=network-online.target
Wants=network-online.target

[Service]
User=squadtalk
WorkingDirectory=$DIR
ExecStart=$DIR/squadtalk-server
Restart=always
RestartSec=3
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
EOF

# Nightly update check (04:30 India time)
cat > /usr/local/bin/squadtalk-update <<EOF
#!/bin/bash
TMP=\$(mktemp)
if curl -fsSL "$DOWNLOAD" -o "\$TMP" && ! cmp -s "\$TMP" "$DIR/squadtalk-server"; then
  chmod +x "\$TMP"
  if "\$TMP" --version >/dev/null; then
    mv "\$TMP" "$DIR/squadtalk-server"
    chown squadtalk:squadtalk "$DIR/squadtalk-server"
    systemctl restart squadtalk
    echo "SquadTalk updated \$(date -u)"
    exit 0
  fi
fi
rm -f "\$TMP"
EOF
chmod +x /usr/local/bin/squadtalk-update

cat > /etc/systemd/system/squadtalk-update.service <<EOF
[Unit]
Description=Update SquadTalk server
[Service]
Type=oneshot
ExecStart=/usr/local/bin/squadtalk-update
EOF
cat > /etc/systemd/system/squadtalk-update.timer <<EOF
[Unit]
Description=Nightly SquadTalk update check
[Timer]
OnCalendar=*-*-* 23:00:00 UTC
Persistent=true
[Install]
WantedBy=timers.target
EOF

# Open the port in the server's own firewall (Oracle's Ubuntu blocks it by default)
iptables -I INPUT -p tcp --dport "$PORT" -j ACCEPT
iptables -I INPUT -p udp --dport "$PORT" -j ACCEPT
if command -v netfilter-persistent >/dev/null; then netfilter-persistent save; fi

systemctl daemon-reload
systemctl enable --now squadtalk.service squadtalk-update.timer
sleep 2
systemctl --no-pager status squadtalk.service | head -5
echo "=== SquadTalk setup finished $(date -u) ==="
