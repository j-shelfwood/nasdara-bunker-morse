//! The result of the last transmission, in large type at the top of the live card. It stays until the next
//! transmission starts being received.

use egui::{Align, Align2, Layout, RichText, Sense, Stroke, Ui, Vec2};
use nasdara_decoder::bunkers::is_izurvive_url;

use super::UiState;
use super::widgets::{ButtonKind, button, paint_check, paint_question};
use crate::engine::{Card, Finished, Shared};
use crate::theme::{self, AMBER, DIM, RED, TEAL, TEXT, alpha};

/// Seconds the border keeps flashing after a result arrives.
const ARRIVAL_S: f32 = 2.5;

pub struct Result {
    pub bunker: Option<usize>,
    pub grid: String,
    pub code: String,
    pub unclear: bool,
}

impl Result {
    pub fn of(f: &Finished) -> Result {
        let grid: String = f.digits.iter().take(6).collect();
        let code: String = f.digits.iter().skip(6).take(4).collect();
        let unclear = f.digits.contains(&'?');
        Result {
            bunker: f.bunker,
            grid,
            code,
            unclear,
        }
    }

    pub fn title(&self) -> String {
        match self.bunker {
            Some(n) => format!("Bunker {n}"),
            None => "Unknown bunker".into(),
        }
    }

    pub fn grid_spaced(&self) -> String {
        if self.grid.chars().count() == 6 {
            format!(
                "{} {}",
                self.grid.chars().take(3).collect::<String>(),
                self.grid.chars().skip(3).collect::<String>()
            )
        } else {
            self.grid.clone()
        }
    }
}

pub fn show(ui: &mut Ui, st: &mut UiState, s: &Shared) {
    let Some(f) = s.finished.as_ref().filter(|_| !s.receiving()) else {
        return;
    };
    let r = Result::of(f);
    let card: Option<&Card> = s.cards.iter().find(|c| match r.bunker {
        Some(b) => c.bunker == Some(b),
        None => c.grid == r.grid,
    });
    let is_new = card.is_none_or(|c| c.count <= 1);
    let arrival = (1.0 - f.heard.elapsed().as_secs_f32() / ARRIVAL_S).clamp(0.0, 1.0);
    if arrival > 0.0 {
        ui.ctx().request_repaint();
    }
    let accent = if r.unclear { AMBER } else { TEAL };
    let response = egui::Frame::new()
        .fill(alpha(accent, 0.07 + 0.08 * arrival))
        .stroke(Stroke::new(
            1.5 + 1.5 * arrival,
            alpha(accent, 0.55 + 0.45 * arrival),
        ))
        .corner_radius(12.0)
        .inner_margin(egui::Margin::symmetric(16, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = Vec2::new(10.0, 4.0);
            ui.horizontal(|ui| {
                let tag = if is_new { "NEW" } else { "HEARD AGAIN" };
                tag_chip(ui, tag, accent);
                ui.label(
                    RichText::new(r.title().to_uppercase())
                        .font(theme::bold(16.0))
                        .color(TEXT),
                );
                state_glyph(ui, r.unclear, accent);
                ui.label(
                    RichText::new(if r.unclear {
                        "some digits unclear"
                    } else {
                        "all digits read"
                    })
                    .font(theme::text(13.0))
                    .color(DIM),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let code = card.map_or(r.code.clone(), |c| c.code.clone());
                    let copied = st
                        .copied
                        .as_ref()
                        .is_some_and(|(k, at)| k == "latest" && at.elapsed().as_secs_f32() < 1.6);
                    if button(
                        ui,
                        if copied { "Copied" } else { "Copy code" },
                        ButtonKind::Quiet,
                    )
                    .clicked()
                    {
                        ui.ctx().copy_text(code);
                        st.copied = Some(("latest".into(), std::time::Instant::now()));
                    }
                    if let Some(url) = card
                        .and_then(|c| c.url.as_deref())
                        .filter(|u| is_izurvive_url(u))
                        && button(ui, "Open map", ButtonKind::Primary).clicked()
                    {
                        let _ = open::that(url);
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.label(RichText::new("GRID").font(theme::bold(11.0)).color(DIM));
                ui.label(
                    RichText::new(r.grid_spaced())
                        .font(theme::digits(44.0))
                        .color(TEXT)
                        .extra_letter_spacing(2.0),
                );
                ui.add_space(24.0);
                ui.label(
                    RichText::new("LOCK CODE")
                        .font(theme::bold(11.0))
                        .color(DIM),
                );
                ui.label(
                    RichText::new(spaced_code(&r.code))
                        .font(theme::digits(44.0))
                        .color(if r.code.contains('?') { RED } else { accent })
                        .extra_letter_spacing(2.0),
                );
            });
        })
        .response;
    let spoken = spoken_text(&r);
    response
        .interact(Sense::focusable_noninteractive())
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Status, true, spoken.clone()));
}

/// What a screen reader says for the result.
pub fn spoken_text(r: &Result) -> String {
    let said = |s: &str| s.replace('?', "unclear");
    let code: Vec<String> = r.code.chars().map(|c| said(&c.to_string())).collect();
    let mut out = format!(
        "{}, grid {}, lock code {}",
        r.title(),
        said(&r.grid_spaced()),
        code.join(" ")
    );
    if r.unclear {
        out.push_str(", some digits unclear");
    }
    out
}

fn spaced_code(code: &str) -> String {
    code.chars()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn tag_chip(ui: &mut Ui, text: &str, color: egui::Color32) {
    let galley = ui
        .painter()
        .layout_no_wrap(text.into(), theme::bold(12.0), theme::BG);
    let (rect, _) = ui.allocate_exact_size(galley.size() + Vec2::new(16.0, 8.0), Sense::hover());
    ui.painter().rect_filled(rect, 6.0, color);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, theme::BG);
}

fn state_glyph(ui: &mut Ui, unclear: bool, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
    if unclear {
        paint_question(ui.painter(), rect.center(), 22.0, color);
    } else {
        ui.painter()
            .circle_stroke(rect.center(), 10.0, Stroke::new(1.8, color));
        paint_check(ui.painter(), rect.center(), 9.0, color, 2.2);
    }
    let _ = Align2::CENTER_CENTER;
}
