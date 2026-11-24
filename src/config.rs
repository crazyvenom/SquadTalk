//! Settings saved to %APPDATA%\SquadTalk\config.json

use crate::protocol::{random_u64, DEFAULT_PORT};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    /// Private identity of this PC. Never shown to anyone.
    pub secret: String,
    pub name: String,
    pub address: String,
    pub port: u16,
    pub password: String,
    pub input_device: String,
    pub output_device: String,
    pub voice_activation: bool,
    /// Windows virtual-key code for push-to-talk (keyboard or mouse button).
    pub ptt_key: i32,
    pub va_threshold_db: i32,
    pub ptt_beeps: bool,
    pub join_sounds: bool,
    /// Per-person volume in percent, keyed by their public ID.
    pub volumes: HashMap<String, u32>,
    /// Rooms of the server you host.
    pub host_rooms: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            secret: String::new(),
            name: String::new(),
            address: String::new(),
            port: DEFAULT_PORT,
            password: String::new(),
            input_device: String::new(),
            output_device: String::new(),
            voice_activation: false,
            ptt_key: 0xC0, // the ` ~ key, left of 1
            va_threshold_db: -40,
            ptt_beeps: true,
            join_sounds: true,
            volumes: HashMap::new(),
            host_rooms: vec!["Lobby".into()],
        }
    }
}

fn path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("SquadTalk").join("config.json")
}

fn write(c: &Config) {
    let p = path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(s) = serde_json::to_string_pretty(c) {
        let _ = std::fs::write(p, s);
    }
}

fn load() -> Config {
    let mut c: Config = std::fs::read_to_string(path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    if c.secret.len() < 16 {
        c.secret = format!("{:016x}{:016x}", random_u64(), random_u64());
        write(&c);
    }
    if c.host_rooms.is_empty() {
        c.host_rooms.push("Lobby".into());
    }
    c
}

static CONFIG: OnceLock<Mutex<Config>> = OnceLock::new();

pub fn cfg() -> MutexGuard<'static, Config> {
    CONFIG
        .get_or_init(|| Mutex::new(load()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

pub fn save() {
    let c = cfg().clone();
    write(&c);
}
