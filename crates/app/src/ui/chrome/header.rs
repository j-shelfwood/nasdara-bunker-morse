//! The title row: logo, status chip, help, keep-on-top, options menu and the audio source switch.

use egui::{Align, Layout, Pos2, RichText, Stroke, Ui, Vec2};

use super::options;
use crate::capture::Source;
use crate::engine::{Command, Handle, Shared};
use crate::theme::{self, DIM};
use crate::ui::UiState;
use crate::ui::widgets::{self, Pill, pill, segmented};

pub fn show(ui: &mut Ui, engine: &Handle, st: &mut UiState, s: &Shared) {
    ui.horizontal(|ui| {
        crate::logo::paint(ui, 30.0);
        ui.add_space(4.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(RichText::new("Bunker Morse").font(theme::bold(17.0)));
            ui.label(
                RichText::new("Nasdara broadcast decoder")
                    .font(theme::text(11.5))
                    .color(DIM),
            );
        });
        ui.add_space(18.0);
        let (label, dot, pulse) = options::status_chip(s, ui.input(|i| i.time) as f32);
        let pulse = if st.settings.reduce_motion {
            0.0
        } else {
            pulse
        };
        pill(ui, &Pill { dot, pulse, label });

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let help = widgets::icon_button(ui, None, "How it works", |p, r, c| {
                p.circle_stroke(r.center(), r.width() / 2.0, Stroke::new(1.5, c));
                p.text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    "?",
                    theme::bold(13.0),
                    c,
                );
            });
            if help.clicked() {
                st.onboarding = Some(0);
            }
            let pin = widgets::icon_button(
                ui,
                Some(st.settings.always_on_top),
                "Keep this window above the game",
                |p, r, c| {
                    p.circle_filled(r.center() - Vec2::new(0.0, 2.0), 3.6, c);
                    p.line_segment(
                        [
                            r.center() + Vec2::new(0.0, 1.0),
                            r.center() + Vec2::new(0.0, r.height() / 2.0),
                        ],
                        Stroke::new(1.8, c),
                    );
                },
            );
            if pin.clicked() {
                st.settings.always_on_top = !st.settings.always_on_top;
            }
            let gear = widgets::icon_button(ui, None, "Options", |p, r, c| {
                for k in 0..3 {
                    let y = r.top() + 2.0 + k as f32 * (r.height() - 4.0) / 2.0;
                    p.line_segment(
                        [Pos2::new(r.left(), y), Pos2::new(r.right(), y)],
                        Stroke::new(1.6, c),
                    );
                }
            });
            egui::Popup::menu(&gear).show(|ui| options::menu(ui, engine, st, s));
            ui.add_space(6.0);
            let current = if s.source == Source::Game { 0 } else { 1 };
            if let Some(i) = segmented(ui, ["DayZ", "System audio"], current) {
                let source = if i == 0 { Source::Game } else { Source::System };
                st.settings.source = source;
                engine.send(Command::SetSource(source));
            }
        });
    });
}
