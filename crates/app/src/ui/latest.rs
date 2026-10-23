//! The result of the last transmission, in large type at the top of the live card. It stays until the next
//! transmission starts being received.

use egui::emath::easing::cubic_out;
use egui::text::{LayoutJob, TextFormat};
use egui::{Align, Color32, FontId, Id, Layout, Margin, RichText, Sense, Stroke, Ui, Vec2};
use nasdara_decoder::bunkers::is_izurvive_url;

use super::UiState;
use super::anim::Motion;
use super::anim::egui_time_of;
use super::bunkers::copied::{copied_recently, mark_copied};
use super::hud::{Panel, paint_frame, panel};
use super::widgets::{ButtonKind, button, paint_check, paint_question};
use crate::engine::{Card, Finished, Shared};
use crate::theme::{self, AMBER, DIM, RED, TEAL, TEXT, alpha};

/// Seconds the border glow takes to settle after a result arrives, and the fade-in of the strip itself.
const ARRIVAL_S: f32 = 1.8;
const FADE_S: f32 = 0.28;
/// Inner width below which the strip stacks its rows (the 300 px column).
const WIDE_FROM: f32 = 560.0;
const GAP: f32 = 8.0;

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

/// Draws the strip; returns whether there was a result to show.
pub fn show(ui: &mut Ui, st: &mut UiState, s: &Shared) -> bool {
    let Some(f) = s.finished.as_ref().filter(|_| !s.receiving()) else {
        return false;
    };
    let r = Result::of(f);
    let card: Option<&Card> = s.cards.iter().find(|c| match r.bunker {
        Some(b) => c.bunker == Some(b),
        None => c.grid == r.grid,
    });
    let is_new = card.is_none_or(|c| c.count <= 1);
    let ctx = ui.ctx().clone();
    let motion = Motion::new(st.settings.reduce_motion);
    let heard = egui_time_of(&ctx, Id::unique("latest-heard"), f.heard);
    let arrival = motion.pulse(&ctx, Some(heard), ARRIVAL_S, cubic_out);
    let fade = 1.0 - motion.pulse(&ctx, Some(heard), FADE_S, cubic_out);
    let accent = if r.unclear { AMBER } else { TEAL };
    let look = Panel {
        fill: alpha(accent, 0.07 + 0.08 * arrival),
        stroke: Stroke::new(1.0 + 0.5 * arrival, alpha(accent, 0.55 + 0.45 * arrival)),
        margin: Margin::symmetric(16, 10),
        ..Panel::default()
    };
    let response = ui
        .scope(|ui| {
            ui.set_opacity(fade);
            panel(ui, look, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(10.0, 6.0);
                let wide = ui.available_width() >= WIDE_FROM;
                header(ui, st, &r, card, is_new, accent, wide);
                readouts(ui, &r, accent, wide);
            })
            .response
        })
        .inner;
    let spoken = spoken_text(&r);
    response
        .interact(Sense::focusable_noninteractive())
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Status, true, spoken.clone()));
    true
}

