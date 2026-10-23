use egui::{Align2, Color32, Pos2, Response, Stroke, StrokeKind};

use super::spaced;
use crate::theme::{self, TEXT};

pub fn paint_caption(
    painter: &egui::Painter,
    pos: Pos2,
    align: Align2,
    text: &str,
    color: Color32,
) {
    painter.text(pos, align, spaced(text), theme::bold(11.0), color);
}

/// A 2 px ring around a widget that has keyboard focus.
pub fn focus_ring(painter: &egui::Painter, response: &Response, radius: f32) {
    if response.has_focus() {
        painter.rect_stroke(
            response.rect.expand(1.0),
            radius + 1.0,
            Stroke::new(2.0, TEXT),
            StrokeKind::Outside,
        );
    }
}
