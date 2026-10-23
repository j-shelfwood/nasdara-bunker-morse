use egui::{Align2, Color32, Pos2, Rect, Stroke, Vec2};

use crate::theme;

/// Width of `symbols` drawn at scale 1.
pub fn symbols_width(symbols: &str) -> f32 {
    let dashes = symbols.chars().filter(|&c| c == '-').count() as f32;
    let all = symbols.chars().count() as f32;
    dashes * 12.0 + (all - dashes) * 4.4 + 3.6 * (all - 1.0).max(0.0)
}

/// Morse symbols as small shapes: `.` a dot, `-` a dash.
pub fn paint_symbols(
    painter: &egui::Painter,
    center: Pos2,
    symbols: &str,
    scale: f32,
    color: Color32,
) {
    paint_symbols_with(painter, center, symbols, scale, |_| color);
}

/// Like `paint_symbols`, with a colour chosen per symbol by its position.
pub fn paint_symbols_with(
    painter: &egui::Painter,
    center: Pos2,
    symbols: &str,
    scale: f32,
    color: impl Fn(usize) -> Color32,
) {
    let (dot, dash, gap) = (4.4 * scale, 12.0 * scale, 3.6 * scale);
    let widths: Vec<f32> = symbols
        .chars()
        .map(|c| if c == '-' { dash } else { dot })
        .collect();
    let total = widths.iter().sum::<f32>() + gap * (widths.len().saturating_sub(1)) as f32;
    let mut x = center.x - total / 2.0;
    for (i, w) in widths.into_iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(x, center.y - dot / 2.0), Vec2::new(w, dot));
        painter.rect_filled(r, dot / 2.0, color(i));
        x += w + gap;
    }
}

/// A check mark drawn from two segments, so it does not depend on the font having the glyph.
pub fn paint_check(painter: &egui::Painter, center: Pos2, size: f32, color: Color32, width: f32) {
    let u = size / 2.0;
    painter.add(egui::Shape::line(
        vec![
            center + Vec2::new(-0.55 * u, 0.05 * u),
            center + Vec2::new(-0.15 * u, 0.5 * u),
            center + Vec2::new(0.6 * u, -0.45 * u),
        ],
        Stroke::new(width, color),
    ));
}

/// A bold question mark inside a ring: the "unclear" mark.
pub fn paint_question(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    painter.circle_stroke(center, size / 2.0 - 1.0, Stroke::new(1.8, color));
    painter.text(
        center,
        Align2::CENTER_CENTER,
        "?",
        theme::bold(size * 0.62),
        color,
    );
}
