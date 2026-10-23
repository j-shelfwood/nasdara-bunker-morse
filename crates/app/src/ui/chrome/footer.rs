//! The bottom line: audio level meter and version.

use egui::{Align, Layout, Rect, RichText, Sense, Stroke, Ui, Vec2};

use crate::engine::Shared;
use crate::theme::{self, DIM, FAINT, RULE, STROKE, SURFACE, TEAL};

pub fn show(ui: &mut Ui, s: &Shared) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(120.0, 8.0), Sense::hover());
        ui.painter().rect_filled(rect, 4.0, SURFACE);
        ui.painter().rect_stroke(
            rect,
            4.0,
            Stroke::new(1.0, STROKE),
            egui::StrokeKind::Inside,
        );
        let level = (s.level.sqrt()).clamp(0.0, 1.0);
        ui.painter().rect_filled(
            Rect::from_min_size(rect.min, Vec2::new(rect.width() * level, rect.height())),
            4.0,
            if level > 0.02 { TEAL } else { RULE },
        );
        ui.label(
            RichText::new("Audio level")
                .font(theme::text(12.0))
                .color(DIM),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                    .font(theme::text(12.0))
                    .color(FAINT),
            );
        });
    });
}
