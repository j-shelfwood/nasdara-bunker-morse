mod cards;
mod onboarding;
mod signal;
mod slots;
mod widgets;

use std::time::Instant;

use egui::{
    Align, CentralPanel, Layout, Panel, Pos2, Rect, RichText, Sense, Stroke, Ui, UserAttentionType,
    Vec2, ViewportCommand, WindowLevel,
};
use serde::{Deserialize, Serialize};

use crate::capture::{Source, Status};
use crate::engine::{Command, Handle, Mode, Note, Shared};
use crate::theme::{self, AMBER, DIM, FAINT, GREEN, RED, STROKE, SURFACE, alpha};
use widgets::{ButtonKind, Pill, button, caption, card, pill, segmented};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub source: Source,
    pub onboarded: bool,
    pub always_on_top: bool,
    pub flash_taskbar: bool,
    pub open_map_for_new: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            source: Source::Game,
            onboarded: false,
            always_on_top: false,
            flash_taskbar: true,
            open_map_for_new: false,
        }
    }
}

pub struct UiState {
    pub settings: Settings,
    pub onboarding: Option<usize>,
    slot_seen: [(Option<char>, f64); 10],
    copied: Option<(String, Instant)>,
    selected: Option<String>,
    on_top_sent: Option<bool>,
    modal_ids: (egui::Id, egui::Id),
}

impl UiState {
    pub fn new(settings: Settings) -> UiState {
        let onboarding = (!settings.onboarded).then_some(0);
        UiState {
            settings,
            onboarding,
            slot_seen: [(None, 0.0); 10],
            copied: None,
            selected: None,
            on_top_sent: None,
            modal_ids: (
                egui::Id::unique("onboarding-backdrop"),
                egui::Id::unique("onboarding"),
            ),
        }
    }
}

pub fn draw(ui: &mut Ui, engine: &Handle, st: &mut UiState) {
    let ctx = ui.ctx().clone();
    let mut s = engine.lock();

    // animation drives its own repaints; idle windows repaint a few times a second so clocks and ages stay current
    if s.receiving() || s.status == Status::Listening {
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    } else {
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }

    react_to_new_bunkers(&ctx, st, &mut s);
    let on_top = st.settings.always_on_top;
    if st.on_top_sent != Some(on_top) {
        st.on_top_sent = Some(on_top);
        ctx.send_viewport_cmd(ViewportCommand::WindowLevel(if on_top {
            WindowLevel::AlwaysOnTop
        } else {
            WindowLevel::Normal
        }));
    }

    Panel::top("header")
        .frame(
            egui::Frame::new()
                .fill(theme::BG)
                .inner_margin(egui::Margin::symmetric(20, 12)),
        )
        .show(ui, |ui| header(ui, engine, st, &s));
    Panel::bottom("footer")
        .frame(
            egui::Frame::new()
                .fill(theme::BG)
                .inner_margin(egui::Margin::symmetric(20, 8)),
        )
        .show(ui, |ui| footer(ui, engine, st, &s));
    CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(theme::BG)
                .inner_margin(egui::Margin {
                    left: 20,
                    right: 20,
                    top: 4,
                    bottom: 4,
                }),
        )
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 14.0;
                    banner(ui, engine, &s);
                    live_card(ui, st, &s);
                    bunkers_card(ui, engine, st, &s);
                });
        });

    drop(s);
    if let Some(page) = st.onboarding {
        let mut page = page;
        match onboarding::show(&ctx, &st.modal_ids, &mut page) {
            onboarding::Outcome::Open => st.onboarding = Some(page),
            onboarding::Outcome::Close => finish_onboarding(st),
            onboarding::Outcome::Demo => {
                finish_onboarding(st);
                engine.send(Command::StartDemo);
            }
        }
    }
}

fn finish_onboarding(st: &mut UiState) {
    st.onboarding = None;
    st.settings.onboarded = true;
}

fn react_to_new_bunkers(ctx: &egui::Context, st: &UiState, s: &mut Shared) {
    for key in std::mem::take(&mut s.changed) {
        if st.settings.flash_taskbar {
            ctx.send_viewport_cmd(ViewportCommand::RequestUserAttention(
                UserAttentionType::Informational,
            ));
        }
        let url = s
            .cards
            .iter()
            .find(|c| c.key == key && c.count == 1)
            .and_then(|c| c.url.clone());
        if let (true, Some(url)) = (st.settings.open_map_for_new, url)
            && nasdara_decoder::bunkers::is_izurvive_url(&url)
        {
            let _ = open::that(url);
        }
    }
}

