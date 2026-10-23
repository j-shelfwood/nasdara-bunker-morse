use egui::{Align2, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::engine::Card;
use crate::theme::{self, AMBER, RED, TEAL};
use crate::ui::widgets::{paint_check, paint_question};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Reading {
    /// Every digit read and the bunker heard at least twice.
    Confirmed,
    /// Heard once.
    Once,
    /// A lock code digit is still unreadable.
    Unclear,
}

pub fn reading(c: &Card) -> Reading {
    if c.unclear > 0 {
        Reading::Unclear
    } else if c.count >= 2 {
        Reading::Confirmed
    } else {
        Reading::Once
    }
}

pub fn reading_color(r: Reading) -> egui::Color32 {
    match r {
        Reading::Confirmed => TEAL,
        Reading::Once => AMBER,
        Reading::Unclear => RED,
    }
}

/// The state of a card as a shape: a check in a ring, a `1x` badge or a `?` in a ring.
pub(super) fn reading_mark(ui: &mut Ui, r: Reading) {
    let color = reading_color(r);
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
    let p = ui.painter();
    match r {
        Reading::Confirmed => {
            p.circle_stroke(rect.center(), 9.0, Stroke::new(1.6, color));
            paint_check(p, rect.center(), 8.0, color, 2.0);
        }
        Reading::Once => {
            p.rect_stroke(rect, 6.0, Stroke::new(1.6, color), StrokeKind::Inside);
            p.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "1×",
                theme::bold(11.0),
                color,
            );
        }
        Reading::Unclear => paint_question(p, rect.center(), 20.0, color),
    }
}
