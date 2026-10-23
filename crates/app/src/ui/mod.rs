mod cards;
mod latest;
mod onboarding;
mod signal;
mod slots;
mod widgets;

use std::collections::HashMap;
use std::time::Instant;

use egui::{
    Align, CentralPanel, Layout, Panel, Pos2, Rect, RichText, Sense, Stroke, Ui, UserAttentionType,
    Vec2, ViewportCommand, WindowLevel,
};
use serde::{Deserialize, Serialize};

use crate::capture::{Source, Status};
use crate::engine::{Command, Handle, Mode, Note, Shared};
use crate::theme::{self, AMBER, DIM, FAINT, RED, RULE, STROKE, SURFACE, TEAL, alpha};
use widgets::{ButtonKind, Pill, button, caption, card, pill, segmented};

/// Smallest window the layout is built for, in points.
pub const MIN_WINDOW: Vec2 = Vec2::new(760.0, 520.0);

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub source: Source,
    pub onboarded: bool,
    pub always_on_top: bool,
    pub flash_taskbar: bool,
    pub open_map_for_new: bool,
    /// No pulses, glows, blinking cursor or pop animations, and no fast repaint for them.
    pub reduce_motion: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            source: Source::Game,
            onboarded: false,
            always_on_top: false,
            flash_taskbar: true,
            open_map_for_new: false,
            reduce_motion: false,
        }
    }
}

pub struct UiState {
    pub settings: Settings,
    pub onboarding: Option<usize>,
    slot_seen: [slots::SlotSeen; 10],
    /// Grid and lock code last seen per card key, so only a new or changed card raises attention.
    card_seen: HashMap<String, (String, String)>,
    copied: Option<(String, Instant)>,
    selected: Option<String>,
    on_top_sent: Option<bool>,
    min_size_sent: bool,
    window_fitted: bool,
    modal_ids: (egui::Id, egui::Id),
}

impl UiState {
    pub fn new(settings: Settings) -> UiState {
        let onboarding = (!settings.onboarded).then_some(0);
        UiState {
            settings,
            onboarding,
            slot_seen: [slots::SlotSeen::default(); 10],
            card_seen: HashMap::new(),
            copied: None,
            selected: None,
            on_top_sent: None,
            min_size_sent: false,
            window_fitted: false,
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
    // a listening window scrolls the signal strip, so it needs a steady rate before a transmission is recognised
    if s.receiving() {
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    } else if s.status == Status::Listening && !st.settings.reduce_motion {
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    } else {
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }

    size_window(&ctx, st);
    let to_open = react_to_new_bunkers(&ctx, st, &mut s);
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
    // opening a link can be slow: never while the engine waits for the lock
    for url in to_open {
        let _ = open::that(url);
    }
    if let Some(page) = st.onboarding {
        let mut page = page;
        match onboarding::show(&ctx, &st.modal_ids, &mut page, st.settings.reduce_motion) {
            onboarding::Outcome::Open => st.onboarding = Some(page),
            onboarding::Outcome::Close => finish_onboarding(st),
            onboarding::Outcome::Demo => {
                finish_onboarding(st);
                engine.send(Command::StartDemo);
            }
        }
    }
}

/// The window opens at 1060x800 points whatever the monitor. On the first frame that knows the monitor size it is
/// cut down to 90% of the monitor, and the minimum size drops so a short window still shows the result.
fn size_window(ctx: &egui::Context, st: &mut UiState) {
    const OPEN: Vec2 = Vec2::new(1060.0, 800.0);
    if !st.min_size_sent {
        st.min_size_sent = true;
        ctx.send_viewport_cmd(ViewportCommand::MinInnerSize(MIN_WINDOW));
    }
    if st.window_fitted {
        return;
    }
    let Some(monitor) = ctx.input(|i| i.viewport().monitor_size) else {
        return;
    };
    st.window_fitted = true;
    let current = ctx
        .input(|i| i.viewport().inner_rect)
        .map_or(OPEN, |r| r.size());
    let limit = (monitor * 0.9).max(MIN_WINDOW);
    if current.x > limit.x || current.y > limit.y {
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(current.min(limit)));
    }
}

fn finish_onboarding(st: &mut UiState) {
    st.onboarding = None;
    st.settings.onboarded = true;
}

fn react_to_new_bunkers(ctx: &egui::Context, st: &mut UiState, s: &mut Shared) -> Vec<String> {
    let mut to_open = Vec::new();
    st.card_seen
        .retain(|key, _| s.cards.iter().any(|c| &c.key == key));
    for key in std::mem::take(&mut s.changed) {
        let Some(card) = s.cards.iter().find(|c| c.key == key) else {
            continue;
        };
        let now = (card.grid.clone(), card.code.clone());
        let previous = st.card_seen.insert(key, now.clone());
        let is_new = previous.is_none();
        if !is_new && previous.as_ref() == Some(&now) {
            continue;
        }
        if st.settings.flash_taskbar {
            ctx.send_viewport_cmd(ViewportCommand::RequestUserAttention(if is_new {
                UserAttentionType::Critical
            } else {
                UserAttentionType::Informational
            }));
        }
        if let (true, true, Some(url)) = (
            st.settings.open_map_for_new,
            card.count == 1,
            card.url.as_deref(),
        ) && nasdara_decoder::bunkers::is_izurvive_url(url)
        {
            to_open.push(url.to_string());
        }
    }
    to_open
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
                ui.checkbox(
                    &mut st.settings.reduce_motion,
                    "Reduce motion (no pulses, glows or blinking)",
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
            TEAL,
            // an idle window repaints a few times a second, too slowly for a smooth breathing dot
            0.0,
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
    if !s.engine_alive {
        lines.push((
            RED,
            "Listening stopped unexpectedly. Close the app and open it again.".to_string(),
            None,
        ));
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
                ui.set_width(ui.available_width());
                // the label wraps and the button drops to the next line when the row is full
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(text)
                            .font(theme::text(13.0))
                            .color(theme::TEXT),
                    );
                    if let Some(a) = action {
                        act = button(ui, a, ButtonKind::Quiet).clicked();
                    }
                });
            });
        if act {
            engine.send(Command::SetSource(Source::System));
        }
    }
}

fn live_card(ui: &mut Ui, st: &mut UiState, s: &Shared) {
    let size = signal::Size::for_window_height(ui.ctx().content_rect().height());
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        let v = slots::view(s);
        let strip = latest::show(ui, st, s);
        let compact = size.is_compact();
        // a short window with a result on screen gives its height to the cards: the strip holds the digits,
        // and the idle ribbon and the headline wait until the window grows or the next transmission starts
        let squeezed = compact && strip;
        if !squeezed {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&v.headline).font(theme::bold(20.0)));
            });
        }
        if !compact {
            ui.add(
                egui::Label::new(RichText::new(&v.hint).font(theme::text(12.5)).color(DIM)).wrap(),
            );
            ui.add_space(4.0);
        }
        if !squeezed {
            slots::show(ui, st, &v);
            ui.add_space(6.0);
            signal::show(ui, s, size, st.settings.reduce_motion);
        }
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
            if level > 0.02 { TEAL } else { RULE },
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
