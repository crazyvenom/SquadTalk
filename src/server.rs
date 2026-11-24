//! The SquadTalk server. Runs inside the host's app.
//!
//! Voice packets are relayed as-is (never decoded), so hosting costs almost
//! no CPU even with many people talking.

use crate::protocol::*;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

struct Client {
    session: u32,
    uid: String,
    name: String,
    room: u32,
    mute: bool,
    deaf: bool,
    server_mute: bool,
    owner: bool,
    tx: mpsc::Sender<String>,
    stream: TcpStream,
    udp_token: u32,
    udp_addr: Option<SocketAddr>,
}

struct State {
    rooms: Vec<RoomInfo>,
    next_room: u32,
    clients: BTreeMap<u32, Client>,
    next_session: u32,
    password: String,
    owner_uid: String,
}

impl State {
    fn broadcast_state(&self) {
        let users = self
            .clients
            .values()
            .map(|c| UserInfo {
                session: c.session,
                uid: c.uid.clone(),
                name: c.name.clone(),
                room: c.room,
                mute: c.mute,
                deaf: c.deaf,
                server_mute: c.server_mute,
                owner: c.owner,
            })
            .collect();
        let line = to_line(&S2C::State { rooms: self.rooms.clone(), users });
        for c in self.clients.values() {
            let _ = c.tx.send(line.clone());
        }
    }

    fn save_rooms(&self) {
        let names: Vec<String> = self.rooms.iter().map(|r| r.name.clone()).collect();
        crate::config::cfg().host_rooms = names;
        crate::config::save();
    }

    fn room_exists(&self, id: u32) -> bool {
        self.rooms.iter().any(|r| r.id == id)
    }
}

fn kick_client(c: Client, reason: &str) {
    let line = to_line(&S2C::Kicked { reason: reason.to_string() });
    let _ = (&c.stream).write_all(line.as_bytes());
    let _ = c.stream.shutdown(Shutdown::Both);
}

pub struct ServerHandle {
    running: Arc<AtomicBool>,
    state: Arc<Mutex<State>>,
    port: u16,
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        {
            let mut s = lk(&self.state);
            for c in s.clients.values() {
                let _ = c.stream.shutdown(Shutdown::Both);
            }
            s.clients.clear();
        }
        // Wake the accept loop so it notices we're stopping.
        let _ = TcpStream::connect_timeout(
            &SocketAddr::from(([127, 0, 0, 1], self.port)),
            Duration::from_millis(300),
        );
    }
}

pub fn start(
    port: u16,
    password: String,
    owner_uid: String,
    room_names: Vec<String>,
) -> std::io::Result<ServerHandle> {
    let listener = TcpListener::bind(("0.0.0.0", port))?;
    let udp = UdpSocket::bind(("0.0.0.0", port))?;
    udp.set_read_timeout(Some(Duration::from_millis(500)))?;

    let mut rooms: Vec<RoomInfo> = room_names
        .into_iter()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .enumerate()
        .map(|(i, name)| RoomInfo { id: i as u32, name })
        .collect();
    if rooms.is_empty() {
        rooms.push(RoomInfo { id: 0, name: "Lobby".into() });
    }
    let next_room = rooms.len() as u32;

    let state = Arc::new(Mutex::new(State {
        rooms,
        next_room,
        clients: BTreeMap::new(),
        next_session: 1,
        password,
        owner_uid,
    }));
    let running = Arc::new(AtomicBool::new(true));

    {
        let state = state.clone();
        let running = running.clone();
        thread::spawn(move || {
            for stream in listener.incoming() {
                if !running.load(Ordering::SeqCst) {
                    break;
                }
                if let Ok(s) = stream {
                    let state = state.clone();
                    thread::spawn(move || handle_conn(s, state));
                }
            }
        });
    }
    {
        let state = state.clone();
        let running = running.clone();
        thread::spawn(move || udp_loop(udp, state, running));
    }

    Ok(ServerHandle { running, state, port })
}

fn handle_conn(stream: TcpStream, st: Arc<Mutex<State>>) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let mut reader = match stream.try_clone() {
        Ok(s) => BufReader::new(s),
        Err(_) => return,
    };
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return;
    }
    let (version, name, secret, password) = match serde_json::from_str::<C2S>(line.trim()) {
        Ok(C2S::Hello { version, name, secret, password }) => (version, name, secret, password),
        _ => return,
    };
    let deny = |reason: &str| {
        let _ = (&stream).write_all(to_line(&S2C::Denied { reason: reason.into() }).as_bytes());
    };
    if version != VERSION {
        deny("Your SquadTalk version is different from the host's. Please both use the same latest version.");
        return;
    }
    if secret.len() < 16 {
        return;
    }
    let uid = uid_of(&secret);
    let mut name: String = name.trim().chars().take(24).collect();
    if name.is_empty() {
        name = "Player".into();
    }

    let session;
    {
        let mut s = lk(&st);
        let owner = uid == s.owner_uid;
        if !owner && !s.password.is_empty() && password != s.password {
            deny("Wrong server password.");
            return;
        }
        // Same person connecting twice: drop the old connection.
        let dups: Vec<u32> = s.clients.values().filter(|c| c.uid == uid).map(|c| c.session).collect();
        for d in dups {
            if let Some(c) = s.clients.remove(&d) {
                kick_client(c, "You connected again from another window or PC.");
            }
        }

        session = s.next_session;
        s.next_session += 1;
        let udp_token = (random_u64() & 0xffff_ffff) as u32;

        let (tx, rx) = mpsc::channel::<String>();
        let mut writer = match stream.try_clone() {
            Ok(w) => w,
            Err(_) => return,
        };
        let shutdown_handle = match stream.try_clone() {
            Ok(w) => w,
            Err(_) => return,
        };
        thread::spawn(move || {
            for msg in rx {
                if writer.write_all(msg.as_bytes()).is_err() {
                    break;
                }
            }
        });
        let _ = tx.send(to_line(&S2C::Welcome { session, udp_token, owner }));
        let lobby = s.rooms.first().map(|r| r.id).unwrap_or(0);
        s.clients.insert(
            session,
            Client {
                session,
                uid,
                name,
                room: lobby,
                mute: false,
                deaf: false,
                server_mute: false,
                owner,
                tx,
                stream: shutdown_handle,
                udp_token,
                udp_addr: None,
            },
        );
        s.broadcast_state();
    }

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let msg = match serde_json::from_str::<C2S>(line.trim()) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mut s = lk(&st);
        if !s.clients.contains_key(&session) {
            break; // kicked
        }
        handle_msg(&mut s, session, msg);
    }

    let mut s = lk(&st);
    if s.clients.remove(&session).is_some() {
        s.broadcast_state();
    }
}

