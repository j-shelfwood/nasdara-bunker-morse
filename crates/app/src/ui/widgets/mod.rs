use egui::{Sense, Stroke, Ui};

use crate::theme::{self, DIM, STROKE, SURFACE};

mod buttons;
mod focus;
mod symbols;
mod toggle;

pub use buttons::{ButtonKind, Pill, button, icon_button, pill, segmented};
pub use focus::{focus_ring, paint_caption};
pub use symbols::{paint_check, paint_question, paint_symbols, paint_symbols_with, symbols_width};
pub use toggle::toggle;

pub fn card(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, STROKE))
        .corner_radius(14.0)
        .inner_margin(egui::Margin::symmetric(20, 16))
        .show(ui, add);
}

/// Small uppercase caption in the dim colour, with the letter spacing a label needs.
pub fn caption(ui: &mut Ui, text: &str) {
    let galley = ui
        .painter()
        .layout_no_wrap(spaced(text), theme::bold(11.0), DIM);
    let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter().galley(rect.min, galley, DIM);
}

pub fn spaced(text: &str) -> String {
    let mut out = String::new();
    for (i, c) in text.to_uppercase().chars().enumerate() {
        if i > 0 {
            out.push('\u{200a}');
        }
        out.push(c);
    }
    out
}
