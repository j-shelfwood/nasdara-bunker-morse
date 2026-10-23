use egui::{Align2, Painter, Pos2, Rect, Stroke, vec2};

use super::projection::Projection;
use super::{built_up, coast};
use crate::theme::{self, AMBER, BG, BRACKET, CUT, DIM, FAINT, HAIRLINE, STROKE, alpha};
use crate::ui::hud::{paint_brackets, paint_frame};

/// Margin between the frame and the plan, in points at scale `k`.
pub(super) fn margin(k: f32) -> f32 {
    16.0 * k
}

/// The chamfered outer frame, its corner brackets and the scale ticks along the edges.
pub(super) fn frame(p: &Painter, rect: Rect, k: f32) {
    let r = rect.shrink(1.0);
    paint_frame(p, r, BG, Stroke::new(HAIRLINE, STROKE), CUT * k.max(1.0));
    paint_brackets(
        p,
        rect.shrink(2.5),
        BRACKET * k,
        Stroke::new(1.5, alpha(AMBER, 0.75)),
    );
    let inner = rect.shrink(margin(k));
    let tick = Stroke::new(HAIRLINE, alpha(DIM, 0.45));
    for i in 1..16 {
        let f = i as f32 / 16.0;
        let len = if i % 4 == 0 { 6.0 * k } else { 3.0 * k };
        let (x, y) = (
            inner.left() + f * inner.width(),
            inner.top() + f * inner.height(),
        );
        let north_gap = (x - rect.center().x).abs() < 12.0 * k;
        if !north_gap {
            p.line_segment(
                [
                    Pos2::new(x, r.top() + 1.0),
                    Pos2::new(x, r.top() + 1.0 + len),
                ],
                tick,
            );
        }
        p.line_segment(
            [
                Pos2::new(x, r.bottom() - 1.0),
                Pos2::new(x, r.bottom() - 1.0 - len),
            ],
            tick,
        );
        for (a, b) in [
            (Pos2::new(r.left() + 1.0, y), vec2(len, 0.0)),
            (Pos2::new(r.right() - 1.0, y), vec2(-len, 0.0)),
        ] {
            p.line_segment([a, a + b], tick);
        }
    }
}

/// The 16 x 16 graticule and the north mark.
pub(super) fn grid(p: &Painter, rect: Rect, inner: Rect, k: f32) {
    for i in 1..16 {
        let f = i as f32 / 16.0;
        let c = alpha(STROKE, if i % 4 == 0 { 0.9 } else { 0.4 });
        p.line_segment(
            [
                Pos2::new(inner.left() + f * inner.width(), inner.top()),
                Pos2::new(inner.left() + f * inner.width(), inner.bottom()),
            ],
            Stroke::new(1.0, c),
        );
        p.line_segment(
            [
                Pos2::new(inner.left(), inner.top() + f * inner.height()),
                Pos2::new(inner.right(), inner.top() + f * inner.height()),
            ],
            Stroke::new(1.0, c),
        );
    }
    p.text(
        Pos2::new(rect.center().x, rect.top() + 8.0 * k),
        Align2::CENTER_CENTER,
        "N",
        theme::bold(11.0 * k),
        FAINT,
    );
}

/// The built-up areas and the coast.
pub(super) fn outlines(p: &Painter, proj: &Projection) {
    let outline = |loops: &[&[(f32, f32)]], stroke: Stroke| {
        for l in loops {
            let points = l
                .iter()
                .map(|&(x, z)| proj.to_screen(f64::from(x), f64::from(z)))
                .collect();
            p.add(egui::Shape::closed_line(points, stroke));
        }
    };
    outline(built_up::BUILT_UP, Stroke::new(1.0, alpha(FAINT, 0.3)));
    outline(coast::COAST, Stroke::new(1.4, alpha(DIM, 0.65)));
}
