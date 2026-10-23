//! Measures vertical text centring: draws digits and letters with `Align2::CENTER_CENTER` in fixed cells and writes
//! `/tmp/centering.png`. `/tmp/measure_centering.py` then prints each glyph's ink offset from the cell centre in px
//! (0 is centred; a worst cap/digit offset above 1.0 px means the font tweaks in `theme.rs` drifted).
//!
//! Run with the real fonts: `NASDARA_FONT_DIR=/tmp/fonts cargo run --release --example centering_probe -p nasdara-bunker-morse`

use egui::{Align2, Color32, Pos2, Rect, Vec2};
use egui_kittest::Harness;
use nasdara_bunker_morse::theme;

fn main() {
    let cases: Vec<(&str, egui::FontId, f32)> = vec![
        ("digits30", theme::digits(30.0), 30.0),
        ("digits46", theme::digits(46.0), 46.0),
        ("bold13", theme::bold(13.0), 13.0),
        ("text13", theme::text(13.0), 13.0),
        ("bold20", theme::bold(20.0), 20.0),
    ];
    let glyphs = ["0", "7", "H", "x"];
    let mut harness = Harness::builder()
        .with_size(egui::vec2(480.0, 400.0))
        .with_context_setup(theme::install)
        .wgpu()
        .build_ui(move |ui| {
            let p = ui.painter();
            p.rect_filled(ui.max_rect(), 0.0, Color32::BLACK);
            for (r, (_, font, _)) in cases.iter().enumerate() {
                for (c, g) in glyphs.iter().enumerate() {
                    let cell = Rect::from_min_size(
                        Pos2::new(10.0 + c as f32 * 110.0, 10.0 + r as f32 * 76.0),
                        Vec2::new(100.0, 70.0),
                    );
                    p.text(
                        cell.center(),
                        Align2::CENTER_CENTER,
                        *g,
                        font.clone(),
                        Color32::WHITE,
                    );
                }
            }
        });
    harness.run_steps(4);
    let img = harness.render().expect("render");
    img.save("/tmp/centering.png").unwrap();
}
