//! Messages exchanged between the SquadTalk app and the server.
//!
//! Control messages (join room, chat, admin actions) travel over TCP as one
//! JSON object per line. Voice travels over UDP as small binary packets:
//!
//!   client -> server  voice: [1][session u32][token u32][seq u16][opus data]
//!   server -> client  voice: [1][sender session u32][seq u16][opus data]
//!   client -> server  ping : [2][session u32][token u32][time u64]
//!   server -> client  ping : [2][time u64]

use serde::{Deserialize, Serialize};
use std::sync::{Mutex, MutexGuard};

pub const VERSION: u32 = 1;
pub const DEFAULT_PORT: u16 = 9987;
pub const PKT_VOICE: u8 = 1;
pub const PKT_PING: u8 = 2;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "t")]
pub enum C2S {
    Hello { version: u32, name: String, secret: String, password: String },
    Ping,
    Join { room: u32 },
    Chat { text: String },
    SetState { mute: bool, deaf: bool },
    Kick { session: u32 },
    ServerMute { session: u32, muted: bool },
    Move { session: u32, room: u32 },
    AddRoom { name: String },
    DeleteRoom { room: u32 },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "t")]
pub enum S2C {
    Welcome { session: u32, udp_token: u32, owner: bool },
    Denied { reason: String },
    State { rooms: Vec<RoomInfo>, users: Vec<UserInfo> },
    Chat { from: u32, name: String, text: String },
    Kicked { reason: String },
    Pong,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RoomInfo {
    pub id: u32,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct UserInfo {
    pub session: u32,
    pub uid: String,
    pub name: String,
    pub room: u32,
    pub mute: bool,
    pub deaf: bool,
    pub server_mute: bool,
    pub owner: bool,
}

pub fn to_line<T: Serialize>(msg: &T) -> String {
    let mut s = serde_json::to_string(msg).unwrap_or_default();
    s.push('\n');
    s
}

/// Public ID derived from a person's private secret. Lets everyone remember
/// settings per person without ever sharing the secret itself.
pub fn uid_of(secret: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in secret.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

pub fn random_u64() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    );
    h.finish()
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Lock a mutex, recovering if another thread panicked while holding it.
pub fn lk<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}
