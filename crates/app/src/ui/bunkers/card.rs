use egui::{
    Align, Align2, Layout, RichText, Role, Sense, Stroke, StrokeKind, Ui, Vec2, WidgetInfo,
};

use super::CARD_W;
use super::reading::{Reading, reading, reading_color, reading_mark};
use crate::engine::Card;
use crate::theme::{self, AMBER, DIM, RED, STROKE, SURFACE_HI, TEAL, TEXT, alpha};
use crate::ui::UiState;
use crate::ui::widgets::{ButtonKind, button, caption};
use nasdara_decoder::bunkers::is_izurvive_url;

pub(super) fn card(ui: &mut Ui, st: &mut UiState, c: &Card) {
    let age = c.updated.elapsed().as_secs_f32();
    let still = st.settings.reduce_motion;
    let fresh = if still {
        0.0
    } else {
        (1.0 - age / 3.0).clamp(0.0, 1.0)
    };
    if fresh > 0.0 {
        ui.ctx().request_repaint();
    }
    let selected = st.selected.as_deref() == Some(&c.key);
    let stroke = if selected {
        alpha(AMBER, 0.7)
    } else {
        theme::mix(STROKE, TEAL, fresh * 0.8)
    };
    let frame = egui::Frame::new()
        .fill(SURFACE_HI)
        .stroke(Stroke::new(1.0 + fresh, stroke))
        .corner_radius(12.0)
        .inner_margin(egui::Margin::same(14));
    let response = frame
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_width(CARD_W - 28.0);
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);
                ui.horizontal(|ui| {
                    reading_mark(ui, reading(c));
                    match c.bunker {
                        Some(n) => caption(ui, &format!("Bunker {n}")),
                        None => caption(ui, "Unknown bunker"),
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let heard = if c.count == 1 {
                            "heard once".to_string()
                        } else {
                            format!("heard {}×", c.count)
                        };
                        ui.label(RichText::new(heard).font(theme::text(12.5)).color(DIM));
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
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    if let Some(url) = c.url.as_deref().filter(|u| is_izurvive_url(u))
                        && button(ui, "Open map", ButtonKind::Primary).clicked()
                    {
                        let _ = open::that(url);
                    }
                    let copied = st
                        .copied
                        .as_ref()
                        .is_some_and(|(k, at)| k == &c.key && at.elapsed().as_secs_f32() < 1.6);
                    if button(
                        ui,
                        if copied { "Copied" } else { "Copy code" },
                        ButtonKind::Quiet,
                    )
                    .clicked()
                    {
                        ui.ctx().copy_text(c.code.clone());
                        st.copied = Some((c.key.clone(), std::time::Instant::now()));
                    }
                });
            })
        })
        .response;
    let response = response.interact(Sense::click());
    let spoken = card_spoken(c, selected);
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, selected, spoken.clone()));
    if response.clicked() {
        st.selected = if selected { None } else { Some(c.key.clone()) };
    }
}

/// What a screen reader says for a card.
fn card_spoken(c: &Card, selected: bool) -> String {
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
        "{name}, grid {}, lock code {}, {state}. {} on the map",
        spaced_grid(&c.grid).replace('?', "unclear"),
        digits(&c.code),
        if selected {
            "Highlighted"
        } else {
            "Select to highlight"
        }
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
            ui.painter().rect_filled(rect, 9.0, theme::BG);
            let border = if unclear {
                alpha(RED, 0.35 + 0.25 * (time * 3.0).sin().abs())
            } else {
                alpha(AMBER, 0.35)
            };
            ui.painter()
                .rect_stroke(rect, 9.0, Stroke::new(1.0, border), StrokeKind::Inside);
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
                "{n} digit{} unclear. Keep listening, the broadcast repeats.",
                if n == 1 { "" } else { "s" }
            )
        }
        Reading::Confirmed => format!("Read on {} transmissions", c.count),
        Reading::Once => "Heard once. Check it against the next transmission.".to_string(),
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
        ui.painter()
            .circle_filled(rect.center() + Vec2::new(0.0, 1.0), 3.5, color);
        ui.add(egui::Label::new(RichText::new(text).font(theme::text(12.5)).color(DIM)).wrap());
    });
}
