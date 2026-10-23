//! First-run walkthrough, also reachable from the help button.

mod scenes;

use scenes::illustration;

use egui::{Area, Color32, Order, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};

use super::widgets::{ButtonKind, button};
use crate::theme::{self, AMBER, DIM, RULE, STROKE, SURFACE, TEXT};

const PAGES: usize = 3;

pub enum Outcome {
    Open,
    Close,
}

pub fn show(
    ctx: &egui::Context,
    ids: &(egui::Id, egui::Id),
    page: &mut usize,
    still: bool,
) -> Outcome {
    let screen = ctx.content_rect();
    let mut outcome = Outcome::Open;
    let last = *page + 1 == PAGES;
    // arrow keys turn the pages; consumed first so they do not also move the keyboard focus
    let (left, right) = ctx.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight),
        )
    });
    if right && !last {
        *page += 1;
    }
    if left && *page > 0 {
        *page -= 1;
    }
    let shown_page = ctx.data(|d| d.get_temp::<usize>(ids.1));
    let refocus = shown_page != Some(*page) || ctx.memory(|m| m.focused().is_none());
    ctx.data_mut(|d| d.insert_temp(ids.1, *page));

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
                    illustration(ui, *page, size.x, still);
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
                                            let next = button(ui, "Next", ButtonKind::Primary);
                                            primary_focus(ui.ctx(), &next, refocus);
                                            if next.clicked() {
                                                *page += 1;
                                            }
                                        } else {
                                            let start =
                                                button(ui, "Start listening", ButtonKind::Primary);
                                            primary_focus(ui.ctx(), &start, refocus);
                                            if start.clicked() {
                                                outcome = Outcome::Close;
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

/// The primary button holds the focus on each page, and keeps the left and right arrows for page turning.
fn primary_focus(ctx: &egui::Context, response: &egui::Response, refocus: bool) {
    if refocus {
        response.request_focus();
    }
    ctx.memory_mut(|m| {
        m.set_focus_lock_filter(
            response.id,
            egui::EventFilter {
                horizontal_arrows: true,
                ..Default::default()
            },
        )
    });
}

fn dots(ui: &mut Ui, page: usize) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(PAGES as f32 * 18.0, 32.0), Sense::hover());
    for i in 0..PAGES {
        let c = Pos2::new(rect.left() + 8.0 + i as f32 * 18.0, rect.center().y);
        if i == page {
            ui.painter()
                .rect_filled(Rect::from_center_size(c, Vec2::new(16.0, 6.0)), 3.0, AMBER);
        } else {
            ui.painter().circle_filled(c, 3.0, RULE);
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

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;

    fn harness() -> Harness<'static, usize> {
        let ids = (egui::Id::unique("backdrop"), egui::Id::unique("modal"));
        Harness::builder()
            .with_context_setup(theme::install)
            .build_ui_state(
                move |ui, page: &mut usize| {
                    let _ = show(ui.ctx(), &ids, page, false);
                },
                0usize,
            )
    }

    #[test]
    fn arrow_keys_turn_pages_and_stop_at_the_ends() {
        let mut h = harness();
        h.run_steps(4);
        h.key_press(egui::Key::ArrowLeft);
        h.run_steps(4);
        assert_eq!(*h.state(), 0, "left on the first page stays");
        h.key_press(egui::Key::ArrowRight);
        h.run_steps(4);
        h.key_press(egui::Key::ArrowRight);
        h.run_steps(4);
        h.key_press(egui::Key::ArrowRight);
        h.run_steps(4);
        assert_eq!(*h.state(), PAGES - 1, "right stops on the last page");
        h.key_press(egui::Key::ArrowLeft);
        h.run_steps(4);
        assert_eq!(*h.state(), PAGES - 2);
    }

    #[test]
    fn enter_presses_the_focused_primary_button() {
        let mut h = harness();
        h.run_steps(4);
        h.key_press(egui::Key::Enter);
        h.run_steps(4);
        assert_eq!(
            *h.state(),
            1,
            "Enter on the initially focused Next advances"
        );
    }
}
