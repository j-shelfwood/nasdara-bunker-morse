use egui::{Align2, Painter, Pos2, Rect, Stroke};

use super::projection::Projection;
use super::{built_up, coast};
use crate::engine::Shared;
use crate::theme::{self, AMBER, DIM, FAINT, RULE, STROKE, alpha};
use crate::ui::UiState;
use crate::ui::bunkers::{Reading, reading, reading_color};
use crate::ui::widgets::paint_check;
use nasdara_decoder::bunkers::bunkers;

/// The 16 x 16 graticule and the north mark.
/// The 16 x 16 graticule and the north mark.
pub(super) fn grid(p: &Painter, rect: Rect, inner: Rect) {
    for k in 1..16 {
        let f = k as f32 / 16.0;
        let c = alpha(STROKE, if k % 4 == 0 { 0.9 } else { 0.4 });
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
        Pos2::new(rect.center().x, rect.top() + 7.0),
        Align2::CENTER_CENTER,
        "N",
        theme::bold(11.0),
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

/// One pin per bunker; returns the hover hint of the pin under the pointer.
pub(super) fn pins(
    p: &Painter,
    proj: &Projection,
    st: &UiState,
    s: &Shared,
    hovered: Option<Pos2>,
    still: bool,
    time: f32,
) -> Option<String> {
    let mut hint = None;
    for b in bunkers() {
        let pos = proj.to_screen(b.x, b.z);
        let card = s.cards.iter().find(|c| c.bunker == Some(b.index));
        match card {
            None => {
                p.circle_filled(pos, 2.6, RULE);
            }
            Some(c) => {
                let sel = st.selected.as_deref() == Some(&c.key);
                let phase = (time * 0.9).fract();
                if sel || (!still && c.updated.elapsed().as_secs_f32() < 4.0) {
                    p.circle_stroke(
                        pos,
                        5.0 + 12.0 * phase,
                        Stroke::new(1.2, alpha(AMBER, 0.6 * (1.0 - phase))),
                    );
                }
                let r = reading(c);
                let color = reading_color(r);
                p.circle_filled(pos, 6.5, alpha(color, 0.18));
                match r {
                    // unclear: an open ring with a question mark, not a different shade
                    Reading::Unclear => {
                        p.circle_filled(pos, 7.0, theme::BG);
                        p.circle_stroke(pos, 7.0, Stroke::new(1.8, color));
                        p.text(pos, Align2::CENTER_CENTER, "?", theme::bold(11.0), color);
                    }
                    Reading::Once => {
                        p.circle_filled(pos, 4.2, color);
                    }
                    Reading::Confirmed => {
                        p.circle_filled(pos, 5.2, color);
                        paint_check(p, pos, 5.0, theme::BG, 1.4);
                    }
                }
            }
        }
        if hovered.is_some_and(|h| h.distance(pos) < 8.0) {
            hint = Some(format!(
                "Bunker {} · grid {} {}",
                b.index,
                &b.grid[..3],
                &b.grid[3..]
            ));
        }
    }
    hint
}