/// Tag, title, state and the two buttons: one row when wide, three stacked rows in a narrow column.
fn header(
    ui: &mut Ui,
    st: &mut UiState,
    r: &Result,
    card: Option<&Card>,
    is_new: bool,
    accent: Color32,
    wide: bool,
) {
    let tag = if is_new { "NEW" } else { "HEARD AGAIN" };
    let title = |ui: &mut Ui| {
        ui.label(
            RichText::new(r.title().to_uppercase())
                .font(theme::bold(22.0))
                .color(TEXT),
        );
    };
    let state = |ui: &mut Ui| {
        state_glyph(ui, r.unclear, accent);
        let text = if r.unclear {
            "some digits unclear"
        } else {
            "all digits read"
        };
        ui.label(RichText::new(text).font(theme::text(13.0)).color(DIM));
    };
    let buttons = |ui: &mut Ui, st: &mut UiState, map_first: bool| {
        let open_map = |ui: &mut Ui| {
            if let Some(url) = card
                .and_then(|c| c.url.as_deref())
                .filter(|u| is_izurvive_url(u))
                && button(ui, "Open map", ButtonKind::Primary).clicked()
            {
                let _ = open::that(url);
            }
        };
        if map_first {
            open_map(ui);
        }
        let label = if copied_recently(ui.ctx(), st, "latest") {
            "Copied"
        } else {
            "Copy code"
        };
        if button(ui, label, ButtonKind::Quiet).clicked() {
            ui.ctx()
                .copy_text(card.map_or(r.code.clone(), |c| c.code.clone()));
            mark_copied(st, "latest");
        }
        if !map_first {
            open_map(ui);
        }
    };
    if wide {
        ui.horizontal(|ui| {
            tag_chip(ui, tag, accent);
            title(ui);
            state(ui);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = GAP;
                buttons(ui, st, false);
            });
        });
    } else {
        ui.horizontal(|ui| {
            tag_chip(ui, tag, accent);
            state(ui);
        });
        title(ui);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            buttons(ui, st, true);
        });
    }
}

/// "GRID 133 021" and "LOCK CODE 9 1 3 6": a caption and its digits on one centre line, side by side when wide and
/// as two rows otherwise.
fn readouts(ui: &mut Ui, r: &Result, accent: Color32, wide: bool) {
    let size = if wide { 44.0 } else { 36.0 };
    let code_color = if r.code.contains('?') { RED } else { accent };
    let grid = r.grid_spaced();
    let code = spaced_code(&r.code);
    if wide {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 24.0;
            readout(ui, "GRID", &grid, TEXT, size);
            readout(ui, "LOCK CODE", &code, code_color, size);
        });
    } else {
        readout(ui, "GRID", &grid, TEXT, size);
        readout(ui, "LOCK CODE", &code, code_color, size);
    }
}

fn job(text: &str, font: FontId, color: Color32, spacing: f32) -> LayoutJob {
    LayoutJob::single_section(
        text.to_string(),
        TextFormat {
            font_id: font,
            color,
            extra_letter_spacing: spacing,
            ..Default::default()
        },
    )
}

/// One caption and its digits in a fixed-height box, both centred on the box's middle line.
fn readout(ui: &mut Ui, caption: &str, digits: &str, color: Color32, size: f32) {
    let cap = ui
        .painter()
        .layout_job(job(caption, theme::bold(11.0), DIM, 0.0));
    let num = ui
        .painter()
        .layout_job(job(digits, theme::digits(size), color, 2.0));
    let width = cap.size().x + GAP + num.size().x;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, size * 1.2), Sense::hover());
    let mid = rect.center().y;
    let p = ui.painter();
    p.galley(
        egui::pos2(rect.left(), mid - cap.size().y / 2.0),
        cap.clone(),
        DIM,
    );
    p.galley(
        egui::pos2(rect.left() + cap.size().x + GAP, mid - num.size().y / 2.0),
        num,
        color,
    );
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

fn tag_chip(ui: &mut Ui, text: &str, color: Color32) {
    let galley = ui
        .painter()
        .layout_no_wrap(text.into(), theme::bold(12.0), theme::BG);
    let (rect, _) = ui.allocate_exact_size(galley.size() + Vec2::new(16.0, 8.0), Sense::hover());
    paint_frame(ui.painter(), rect, color, Stroke::NONE, 4.0);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, theme::BG);
}

fn state_glyph(ui: &mut Ui, unclear: bool, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
    if unclear {
        paint_question(ui.painter(), rect.center(), 22.0, color);
    } else {
        ui.painter()
            .circle_stroke(rect.center(), 10.0, Stroke::new(1.8, color));
        paint_check(ui.painter(), rect.center(), 9.0, color, 2.2);
    }
}
