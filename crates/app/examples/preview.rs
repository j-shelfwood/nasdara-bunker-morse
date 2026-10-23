//! Renders the window to PNG files with a real engine fed by the demo transmission, for checking the look without
//! a display: `cargo run --release --example preview -- <out-dir> [--size WxH] <stream-second>...`.
//! `--size 760x520` renders at another window size in points (default 1060x800); file names then carry the size.
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
    let mut size = (1060.0_f32, 800.0_f32);
    let mut marks: Vec<f64> = Vec::new();
    // `--result clear|unclear`: a synthetic finished transmission and a few cards (stand-ins, not decoded audio)
    let mut result: Option<String> = None;
    while let Some(a) = args.next() {
        if a == "--size" {
            let parsed = args.next().and_then(|v| {
                let (w, h) = v.split_once('x')?;
                Some((w.parse().ok()?, h.parse().ok()?))
            });
            size = parsed.expect("--size needs WxH, e.g. 760x520");
        } else if a == "--result" {
            result = args.next();
        } else if let Ok(m) = a.parse() {
            marks.push(m);
        }
    }
    let suffix = if size == (1060.0, 800.0) {
        String::new()
    } else {
        format!("-{}x{}", size.0, size.1)
    };
    std::fs::create_dir_all(&out).unwrap();

    let engine = engine::start(Source::Game, egui::Context::default(), None);
    let settings = Settings {
        onboarded: !marks.is_empty() || result.is_some(),
        ..Settings::default()
    };
    let mut harness = Harness::builder()
        .with_size(egui::vec2(size.0, size.1))
        .with_step_dt(0.016)
        .with_context_setup(theme::install)
        .wgpu()
        .build_ui_state(
            |ui, (engine, st): &mut (Handle, UiState)| ui::draw(ui, engine, st),
            (engine.clone(), UiState::new(settings)),
        );

    if let Some(kind) = result {
        scenario(&engine, kind == "unclear");
        harness.run_steps(3);
        harness.run_steps(6);
        save(
            &mut harness,
            &out.join(format!("result-{kind}{suffix}.png")),
        );
        eprintln!("wrote result-{kind}{suffix}.png");
        return;
    }
    if marks.is_empty() {
        for page in 0..3 {
            harness.state_mut().1.onboarding = Some(page);
            harness.run_steps(8);
            save(
                &mut harness,
                &out.join(format!("onboarding-{page}{suffix}.png")),
            );
        }
        harness.state_mut().1.onboarding = None;
        harness.run_steps(8);
        save(&mut harness, &out.join(format!("idle{suffix}.png")));
        return;
    }
    engine.send(Command::StartDemo);
    for mark in marks {
        while engine.lock().clock < mark {
            std::thread::sleep(Duration::from_millis(50));
        }
        harness.run_steps(6);
        save(&mut harness, &out.join(format!("t{mark:04.0}{suffix}.png")));
        eprintln!("wrote t{mark:04.0}{suffix}.png");
    }
}

fn scenario(engine: &Handle, unclear: bool) {
    use nasdara_bunker_morse::capture::Status;
    use nasdara_bunker_morse::engine::{Card, Finished};
    use nasdara_decoder::bunkers::{bunkers, izurvive_url};
    use std::time::Instant;

    let card = |n: usize, code: &str, count: usize, unclear: usize, corrected: bool| {
        let b = bunkers().iter().find(|b| b.index == n).expect("bunker");
        Card {
            key: format!("b{n}"),
            bunker: Some(n),
            grid: b.grid.clone(),
            code: code.into(),
            count,
            unclear,
            corrected,
            position: Some((b.x, b.z)),
            url: Some(izurvive_url(b.x, b.z, 7)),
            updated: Instant::now() - Duration::from_secs(30 - n as u64),
        }
    };
    let b17 = bunkers().iter().find(|b| b.index == 17).expect("bunker");
    let code = if unclear { "9?3?" } else { "9136" };
    let digits: Vec<char> = format!("{}{code}", b17.grid).chars().collect();
    let support: Vec<f32> = (0..10)
        .map(|i| [0.9, 0.8, 0.95, 0.6, 0.5, 0.9, 0.9, 0.3, 0.85, 0.2][i])
        .collect();
    let mut s = engine.lock();
    s.status = Status::Listening;
    s.cards = vec![
        card(1, "4821", 3, 0, false),
        card(10, "0075", 1, 0, false),
        card(5, "7??2", 2, 2, false),
        card(17, code, 1, usize::from(unclear) * 2, false),
        card(8, "5512", 2, 0, true),
    ];
    s.finished = Some(Finished {
        digits,
        support,
        starts: (0..10).map(|i| i as f64 * 6.0).collect(),
        heard: Instant::now() - Duration::from_secs(10),
        bunker: Some(17),
    });
    s.burst_beeps = 0;
}

fn save(harness: &mut Harness<'_, (Handle, UiState)>, path: &std::path::Path) {
    let img = harness.render().expect("render");
    img.save(path).expect("write png");
}
