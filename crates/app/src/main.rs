#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() -> eframe::Result {
    nasdara_bunker_morse::app::run()
}
