//! The 23 bunkers on a square plan of the map, drawn inside a rect the caller allocates.

mod built_up;
mod coast;
mod paint;
mod projection;

use egui::{Rect, Sense, Ui};

use super::UiState;
use crate::engine::Shared;
use crate::theme;
use projection::Projection;

/// Bunkers that were heard glow; the selected card's bunker pulses.
pub fn show(ui: &mut Ui, st: &UiState, s: &Shared, rect: Rect) {
    let response = ui.interact(rect, ui.make_persistent_id("map"), Sense::hover());
    let p = ui.painter_at(rect);
    let still = st.settings.reduce_motion;
    let time = if still {
        0.0
    } else {
        ui.input(|i| i.time) as f32
    };
    p.rect_filled(rect, 12.0, theme::BG);
    p.rect_stroke(
        rect,
        12.0,
        egui::Stroke::new(1.0, theme::STROKE),
        egui::StrokeKind::Inside,
    );
    let inner = rect.shrink(12.0);
    paint::grid(&p, rect, inner);
    let proj = Projection::new(inner);
    paint::outlines(&p, &proj);
    let hint = paint::pins(&p, &proj, st, s, response.hover_pos(), still, time);
    if let Some(h) = hint {
        response.on_hover_text(h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nasdara_decoder::bunkers::bunkers;

    fn inside(loop_: &[(f32, f32)], x: f32, z: f32) -> bool {
        let mut hit = false;
        for (i, &(ax, az)) in loop_.iter().enumerate() {
            let (bx, bz) = loop_[(i + 1) % loop_.len()];
            if (az > z) != (bz > z) && x < ax + (z - az) / (bz - az) * (bx - ax) {
                hit = !hit;
            }
        }
        hit
    }

    #[test]
    fn every_bunker_lies_on_the_outlined_land() {
        for b in bunkers() {
            let land = coast::COAST
                .iter()
                .any(|l| inside(l, b.x as f32, b.z as f32));
            assert!(land, "bunker {} is outside the island outline", b.index);
        }
    }
}
