//! Connection from the app to a SquadTalk server.

use crate::audio::Mixer;
use crate::protocol::*;
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

pub enum Event {
    Connected,
    Failed(String),
    Denied(String),
    Kicked(String),
    Disconnected,
    Chat { name: String, text: String },
    State,
}

#[derive(Default)]
pub struct Shared {
    pub session: u32,
    pub owner: bool,
    pub rooms: Vec<RoomInfo>,
    pub users: Vec<UserInfo>,
    pub events: VecDeque<Event>,
    pub ping_ms: Option<u32>,
}

pub struct Net {
    pub shared: Mutex<Shared>,
    writer: Mutex<Option<TcpStream>>,
    udp: OnceLock<UdpSocket>,
    session: AtomicU32,
    token: AtomicU32,
    running: AtomicBool,
    pub self_mute: AtomicBool,
    pub deaf: AtomicBool,
    pub self_speaking: AtomicBool,
    /// Microphone loudness in tenths of a dB.
    pub mic_level: AtomicI32,
    notify: Arc<dyn Fn() + Send + Sync>,
}

impl Net {
    pub fn running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn notify(&self) {
        (self.notify)();
    }

    fn push(&self, ev: Event) {
        lk(&self.shared).events.push_back(ev);
        self.notify();
    }

    pub fn send(&self, msg: &C2S) {
        if let Some(w) = lk(&self.writer).as_mut() {
            let _ = w.write_all(to_line(msg).as_bytes());
        }
    }

    fn udp_header(&self, kind: u8, cap: usize) -> Vec<u8> {
        let mut p = Vec::with_capacity(9 + cap);
        p.push(kind);
        p.extend_from_slice(&self.session.load(Ordering::Relaxed).to_le_bytes());
        p.extend_from_slice(&self.token.load(Ordering::Relaxed).to_le_bytes());
        p
    }

    pub fn send_voice(&self, seq: u16, payload: &[u8]) {
        if let Some(u) = self.udp.get() {
            let mut p = self.udp_header(PKT_VOICE, 2 + payload.len());
            p.extend_from_slice(&seq.to_le_bytes());
            p.extend_from_slice(payload);
            let _ = u.send(&p);
        }
    }

    fn send_ping(&self) {
        if let Some(u) = self.udp.get() {
            let mut p = self.udp_header(PKT_PING, 8);
            p.extend_from_slice(&now_ms().to_le_bytes());
            let _ = u.send(&p);
        }
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(w) = lk(&self.writer).take() {
            let _ = w.shutdown(Shutdown::Both);
        }
    }
}

pub fn connect(
    addr: String,
    port: u16,
    name: String,
    secret: String,
    password: String,
    notify: Arc<dyn Fn() + Send + Sync>,
    mixer: Arc<Mutex<Mixer>>,
) -> Arc<Net> {
    let net = Arc::new(Net {
        shared: Mutex::new(Shared::default()),
        writer: Mutex::new(None),
        udp: OnceLock::new(),
        session: AtomicU32::new(0),
        token: AtomicU32::new(0),
        running: AtomicBool::new(true),
        self_mute: AtomicBool::new(false),
        deaf: AtomicBool::new(false),
        self_speaking: AtomicBool::new(false),
        mic_level: AtomicI32::new(-1000),
        notify,
    });
    let n = net.clone();
    thread::spawn(move || run(n, addr, port, name, secret, password, mixer));
    net
}

