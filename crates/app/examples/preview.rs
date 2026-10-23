//! Renders the window to PNG files with a real engine fed by the demo transmission, for checking the look without
//! a display: `cargo run --release --example preview -- <out-dir> <stream-second>...`.
//! Set NASDARA_FONT_DIR to a folder with segoeui.ttf, segoeuib.ttf and bahnschrift.ttf to preview with the Windows fonts.

use std::path::PathBuf;
use std::time::Duration;

use egui_kittest::Harness;
use nasdara_bunker_morse::capture::Source;
use nasdara_bunker_morse::engine::{self, Command, Handle};
use nasdara_bunker_morse::theme;
use nasdara_bunker_morse::ui::{self, Settings, UiState};

fn main() {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().unwrap_or_else(|| "preview".into()));
    let marks: Vec<f64> = args.filter_map(|a| a.parse().ok()).collect();
    std::fs::create_dir_all(&out).unwrap();

    let engine = engine::start(Source::Game, egui::Context::default(), None);
    let settings = Settings {
        onboarded: !marks.is_empty(),
        ..Settings::default()
    };
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1060.0, 800.0))
        .with_step_dt(0.016)
        .with_context_setup(theme::install)
        .wgpu()
        .build_ui_state(
            |ui, (engine, st): &mut (Handle, UiState)| ui::draw(ui, engine, st),
            (engine.clone(), UiState::new(settings)),
        );

    if marks.is_empty() {
        harness.run_steps(8);
        save(&mut harness, &out.join("onboarding.png"));
        return;
    }
    engine.send(Command::StartDemo);
    for mark in marks {
        while engine.lock().clock < mark {
            std::thread::sleep(Duration::from_millis(50));
        }
        harness.run_steps(6);
        save(&mut harness, &out.join(format!("t{mark:04.0}.png")));
        eprintln!("wrote t{mark:04.0}.png");
    }
}

fn save(harness: &mut Harness<'_, (Handle, UiState)>, path: &std::path::Path) {
    let img = harness.render().expect("render");
    img.save(path).expect("write png");
}
