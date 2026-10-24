//! The bunkers heard: a tile for each of the 23 (`roster`), the card of the selected one (`card`), the menu a click
//! on a card opens (`actions`), and the
//! placeholder that stands in the dossier slot while there is nothing to show.

pub(super) mod actions;
mod card;
pub(super) mod copied;
mod reading;
mod roster;

pub use reading::{Reading, reading, reading_color};

use egui::{Pos2, Sense, Stroke, Ui, Vec2};

use crate::theme::{self, DIM, FAINT, RULE, alpha};
pub(in crate::ui) use card::card;
pub(in crate::ui) use roster::show as roster;

/// A dashed box `height` tall with a title and a line of text, both centred on its middle.
pub(super) fn empty(ui: &mut Ui, height: f32, title: &str, text: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    let p = ui.painter();
    p.add(egui::Shape::dashed_line(
        &[
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
            rect.left_top(),
        ],
        Stroke::new(1.0, alpha(RULE, 0.9)),
        5.0,
        5.0,
    ));
    // two centred blocks, the second wrapped to the box, stacked about the box's own middle
    let wrap = rect.width() - 32.0;
    let title = p.layout(title.into(), theme::bold(15.0), DIM, wrap);
    let body = p.layout(text.into(), theme::text(13.0), FAINT, wrap);
    let gap = 8.0;
    let top = rect.center().y - (title.size().y + gap + body.size().y) / 2.0;
    p.galley(
        Pos2::new(rect.center().x - title.size().x / 2.0, top),
        title.clone(),
        DIM,
    );
    p.galley(
        Pos2::new(
            rect.center().x - body.size().x / 2.0,
            top + title.size().y + gap,
        ),
        body,
        FAINT,
    );
}