fn header(ui: &mut Ui, engine: &Handle, st: &mut UiState, s: &Shared) {
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
        let (label, dot, pulse) = status_chip(s, ui.input(|i| i.time) as f32);
        pill(ui, &Pill { dot, pulse, label });

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let help = widgets::icon_button(ui, false, "How it works", |p, r, c| {
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
                st.settings.always_on_top,
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
            let gear = widgets::icon_button(ui, false, "Options", |p, r, c| {
                for k in 0..3 {
                    let y = r.top() + 2.0 + k as f32 * (r.height() - 4.0) / 2.0;
                    p.line_segment(
                        [Pos2::new(r.left(), y), Pos2::new(r.right(), y)],
                        Stroke::new(1.6, c),
                    );
                }
            });
            egui::Popup::menu(&gear).show(|ui| {
                ui.set_min_width(250.0);
                ui.checkbox(
                    &mut st.settings.flash_taskbar,
                    "Flash the taskbar when a bunker is read",
                );
                ui.checkbox(
                    &mut st.settings.open_map_for_new,
                    "Open the map for each new bunker",
                );
                ui.separator();
                if ui.button("Forget the bunkers heard so far").clicked() {
                    engine.send(Command::ClearCards);
                }
                if ui.button("Show the walkthrough again").clicked() {
                    st.onboarding = Some(0);
                }
            });
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

fn status_chip(s: &Shared, time: f32) -> (String, egui::Color32, f32) {
    let breathe = |speed: f32| (time * speed).fract();
    if s.mode == Mode::Demo {
        return ("Demo, not real audio".into(), theme::BLUE, breathe(0.8));
    }
    if s.receiving() {
        return ("Receiving a transmission".into(), AMBER, breathe(1.6));
    }
    match &s.status {
        Status::Listening => (
            if s.source == Source::Game {
                "Listening to DayZ".into()
            } else {
                "Listening to system audio".into()
            },
            GREEN,
            breathe(0.5),
        ),
        Status::WaitingForGame => ("Waiting for DayZ".into(), FAINT, 0.0),
        Status::Failed(_) => ("Cannot listen".into(), RED, 0.0),
    }
}

fn banner(ui: &mut Ui, engine: &Handle, s: &Shared) {
    let silent = s.mode == Mode::Live
        && s.source == Source::Game
        && s.status == Status::Listening
        && s.silent_for > 20.0;
    let mut lines: Vec<(egui::Color32, String, Option<&str>)> = Vec::new();
    if silent {
        lines.push((AMBER, format!("Nothing audible from DayZ for {} seconds. On some PCs Windows does not hand the game's sound to the app.", s.silent_for as u64), Some("Listen to system audio")));
    }
    if let Status::Failed(e) = &s.status {
        lines.push((RED, e.clone(), None));
    }
    for n in &s.notes {
        match n {
            Note::Info(t) => lines.push((DIM, t.clone(), None)),
            Note::Warn(t) => lines.push((AMBER, t.clone(), None)),
        }
    }
    for (color, text, action) in lines {
        let mut act = false;
        egui::Frame::new()
            .fill(alpha(color, 0.08))
            .stroke(Stroke::new(1.0, alpha(color, 0.35)))
            .corner_radius(10.0)
            .inner_margin(egui::Margin::symmetric(14, 9))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(text)
                            .font(theme::text(13.0))
                            .color(theme::TEXT),
                    );
                    if let Some(a) = action {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            act = button(ui, a, ButtonKind::Quiet).clicked()
                        });
                    }
                });
            });
        if act {
            engine.send(Command::SetSource(Source::System));
        }
    }
}

fn live_card(ui: &mut Ui, st: &mut UiState, s: &Shared) {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        let v = slots::view(s);
        ui.horizontal(|ui| {
            ui.label(RichText::new(&v.headline).font(theme::bold(15.0)));
        });
        ui.label(RichText::new(&v.hint).font(theme::text(12.5)).color(DIM));
        ui.add_space(4.0);
        slots::show(ui, st, &v);
        ui.add_space(6.0);
        signal::show(ui, s);
    });
}

fn bunkers_card(ui: &mut Ui, engine: &Handle, st: &mut UiState, s: &Shared) {
    let _ = engine;
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            caption(ui, "Bunkers heard");
            ui.label(
                RichText::new(format!(
                    "{} of 23",
                    s.cards.iter().filter(|c| c.bunker.is_some()).count()
                ))
                .font(theme::text(12.5))
                .color(DIM),
            );
        });
        ui.add_space(4.0);
        let map_side = 200.0;
        ui.horizontal_top(|ui| {
            let list_w = (ui.available_width() - map_side - 16.0).max(320.0);
            ui.vertical(|ui| {
                ui.set_width(list_w);
                ui.set_max_width(list_w);
                cards::show(ui, st, s);
            });
            cards::map(ui, st, s, map_side);
        });
    });
}

fn footer(ui: &mut Ui, engine: &Handle, st: &mut UiState, s: &Shared) {
    ui.horizontal(|ui| {
        // level meter
        let (rect, _) = ui.allocate_exact_size(Vec2::new(120.0, 8.0), Sense::hover());
        ui.painter().rect_filled(rect, 4.0, SURFACE);
        ui.painter().rect_stroke(
            rect,
            4.0,
            Stroke::new(1.0, STROKE),
            egui::StrokeKind::Inside,
        );
        let level = (s.level.sqrt()).clamp(0.0, 1.0);
        ui.painter().rect_filled(
            Rect::from_min_size(rect.min, Vec2::new(rect.width() * level, rect.height())),
            4.0,
            if level > 0.02 { GREEN } else { FAINT },
        );
        ui.label(
            RichText::new("Audio level")
                .font(theme::text(12.0))
                .color(DIM),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                    .font(theme::text(12.0))
                    .color(FAINT),
            );
            ui.add_space(8.0);
            if s.mode == Mode::Demo {
                if button(ui, "Stop demo", ButtonKind::Quiet).clicked() {
                    engine.send(Command::StopDemo);
                }
            } else if button(ui, "Run the demo", ButtonKind::Quiet).clicked() {
                engine.send(Command::StartDemo);
            }
        });
    });
    let _ = st;
}