fn run(
    net: Arc<Net>,
    addr: String,
    port: u16,
    name: String,
    secret: String,
    password: String,
    mixer: Arc<Mutex<Mixer>>,
) {
    let fail = |msg: String| {
        net.running.store(false, Ordering::SeqCst);
        net.push(Event::Failed(msg));
    };

    let sa = match (addr.as_str(), port).to_socket_addrs() {
        Ok(mut it) => match it.next() {
            Some(a) => a,
            None => return fail(format!("Could not find the address \"{addr}\".")),
        },
        Err(_) => return fail(format!("\"{addr}\" is not a valid address. It should look like 100.101.102.103")),
    };
    let stream = match TcpStream::connect_timeout(&sa, Duration::from_secs(6)) {
        Ok(s) => s,
        Err(e) => {
            return fail(format!(
                "Nobody answered at {addr} (port {port}).\n\nCheck that:\n• the host clicked \"Host a server\"\n• Tailscale is turned on for both of you\n• the address is the host's Tailscale IP\n\n(Details: {e})"
            ))
        }
    };
    let _ = stream.set_nodelay(true);
    let mut reader = match stream.try_clone() {
        Ok(s) => BufReader::new(s),
        Err(e) => return fail(e.to_string()),
    };
    match stream.try_clone() {
        Ok(w) => *lk(&net.writer) = Some(w),
        Err(e) => return fail(e.to_string()),
    }
    net.send(&C2S::Hello { version: VERSION, name, secret, password });

    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return fail("The server did not reply. Make sure both of you use the same SquadTalk version.".into());
    }
    match serde_json::from_str::<S2C>(line.trim()) {
        Ok(S2C::Welcome { session, udp_token, owner }) => {
            net.session.store(session, Ordering::SeqCst);
            net.token.store(udp_token, Ordering::SeqCst);
            let mut sh = lk(&net.shared);
            sh.session = session;
            sh.owner = owner;
        }
        Ok(S2C::Denied { reason }) => {
            net.running.store(false, Ordering::SeqCst);
            net.push(Event::Denied(reason));
            return;
        }
        _ => return fail("The server sent something unexpected.".into()),
    }

    let udp = match UdpSocket::bind("0.0.0.0:0").and_then(|u| u.connect(sa).map(|_| u)) {
        Ok(u) => u,
        Err(e) => return fail(format!("Could not open a voice connection: {e}")),
    };
    let _ = udp.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = net.udp.set(udp);
    net.push(Event::Connected);

    {
        let n = net.clone();
        thread::spawn(move || udp_receive(n, mixer));
    }
    {
        let n = net.clone();
        thread::spawn(move || {
            let mut i: u32 = 0;
            while n.running() {
                n.send_ping();
                if i % 5 == 0 {
                    n.send(&C2S::Ping);
                }
                i = i.wrapping_add(1);
                thread::sleep(Duration::from_secs(1));
            }
        });
    }

    // Server answers our pings every 5 s, so 20 s of silence means it's gone.
    let _ = stream.set_read_timeout(Some(Duration::from_secs(20)));
    let mut kicked = false;
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        match serde_json::from_str::<S2C>(line.trim()) {
            Ok(S2C::State { rooms, users }) => {
                {
                    let mut sh = lk(&net.shared);
                    sh.rooms = rooms;
                    sh.users = users;
                }
                net.push(Event::State);
            }
            Ok(S2C::Chat { name, text, .. }) => net.push(Event::Chat { name, text }),
            Ok(S2C::Kicked { reason }) => {
                kicked = true;
                net.running.store(false, Ordering::SeqCst);
                net.push(Event::Kicked(reason));
                break;
            }
            _ => {}
        }
    }
    if net.running() && !kicked {
        net.running.store(false, Ordering::SeqCst);
        net.push(Event::Disconnected);
    }
}

fn udp_receive(net: Arc<Net>, mixer: Arc<Mutex<Mixer>>) {
    let udp = match net.udp.get() {
        Some(u) => u,
        None => return,
    };
    let mut buf = [0u8; 1500];
    let mut pcm = vec![0f32; 5760];
    let mut decoders: HashMap<u32, opus::Decoder> = HashMap::new();
    while net.running() {
        let n = match udp.recv(&mut buf) {
            Ok(n) => n,
            Err(_) => continue,
        };
        if n >= 9 && buf[0] == PKT_PING {
            let mut t = [0u8; 8];
            t.copy_from_slice(&buf[1..9]);
            let sent = u64::from_le_bytes(t);
            let rtt = now_ms().saturating_sub(sent).min(9999) as u32;
            lk(&net.shared).ping_ms = Some(rtt);
        } else if n > 7 && buf[0] == PKT_VOICE {
            if net.deaf.load(Ordering::Relaxed) {
                continue;
            }
            let sender = u32::from_le_bytes([buf[1], buf[2], buf[3], buf[4]]);
            if !decoders.contains_key(&sender) {
                match opus::Decoder::new(48000, opus::Channels::Mono) {
                    Ok(d) => {
                        decoders.insert(sender, d);
                    }
                    Err(_) => continue,
                }
            }
            if let Some(dec) = decoders.get_mut(&sender) {
                if let Ok(samples) = dec.decode_float(&buf[7..n], &mut pcm, false) {
                    lk(&mixer).push(sender, &pcm[..samples]);
                }
            }
        }
    }
}
