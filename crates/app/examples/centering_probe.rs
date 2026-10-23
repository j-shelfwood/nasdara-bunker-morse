//! Measures vertical text centring. Draws digits and letters with `Align2::CENTER_CENTER` in fixed cells, finds the ink
//! of each glyph in the rendered image and prints its offset from the cell centre in px (negative is too high).
//! Exits with an error when a digit or capital is more than 1.0 px off, which means the font tweaks in `theme.rs`
//! drifted from the fonts. Lowercase glyphs are printed but not judged: they centre on the x-height, not the cap height.
//!
//! Run with the real fonts: `NASDARA_FONT_DIR=/tmp/fonts cargo run --release --example centering_probe -p nasdara-bunker-morse`

use egui::{Align2, Color32, FontId, Pos2, Rect, Vec2};
use egui_kittest::Harness;
use nasdara_bunker_morse::theme;

const CELL: Vec2 = Vec2::new(100.0, 70.0);
const GLYPHS: [&str; 4] = ["0", "7", "H", "x"];
const TOLERANCE_PX: f32 = 1.0;

fn main() {
    let cases: Vec<(&str, FontId)> = vec![
        ("digits30", theme::digits(30.0)),
        ("digits46", theme::digits(46.0)),
        ("bold13", theme::bold(13.0)),
        ("text13", theme::text(13.0)),
        ("bold20", theme::bold(20.0)),
    ];
    let cell_at = |row: usize, col: usize| {
        Rect::from_min_size(
            Pos2::new(10.0 + col as f32 * 110.0, 10.0 + row as f32 * 76.0),
            CELL,
        )
    };
    let size = egui::vec2(480.0, 400.0);
    let fonts: Vec<FontId> = cases.iter().map(|(_, f)| f.clone()).collect();
    let mut harness = Harness::builder()
        .with_size(size)
        .with_context_setup(theme::install)
        .wgpu()
        .build_ui(move |ui| {
            let p = ui.painter();
            p.rect_filled(ui.max_rect(), 0.0, Color32::BLACK);
            for (row, font) in fonts.iter().enumerate() {
                for (col, glyph) in GLYPHS.iter().enumerate() {
                    p.text(
                        cell_at(row, col).center(),
                        Align2::CENTER_CENTER,
                        *glyph,
                        font.clone(),
                        Color32::WHITE,
                    );
                }
            }
        });
    harness.run_steps(4);
    let img = harness.render().expect("render");
    let scale = img.width() as f32 / size.x;

    let mut worst = 0.0_f32;
    for (row, (name, _)) in cases.iter().enumerate() {
        let mut line = format!("{name:9}");
        for (col, glyph) in GLYPHS.iter().enumerate() {
            let cell = cell_at(row, col);
            let (mut top, mut bottom) = (f32::MAX, f32::MIN);
            for y in (cell.top() * scale) as u32..(cell.bottom() * scale) as u32 {
                for x in (cell.left() * scale) as u32..(cell.right() * scale) as u32 {
                    if img.get_pixel(x, y).0[0] > 60 {
                        top = top.min(y as f32);
                        bottom = bottom.max(y as f32);
                    }
                }
            }
            let offset = ((top + bottom) / 2.0 - cell.center().y * scale) / scale;
            if *glyph != "x" {
                worst = worst.max(offset.abs());
            }
            line.push_str(&format!(" {glyph}:{offset:+.1}"));
        }
        println!("{line}");
    }
    println!("worst digit/capital offset: {worst:.1} px (limit {TOLERANCE_PX:.1})");
    if worst > TOLERANCE_PX {
        std::process::exit(1);
    }
}