fn handle_msg(s: &mut State, me: u32, msg: C2S) {
    let owner = s.clients.get(&me).map(|c| c.owner).unwrap_or(false);
    match msg {
        C2S::Hello { .. } => {}
        C2S::Ping => {
            if let Some(c) = s.clients.get(&me) {
                let _ = c.tx.send(to_line(&S2C::Pong));
            }
        }
        C2S::Join { room } => {
            if s.room_exists(room) {
                if let Some(c) = s.clients.get_mut(&me) {
                    c.room = room;
                }
                s.broadcast_state();
            }
        }
        C2S::Chat { text } => {
            let text: String = text.trim().chars().take(500).collect();
            if text.is_empty() {
                return;
            }
            let (room, name) = match s.clients.get(&me) {
                Some(c) => (c.room, c.name.clone()),
                None => return,
            };
            let line = to_line(&S2C::Chat { from: me, name, text });
            for c in s.clients.values().filter(|c| c.room == room) {
                let _ = c.tx.send(line.clone());
            }
        }
        C2S::SetState { mute, deaf } => {
            if let Some(c) = s.clients.get_mut(&me) {
                c.mute = mute;
                c.deaf = deaf;
            }
            s.broadcast_state();
        }
        C2S::Kick { session } if owner => {
            if session != me {
                if let Some(c) = s.clients.remove(&session) {
                    kick_client(c, "You were kicked by the server owner.");
                    s.broadcast_state();
                }
            }
        }
        C2S::ServerMute { session, muted } if owner => {
            if let Some(c) = s.clients.get_mut(&session) {
                c.server_mute = muted;
            }
            s.broadcast_state();
        }
        C2S::Move { session, room } if owner => {
            if s.room_exists(room) {
                if let Some(c) = s.clients.get_mut(&session) {
                    c.room = room;
                }
                s.broadcast_state();
            }
        }
        C2S::AddRoom { name } if owner => {
            let name: String = name.trim().chars().take(40).collect();
            if name.is_empty() || s.rooms.len() >= 50 || s.rooms.iter().any(|r| r.name == name) {
                return;
            }
            let id = s.next_room;
            s.next_room += 1;
            s.rooms.push(RoomInfo { id, name });
            s.save_rooms();
            s.broadcast_state();
        }
        C2S::DeleteRoom { room } if owner => {
            let lobby = s.rooms.first().map(|r| r.id).unwrap_or(0);
            if room == lobby || !s.room_exists(room) {
                return;
            }
            s.rooms.retain(|r| r.id != room);
            for c in s.clients.values_mut() {
                if c.room == room {
                    c.room = lobby;
                }
            }
            s.save_rooms();
            s.broadcast_state();
        }
        _ => {}
    }
}

fn udp_loop(sock: UdpSocket, st: Arc<Mutex<State>>, running: Arc<AtomicBool>) {
    let mut buf = [0u8; 1500];
    let mut out: Vec<u8> = Vec::with_capacity(1500);
    let mut targets: Vec<SocketAddr> = Vec::with_capacity(64);
    while running.load(Ordering::SeqCst) {
        let (n, from) = match sock.recv_from(&mut buf) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if n < 9 {
            continue;
        }
        let kind = buf[0];
        let session = u32::from_le_bytes([buf[1], buf[2], buf[3], buf[4]]);
        let token = u32::from_le_bytes([buf[5], buf[6], buf[7], buf[8]]);
        targets.clear();
        out.clear();
        {
            let mut s = lk(&st);
            let (room, blocked) = match s.clients.get_mut(&session) {
                Some(c) if c.udp_token == token => {
                    c.udp_addr = Some(from);
                    (c.room, c.mute || c.deaf || c.server_mute)
                }
                _ => continue,
            };
            if kind == PKT_PING {
                out.push(PKT_PING);
                out.extend_from_slice(&buf[9..n]);
                targets.push(from);
            } else if kind == PKT_VOICE && !blocked && n > 11 {
                out.push(PKT_VOICE);
                out.extend_from_slice(&session.to_le_bytes());
                out.extend_from_slice(&buf[9..n]); // seq + opus data
                for c in s.clients.values() {
                    if c.session != session && c.room == room && !c.deaf {
                        if let Some(a) = c.udp_addr {
                            targets.push(a);
                        }
                    }
                }
            }
        }
        for t in &targets {
            let _ = sock.send_to(&out, t);
        }
    }
}
