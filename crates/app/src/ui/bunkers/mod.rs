//! One card per bunker heard, and a map of where they are.

mod card;
pub(super) mod copied;
mod reading;

pub use reading::{Reading, reading, reading_color};

use egui::{Pos2, Sense, Stroke, Ui, Vec2};

use super::UiState;
use crate::engine::{Card, Shared};
use crate::theme::{self, DIM, FAINT, RULE, alpha};
use card::card;

pub(super) const CARD_W: f32 = 296.0;

pub fn show(ui: &mut Ui, st: &mut UiState, s: &Shared) {
    if s.cards.is_empty() {
        empty(ui);
        return;
    }
    let mut cards: Vec<&Card> = s.cards.iter().collect();
    cards.sort_by_key(|c| std::cmp::Reverse(c.updated));
    let gap = 12.0;
    let per_row = (((ui.available_width() + gap) / (CARD_W + gap)).floor() as usize).max(1);
    ui.spacing_mut().item_spacing = Vec2::splat(gap);
    for row in cards.chunks(per_row) {
        ui.horizontal_top(|ui| {
            for c in row {
                card(ui, st, c);
            }
        });
    }
}

fn empty(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 120.0), Sense::hover());
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
    let title = p.layout("No bunker heard yet".into(), theme::bold(15.0), DIM, wrap);
    let body = p.layout(
        "Each bunker that transmits gets a card with its map grid, lock code and a map link."
            .into(),
        theme::text(13.0),
        FAINT,
        wrap,
    );
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
