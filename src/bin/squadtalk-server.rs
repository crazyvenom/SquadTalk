//! SquadTalk standalone server, for a machine that runs 24/7 (e.g. Oracle Cloud).
//!
//! Usage:
//!   squadtalk-server [--config FILE] [--port N] [--password TEXT] [--owner-code TEXT] [--setup]
//!
//! --setup saves the given values into the config file and exits.
//!
//! Settings live in the config file (default: squadtalk-server.json in the
//! current folder). Values given on the command line are saved into it.

use serde::{Deserialize, Serialize};
use squadtalk::protocol::{lk, random_u64, DEFAULT_PORT, VERSION};
use squadtalk::server::{self, Options, Saved};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct ServerConfig {
    port: u16,
    /// Everyone needs this to join. Empty = no password.
    password: String,
    /// Type this as the password once to become an owner.
    owner_code: String,
    /// Public IDs of owners (filled in automatically).
    owner_ids: Vec<String>,
    rooms: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            port: DEFAULT_PORT,
            password: String::new(),
            owner_code: String::new(),
            owner_ids: Vec::new(),
            rooms: vec!["Lobby".into()],
        }
    }
}

fn random_code() -> String {
    const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut n = random_u64();
    (0..10)
        .map(|_| {
            let c = CHARS[(n % CHARS.len() as u64) as usize] as char;
            n /= CHARS.len() as u64;
            c
        })
        .collect()
}

fn write(path: &PathBuf, c: &ServerConfig) {
    match serde_json::to_string_pretty(c) {
        Ok(s) => {
            if let Err(e) = std::fs::write(path, s) {
                eprintln!("Could not save {}: {e}", path.display());
            }
        }
        Err(e) => eprintln!("Could not save settings: {e}"),
    }
}

fn main() {
    let mut path = PathBuf::from("squadtalk-server.json");
    let mut port: Option<u16> = None;
    let mut password: Option<String> = None;
    let mut owner_code: Option<String> = None;
    let mut setup_only = false;

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--config" => path = args.next().map(PathBuf::from).unwrap_or(path),
            "--port" => port = args.next().and_then(|v| v.parse().ok()),
            "--password" => password = args.next(),
            "--owner-code" => owner_code = args.next(),
            "--setup" => setup_only = true,
            "--version" => {
                println!("squadtalk-server {} (protocol {VERSION})", env!("CARGO_PKG_VERSION"));
                return;
            }
            "-h" | "--help" => {
                println!("squadtalk-server [--config FILE] [--port N] [--password TEXT] [--owner-code TEXT] [--setup]");
                return;
            }
            other => {
                eprintln!("Unknown option: {other}  (try --help)");
                std::process::exit(2);
            }
        }
    }

    let mut cfg: ServerConfig = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    if let Some(p) = port {
        cfg.port = p;
    }
    if let Some(p) = password {
        cfg.password = p;
    }
    if let Some(c) = owner_code {
        cfg.owner_code = c;
    }
    if cfg.owner_code.trim().is_empty() {
        cfg.owner_code = random_code();
    }
    if cfg.rooms.is_empty() {
        cfg.rooms.push("Lobby".into());
    }
    write(&path, &cfg);
    if setup_only {
        println!("Settings saved to {}.", path.display());
        return;
    }

    let shared = Arc::new(Mutex::new(cfg.clone()));
    let on_save = {
        let shared = shared.clone();
        let path = path.clone();
        Arc::new(move |saved: Saved| {
            let mut c = lk(&shared);
            c.rooms = saved.rooms;
            c.owner_ids = saved.owner_uids;
            write(&path, &c);
        })
    };

    let opts = Options {
        port: cfg.port,
        password: cfg.password.clone(),
        owner_code: cfg.owner_code.clone(),
        owner_uids: cfg.owner_ids.clone(),
        rooms: cfg.rooms.clone(),
        on_save,
        log: true,
    };
    let _server = match server::start(opts) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Could not start on port {}: {e}", cfg.port);
            std::process::exit(1);
        }
    };

    println!("SquadTalk server {} is running on port {} (TCP + UDP).", env!("CARGO_PKG_VERSION"), cfg.port);
    println!("Settings file: {}", path.display());
    if cfg.password.is_empty() {
        println!("Server password: (none)");
    } else {
        println!("Server password: set");
    }
    println!("Owner code: {}  (type it as the password once to become owner)", cfg.owner_code);
    println!("Owners so far: {}", cfg.owner_ids.len());

    loop {
        std::thread::park();
    }
}
