// SquadTalk: lightweight, self-hosted voice chat for gaming.
#![windows_subsystem = "windows"]

mod audio;
mod client;
mod config;
mod protocol;
mod server;
mod ui;

fn main() {
    ui::run();
}
