//! First-run walkthrough, also reachable from the help button.

use egui::{Align2, Area, Color32, Order, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};

use super::widgets::{ButtonKind, button, paint_symbols};
use crate::theme::{self, AMBER, DIM, FAINT, GREEN, STROKE, SURFACE, SURFACE_HI, TEXT, alpha};

const PAGES: usize = 3;

pub enum Outcome {
    Open,
    Close,
    Demo,
}

pub fn show(ctx: &egui::Context, ids: &(egui::Id, egui::Id), page: &mut usize) -> Outcome {
    let screen = ctx.content_rect();
    let mut outcome = Outcome::Open;

    Area::new(ids.0)
        .order(Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
            ui.painter()
                .rect_filled(rect, 0.0, Color32::from_black_alpha(190));
        });

    let size = Vec2::new(
        660.0_f32.min(screen.width() - 40.0),
        470.0_f32.min(screen.height() - 40.0),
    );
    let pos = screen.center() - size / 2.0;
    Area::new(ids.1)
        .order(Order::Tooltip)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(SURFACE)
                .stroke(Stroke::new(1.0, STROKE))
                .corner_radius(16.0)
                .inner_margin(egui::Margin::same(0))
                .show(ui, |ui| {
                    ui.set_min_size(size);
                    ui.set_max_size(size);
                    illustration(ui, *page, size.x);
                    ui.add_space(8.0);
                    egui::Frame::new()
                        .inner_margin(egui::Margin::symmetric(32, 0))
                        .show(ui, |ui| {
                            ui.set_width(size.x - 64.0);
                            body(ui, *page);
                        });
                    ui.add_space(ui.available_height() - 58.0);
                    egui::Frame::new()
                        .inner_margin(egui::Margin::symmetric(28, 0))
                        .show(ui, |ui| {
                            ui.set_width(size.x - 56.0);
                            ui.horizontal(|ui| {
                                dots(ui, *page);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if *page + 1 < PAGES {
                                            if button(ui, "Next", ButtonKind::Primary).clicked() {
                                                *page += 1;
                                            }
                                        } else {
                                            if button(ui, "Start listening", ButtonKind::Primary)
                                                .clicked()
                                            {
                                                outcome = Outcome::Close;
                                            }
                                            if button(ui, "Try the demo", ButtonKind::Quiet)
                                                .clicked()
                                            {
                                                outcome = Outcome::Demo;
                                            }
                                        }
                                        if *page > 0
                                            && button(ui, "Back", ButtonKind::Quiet).clicked()
                                        {
                                            *page -= 1;
                                        }
                                    },
                                );
                            });
                        });
                });
        });

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        outcome = Outcome::Close;
    }
    outcome
}

fn dots(ui: &mut Ui, page: usize) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(PAGES as f32 * 18.0, 32.0), Sense::hover());
    for i in 0..PAGES {
        let c = Pos2::new(rect.left() + 8.0 + i as f32 * 18.0, rect.center().y);
        if i == page {
            ui.painter()
                .rect_filled(Rect::from_center_size(c, Vec2::new(16.0, 6.0)), 3.0, AMBER);
        } else {
            ui.painter().circle_filled(c, 3.0, FAINT);
        }
    }
}

fn body(ui: &mut Ui, page: usize) {
    let (title, lines): (&str, &[&str]) = match page {
        0 => (
            "Read the bunker broadcast",
            &[
                "The bunkers on Nasdara announce their location and lock code over the radio, in Morse code.",
                "This app listens to the game's audio and turns the beeps into digits, so you can read the broadcast instead of hearing it.",
            ],
        ),
        1 => (
            "In the game",
            &[
                "1.  Start DayZ and join a Nasdara server.",
                "2.  Hold a powered radio tuned to the bunker frequency.",
                "3.  Wait for the broadcast window, 17:45 to 06:00 game time.",
                "Three of the 23 bunkers transmit at a time, about 90 seconds each, and the cycle repeats. A digit missed on one pass is filled in on the next.",
            ],
        ),
        _ => (
            "Your audio stays on your PC",
            &[
                "The app listens to DayZ's audio only. Nothing is recorded, saved or sent anywhere, and the app makes no network connections.",
                "If it stays silent while the game plays sound, switch to System audio at the top right. That listens to everything your PC plays, and only accepts a message that matches a real bunker.",
            ],
        ),
    };
    ui.label(RichText::new(title).font(theme::bold(24.0)).color(TEXT));
    ui.add_space(6.0);
    for l in lines {
        ui.label(RichText::new(*l).font(theme::text(14.5)).color(DIM));
        ui.add_space(2.0);
    }
}

/// A looping picture for each page, painted from the same shapes the live ribbon uses.
fn illustration(ui: &mut Ui, page: usize, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 150.0), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(
        rect,
        egui::CornerRadius {
            nw: 16,
            ne: 16,
            sw: 0,
            se: 0,
        },
        theme::BG,
    );
    let t = ui.input(|i| i.time) as f32;
    ui.ctx().request_repaint();
    match page {
        0 => morse_loop(&p, rect, t),
        1 => radio_scene(&p, rect, t),
        _ => shield(&p, rect, t),
    }
}

