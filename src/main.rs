// SquadTalk: lightweight, self-hosted voice chat for gaming.
#![windows_subsystem = "windows"]

pub use squadtalk::{protocol, server};

mod audio;
mod client;
mod config;
mod ui;

fn main() {
    ui::run();
}
