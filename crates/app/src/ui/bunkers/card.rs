use egui::emath::easing::cubic_out;
use egui::{Align, Align2, Id, Layout, RichText, Role, Sense, Stroke, Ui, Vec2, WidgetInfo};

use super::actions::{self, SPOKEN_HINT, Target};
use super::copied::copied_recently;
use super::reading::{Reading, reading, reading_color, reading_mark};
use crate::engine::Card;
use crate::theme::{self, AMBER, DIM, RED, STROKE, SURFACE_HI, TEAL, TEXT, alpha};
use crate::ui::UiState;
use crate::ui::anim::Motion;
use crate::ui::anim::{changed_at, egui_time_of, first_seen};
use crate::ui::hud::{Panel, paint_focus, paint_frame, panel};
use crate::ui::widgets::caption;

/// Seconds a card takes to fade in, and how long the border glow and the count flash take to settle.
const APPEAR_S: f32 = 0.32;
const GLOW_S: f32 = 1.6;
const COUNT_S: f32 = 0.5;
/// Corner cut of one lock code cell.
const CELL_CUT: f32 = 5.0;

pub(in crate::ui) fn card(ui: &mut Ui, st: &mut UiState, c: &Card) {
    let ctx = ui.ctx().clone();
    let still = st.settings.reduce_motion;
    let motion = Motion::new(still);
    let key = |what: &str| Id::unique(("bunker-card", &c.key, what));
    let appear = 1.0
        - motion.pulse(
            &ctx,
            Some(first_seen(&ctx, key("first"))),
            APPEAR_S,
            cubic_out,
        );
    let updated = egui_time_of(&ctx, key("updated"), c.updated);
    let fresh = motion.pulse(&ctx, Some(updated), GLOW_S, cubic_out);
    let counted = changed_at(&ctx, key("count"), &c.count);
    let flash = motion.pulse(&ctx, counted, COUNT_S, cubic_out);
    let selected = st.selected.as_deref() == Some(&c.key);
    let stroke = if selected {
        alpha(AMBER, 0.7)
    } else {
        theme::mix(STROKE, TEAL, fresh * 0.8)
    };
    let look = Panel {
        fill: SURFACE_HI,
        stroke: Stroke::new(1.0 + fresh * 0.5, stroke),
        margin: egui::Margin::same(14),
        min_height: Some(ui.available_height()),
        ..Panel::default()
    };
    let response = ui
        .scope(|ui| {
            ui.set_opacity(appear);
            panel(ui, look, |ui| card_body(ui, st, c, still, flash)).response
        })
        .inner;
    let response = response.interact(Sense::click());
    paint_focus(ui.painter(), &response, CELL_CUT);
    let spoken = format!("{}. {SPOKEN_HINT}", card_facts(c));
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, spoken.clone()));
    let target = Target {
        key: &c.key,
        code: &c.code,
        url: c.url.as_deref(),
    };
    actions::menu(&response, st, &target);
    response.on_hover_cursor(egui::CursorIcon::PointingHand);
}

fn card_body(ui: &mut Ui, st: &UiState, c: &Card, still: bool, flash: f32) {
    ui.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);
    let copied = copied_recently(ui.ctx(), st, &c.key);
    ui.horizontal(|ui| {
        reading_mark(ui, reading(c));
        match c.bunker {
            Some(n) => caption(ui, &format!("Bunker {n}")),
            None => caption(ui, "Unknown bunker"),
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let (text, color) = if copied {
                ("code copied".to_string(), TEAL)
            } else if c.count == 1 {
                ("heard once".to_string(), theme::mix(DIM, AMBER, flash))
            } else {
                (format!("heard {}×", c.count), theme::mix(DIM, AMBER, flash))
            };
            ui.label(RichText::new(text).font(theme::text(12.5)).color(color));
        });
    });
    ui.label(
        RichText::new(spaced_grid(&c.grid))
            .font(theme::digits(28.0))
            .color(TEXT)
            .extra_letter_spacing(1.0),
    );
    code_row(ui, c, still);
    status_line(ui, c);
}

/// What a screen reader says for a tile: the card's facts, and whether the map marks it.
pub(super) fn card_spoken(c: &Card, selected: bool) -> String {
    format!(
        "{}. {} on the map",
        card_facts(c),
        if selected {
            "Highlighted"
        } else {
            "Select to highlight"
        }
    )
}

/// The bunker, its grid, its lock code and how sure the reading is.
fn card_facts(c: &Card) -> String {
    let digits = |s: &str| {
        s.chars()
            .map(|ch| {
                if ch == '?' {
                    "unclear".to_string()
                } else {
                    ch.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let name = c
        .bunker
        .map_or("Unknown bunker".to_string(), |n| format!("Bunker {n}"));
    let state = match reading(c) {
        Reading::Confirmed => format!("read on {} transmissions", c.count),
        Reading::Once => "heard once".to_string(),
        Reading::Unclear => format!("{} digits unclear", c.unclear),
    };
    format!(
        "{name}, grid {}, lock code {}, {state}",
        spaced_grid(&c.grid).replace('?', "unclear"),
        digits(&c.code),
    )
}

fn spaced_grid(grid: &str) -> String {
    if grid.len() == 6 {
        format!("{} {}", &grid[..3], &grid[3..])
    } else {
        grid.to_string()
    }
}

fn code_row(ui: &mut Ui, c: &Card, still: bool) {
    let time = if still {
        0.0
    } else {
        ui.input(|i| i.time) as f32
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for ch in c.code.chars() {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(40.0, 54.0), Sense::hover());
            let unclear = ch == '?';
            let border = if unclear {
                alpha(RED, 0.35 + 0.25 * (time * 3.0).sin().abs())
            } else {
                alpha(AMBER, 0.35)
            };
            paint_frame(
                ui.painter(),
                rect,
                theme::BG,
                Stroke::new(1.0, border),
                CELL_CUT,
            );
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                ch.to_string(),
                theme::digits(36.0),
                if unclear { RED } else { AMBER },
            );
        }
        ui.add_space(4.0);
        ui.label(
            RichText::new("lock code")
                .font(theme::text(12.0))
                .color(DIM),
        );
    });
    if c.code.contains('?') && !still {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
    }
}

fn status_line(ui: &mut Ui, c: &Card) {
    let r = reading(c);
    let text = match r {
        Reading::Unclear => {
            let n = c.unclear;
            format!(
                "{n} digit{} unclear. Keep listening.",
                if n == 1 { "" } else { "s" }
            )
        }
        Reading::Confirmed => format!("Read on {} transmissions", c.count),
        Reading::Once => "Heard once. Check it on the next one.".to_string(),
    };
    status_row(ui, r, &text);
    if c.corrected {
        status_row(
            ui,
            Reading::Once,
            "Grid adjusted to the nearest known bunker",
        );
    }
}

fn status_row(ui: &mut Ui, r: Reading, text: &str) {
    let color = reading_color(r);
    ui.horizontal_top(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(8.0, 16.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 3.5, color);
        ui.add(egui::Label::new(RichText::new(text).font(theme::text(12.5)).color(DIM)).wrap());
    });
}