fn morse_loop(p: &egui::Painter, rect: Rect, t: f32) {
    // a message sliding in as Morse and settling into digits
    let message = [('1', ".----"), ('2', "..---"), ('9', "----.")];
    let cycle = (t * 0.35).fract();
    let y = rect.center().y;
    for (i, (digit, symbols)) in message.iter().enumerate() {
        let cx = rect.center().x + (i as f32 - 1.0) * 150.0;
        let local = ((cycle * 3.0 - i as f32 * 0.55) / 1.0).clamp(0.0, 1.0);
        paint_symbols(
            p,
            Pos2::new(cx, y - 18.0),
            symbols,
            1.7,
            alpha(AMBER, 0.35 + 0.65 * (1.0 - local)),
        );
        p.text(
            Pos2::new(cx, y + 28.0),
            Align2::CENTER_CENTER,
            digit.to_string(),
            theme::digits(44.0),
            alpha(TEXT, local),
        );
    }
}

fn radio_scene(p: &egui::Painter, rect: Rect, t: f32) {
    let c = Pos2::new(rect.center().x - 120.0, rect.center().y + 18.0);
    // radio body
    let body = Rect::from_center_size(c, Vec2::new(70.0, 52.0));
    p.rect_filled(body, 9.0, SURFACE_HI);
    p.rect_stroke(body, 9.0, Stroke::new(1.5, DIM), egui::StrokeKind::Inside);
    p.line_segment(
        [
            body.right_top() - Vec2::new(14.0, 0.0),
            body.right_top() + Vec2::new(-6.0, -30.0),
        ],
        Stroke::new(2.0, DIM),
    );
    p.circle_filled(body.center() - Vec2::new(14.0, 0.0), 10.0, theme::BG);
    p.circle_stroke(
        body.center() - Vec2::new(14.0, 0.0),
        10.0,
        Stroke::new(1.2, FAINT),
    );
    p.rect_filled(
        Rect::from_center_size(body.center() + Vec2::new(16.0, -8.0), Vec2::new(18.0, 6.0)),
        2.0,
        alpha(GREEN, 0.5 + 0.4 * (t * 2.0).sin().abs()),
    );
    // waves from a bunker on the right travelling to the radio
    let origin = Pos2::new(rect.center().x + 150.0, rect.center().y + 10.0);
    p.rect_filled(
        Rect::from_center_size(origin + Vec2::new(0.0, 8.0), Vec2::new(44.0, 26.0)),
        4.0,
        SURFACE_HI,
    );
    p.rect_stroke(
        Rect::from_center_size(origin + Vec2::new(0.0, 8.0), Vec2::new(44.0, 26.0)),
        4.0,
        Stroke::new(1.5, FAINT),
        egui::StrokeKind::Inside,
    );
    p.text(
        origin + Vec2::new(0.0, -18.0),
        Align2::CENTER_CENTER,
        "bunker",
        theme::text(11.0),
        FAINT,
    );
    for k in 0..4 {
        let phase = (t * 0.6 + k as f32 * 0.25).fract();
        let x = origin.x - 40.0 - phase * 190.0;
        p.line_segment(
            [
                Pos2::new(x, origin.y - 14.0 - 12.0 * phase),
                Pos2::new(x, origin.y + 14.0 + 12.0 * phase),
            ],
            Stroke::new(2.0, alpha(AMBER, 0.8 * (1.0 - phase))),
        );
    }
}

fn shield(p: &egui::Painter, rect: Rect, t: f32) {
    let c = rect.center();
    // sound in, digits out, nothing leaving: a closed box
    let boxr = Rect::from_center_size(c, Vec2::new(150.0, 86.0));
    p.rect_filled(boxr, 12.0, SURFACE_HI);
    p.rect_stroke(
        boxr,
        12.0,
        Stroke::new(1.5, alpha(GREEN, 0.6)),
        egui::StrokeKind::Inside,
    );
    p.text(c, Align2::CENTER_CENTER, "your PC", theme::bold(15.0), TEXT);
    for k in 0..3 {
        let phase = (t * 0.5 + k as f32 / 3.0).fract();
        let x = boxr.left() - 130.0 + phase * 120.0;
        p.circle_filled(
            Pos2::new(x, c.y),
            4.0,
            alpha(AMBER, (1.0 - (phase - 0.5).abs() * 2.0).max(0.0)),
        );
    }
    p.text(
        Pos2::new(boxr.left() - 70.0, c.y - 28.0),
        Align2::CENTER_CENTER,
        "game audio",
        theme::text(11.5),
        FAINT,
    );
    p.line_segment(
        [
            boxr.right_center() + Vec2::new(14.0, 0.0),
            boxr.right_center() + Vec2::new(70.0, 0.0),
        ],
        Stroke::new(1.5, alpha(FAINT, 0.6)),
    );
    p.text(
        boxr.right_center() + Vec2::new(44.0, -16.0),
        Align2::CENTER_CENTER,
        "no network",
        theme::text(11.5),
        FAINT,
    );
    p.line_segment(
        [
            boxr.right_center() + Vec2::new(64.0, -8.0),
            boxr.right_center() + Vec2::new(78.0, 8.0),
        ],
        Stroke::new(2.0, theme::RED),
    );
    p.line_segment(
        [
            boxr.right_center() + Vec2::new(64.0, 8.0),
            boxr.right_center() + Vec2::new(78.0, -8.0),
        ],
        Stroke::new(2.0, theme::RED),
    );
}
