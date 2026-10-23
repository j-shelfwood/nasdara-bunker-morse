//! One card per bunker heard, and a map of where they are.

mod card;
mod reading;
pub(super) mod stamp;

pub use reading::{Reading, reading, reading_color};

use egui::{Align2, Sense, Stroke, Ui, Vec2};

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
    p.text(
        rect.center() - Vec2::new(0.0, 12.0),
        Align2::CENTER_CENTER,
        "No bunker heard yet",
        theme::bold(15.0),
        DIM,
    );
    p.text(
        rect.center() + Vec2::new(0.0, 12.0),
        Align2::CENTER_CENTER,
        "Each bunker that transmits gets a card with its map grid, lock code and a map link.",
        theme::text(13.0),
        FAINT,
    );
}
