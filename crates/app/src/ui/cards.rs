//! One card per bunker heard, and a map of where they are.

use egui::{Align, Align2, Layout, Pos2, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};
use nasdara_decoder::bunkers::{MAP_SIZE, bunkers, is_izurvive_url};

use super::UiState;
use super::widgets::{ButtonKind, button, caption};
use crate::engine::{Card, Shared};
use crate::theme::{self, AMBER, DIM, FAINT, GREEN, RED, STROKE, SURFACE_HI, TEXT, alpha};

const CARD_W: f32 = 296.0;

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
        Stroke::new(1.0, alpha(FAINT, 0.7)),
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

fn card(ui: &mut Ui, st: &mut UiState, c: &Card) {
    let age = c.updated.elapsed().as_secs_f32();
    let fresh = (1.0 - age / 3.0).clamp(0.0, 1.0);
    if fresh > 0.0 {
        ui.ctx().request_repaint();
    }
    let selected = st.selected.as_deref() == Some(&c.key);
    let stroke = if selected {
        alpha(AMBER, 0.7)
    } else {
        theme::mix(STROKE, GREEN, fresh * 0.8)
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
                        ui.label(RichText::new(heard).font(theme::text(12.0)).color(DIM));
                    });
                });
                ui.label(
                    RichText::new(spaced_grid(&c.grid))
                        .font(theme::digits(23.0))
                        .color(TEXT)
                        .extra_letter_spacing(1.0),
                );
                code_row(ui, c);
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
    if response.interact(Sense::click()).clicked() {
        st.selected = if selected { None } else { Some(c.key.clone()) };
    }
}

fn spaced_grid(grid: &str) -> String {
    if grid.len() == 6 {
        format!("{} {}", &grid[..3], &grid[3..])
    } else {
        grid.to_string()
    }
}

fn code_row(ui: &mut Ui, c: &Card) {
    let time = ui.input(|i| i.time) as f32;
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
    if c.code.contains('?') {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(50));
    }
}

fn status_line(ui: &mut Ui, c: &Card) {
    let (color, text) = if c.unclear > 0 {
        let n = c.unclear;
        (
            RED,
            format!(
                "{n} digit{} unclear. Keep listening, the broadcast repeats.",
                if n == 1 { "" } else { "s" }
            ),
        )
    } else if c.count >= 2 {
        (GREEN, format!("Confirmed over {} transmissions", c.count))
    } else {
        (
            AMBER,
            "Heard once. A second pass would confirm it.".to_string(),
        )
    };
    let color = if c.corrected && c.unclear == 0 && c.count < 2 {
        AMBER
    } else {
        color
    };
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 3.5, color);
        ui.label(RichText::new(text).font(theme::text(12.5)).color(DIM));
    });
}

/// The 23 bunkers on a square plan of the map. Bunkers that were heard glow; the selected card's bunker pulses.
pub fn map(ui: &mut Ui, st: &UiState, s: &Shared, side: f32) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
    let p = ui.painter_at(rect);
    let time = ui.input(|i| i.time) as f32;
    p.rect_filled(rect, 12.0, theme::BG);
    p.rect_stroke(rect, 12.0, Stroke::new(1.0, STROKE), StrokeKind::Inside);
    let inner = rect.shrink(12.0);
    for k in 1..16 {
        let f = k as f32 / 16.0;
        let c = alpha(STROKE, if k % 4 == 0 { 0.9 } else { 0.4 });
        p.line_segment(
            [
                Pos2::new(inner.left() + f * inner.width(), inner.top()),
                Pos2::new(inner.left() + f * inner.width(), inner.bottom()),
            ],
            Stroke::new(1.0, c),
        );
        p.line_segment(
            [
                Pos2::new(inner.left(), inner.top() + f * inner.height()),
                Pos2::new(inner.right(), inner.top() + f * inner.height()),
            ],
            Stroke::new(1.0, c),
        );
    }
    p.text(
        Pos2::new(rect.center().x, rect.top() + 7.0),
        Align2::CENTER_CENTER,
        "N",
        theme::bold(9.5),
        FAINT,
    );
    let to_screen = |x: f64, z: f64| {
        Pos2::new(
            inner.left() + (x / MAP_SIZE) as f32 * inner.width(),
            inner.top() + ((MAP_SIZE - z) / MAP_SIZE) as f32 * inner.height(),
        )
    };

    let hovered = response.hover_pos();
    let mut hint = None;
    for b in bunkers() {
        let pos = to_screen(b.x, b.z);
        let card = s.cards.iter().find(|c| c.bunker == Some(b.index));
        match card {
            None => {
                p.circle_filled(pos, 2.6, alpha(FAINT, 0.9));
            }
            Some(c) => {
                let sel = st.selected.as_deref() == Some(&c.key);
                let phase = (time * 0.9).fract();
                if sel || c.updated.elapsed().as_secs_f32() < 4.0 {
                    p.circle_stroke(
                        pos,
                        5.0 + 12.0 * phase,
                        Stroke::new(1.2, alpha(AMBER, 0.6 * (1.0 - phase))),
                    );
                }
                p.circle_filled(pos, 6.5, alpha(AMBER, 0.18));
                p.circle_filled(
                    pos,
                    4.2,
                    if c.unclear > 0 {
                        theme::mix(AMBER, RED, 0.5)
                    } else {
                        AMBER
                    },
                );
            }
        }
        if hovered.is_some_and(|h| h.distance(pos) < 8.0) {
            hint = Some(format!(
                "Bunker {} · grid {} {}",
                b.index,
                &b.grid[..3],
                &b.grid[3..]
            ));
        }
    }
    if let Some(h) = hint {
        response.on_hover_text(h);
    }
}
