//! A Morse cheat sheet for the ten digits, in the same dots and dashes as the live ribbon. Tapping a digit plays its
//! pattern; while a message arrives, the digits that still fit what has been heard so far light up.

use egui::{Align2, Role, Sense, Stroke, StrokeKind, Ui, Vec2, WidgetInfo};
use nasdara_decoder::morse::pattern_of;

use super::widgets::{caption, card, focus_ring, paint_symbols_with};
use super::{UiState, slots};
use crate::engine::Shared;
use crate::theme::{self, AMBER, DIM, RULE, STROKE, SURFACE_HI, TEXT, alpha};

const TILE: Vec2 = Vec2::new(88.0, 82.0);
const GAP: f32 = 8.0;
/// Playback timing in seconds, in the ratio Morse uses (a dash is three dots) and slow enough to follow.
const DOT_S: f64 = 0.3;
const DASH_S: f64 = 0.9;
const PAUSE_S: f64 = 0.3;
/// How long a finished pattern stays lit, and how long a reduced-motion tap stays marked.
const LINGER_S: f64 = 0.8;
const STILL_S: f64 = 2.5;
/// Morse order: the ones that start with dots, then the ones that start with dashes.
const ORDER: &str = "1234567890";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Upcoming,
    Playing,
    Played,
}

/// Where each symbol of `pattern` is `t` seconds into its playback.
fn progress(pattern: &str, t: f64) -> Vec<Step> {
    let mut start = 0.0;
    pattern
        .chars()
        .map(|c| {
            let len = if c == '-' { DASH_S } else { DOT_S };
            let step = if t < start {
                Step::Upcoming
            } else if t < start + len {
                Step::Playing
            } else {
                Step::Played
            };
            start += len + PAUSE_S;
            step
        })
        .collect()
}

fn playback_length(pattern: &str) -> f64 {
    pattern
        .chars()
        .map(|c| (if c == '-' { DASH_S } else { DOT_S }) + PAUSE_S)
        .sum()
}

fn spoken(pattern: &str) -> String {
    pattern
        .chars()
        .map(|c| if c == '-' { "dash" } else { "dot" })
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn show(ui: &mut Ui, st: &mut UiState, s: &Shared) {
    let live = slots::live_symbols(s);
    let now = ui.input(|i| i.time);
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        caption(ui, "Morse cheat sheet");
        ui.add_space(4.0);
        ui.add(
            egui::Label::new(
                egui::RichText::new(
                    "Only digits are sent, each as five beeps: a short blip is a dot, a long beep is a dash. \
                     Tap a digit to see its pattern. While a message arrives, the digits that still fit what \
                     you have heard light up.",
                )
                .font(theme::text(12.5))
                .color(DIM),
            )
            .wrap(),
        );
        ui.add_space(8.0);
        let columns = if ui.available_width() >= 10.0 * TILE.x + 9.0 * GAP {
            10
        } else {
            5
        };
        let digits: Vec<char> = ORDER.chars().collect();
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(GAP);
            for row in digits.chunks(columns) {
                ui.horizontal(|ui| {
                    for &d in row {
                        tile(ui, st, d, &live, now);
                    }
                });
            }
        });
        ui.add_space(4.0);
        ui.add(
            egui::Label::new(
                egui::RichText::new(
                    "The pattern is regular. 1 to 5: that many dots, then dashes. 6 to 9: dashes first (one for 6, \
                     up to four for 9), then dots. 0 is five dashes.",
                )
                .font(theme::text(12.5))
                .color(DIM),
            )
            .wrap(),
        );
    });
}

fn tile(ui: &mut Ui, st: &mut UiState, digit: char, live: &str, now: f64) {
    let pattern = pattern_of(digit);
    let (rect, response) = ui.allocate_exact_size(TILE, Sense::click());
    let still = st.settings.reduce_motion;
    if response.clicked() {
        st.guide_play = Some((digit, now));
    }
    // the tapped digit's playback, as a step per symbol; none once it has finished and lingered
    let playing = st
        .guide_play
        .filter(|&(d, _)| d == digit)
        .map(|(_, at)| now - at)
        .filter(|&t| {
            t < if still {
                STILL_S
            } else {
                playback_length(pattern) + LINGER_S
            }
        });
    let steps = playing.map(|t| {
        if still {
            vec![Step::Playing; pattern.len()]
        } else {
            progress(pattern, t)
        }
    });
    if let Some(t) = playing {
        if still {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs_f64(STILL_S - t));
        } else {
            ui.ctx().request_repaint();
        }
    }
    let heard = live.chars().count();
    let fits = heard > 0 && pattern.starts_with(live);
    let fill = if fits && heard == pattern.len() {
        alpha(AMBER, 0.14)
    } else {
        SURFACE_HI
    };
    let edge = if fits {
        Stroke::new(1.5, alpha(AMBER, 0.7))
    } else if response.hovered() {
        Stroke::new(1.0, RULE)
    } else {
        Stroke::new(1.0, STROKE)
    };
    let p = ui.painter();
    p.rect_filled(rect, 10.0, fill);
    p.rect_stroke(rect, 10.0, edge, StrokeKind::Inside);
    p.text(
        rect.center_top() + Vec2::new(0.0, 28.0),
        Align2::CENTER_CENTER,
        digit,
        theme::digits(30.0),
        if fits { AMBER } else { TEXT },
    );
    let symbols_at = rect.center_bottom() - Vec2::new(0.0, 20.0);
    paint_symbols_with(p, symbols_at, pattern, 1.0, |i| {
        match steps.as_ref().map(|s| s[i]) {
            Some(Step::Playing) => AMBER,
            Some(Step::Played) => alpha(AMBER, 0.55),
            Some(Step::Upcoming) => RULE,
            None if fits && i < heard => AMBER,
            None => DIM,
        }
    });
    focus_ring(p, &response, 10.0);
    let label = format!("Digit {digit}: {}", spoken(pattern));
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, &label));
    response.on_hover_cursor(egui::CursorIcon::PointingHand);
}

#[cfg(test)]
mod tests {
    use super::*;
    use Step::{Played, Playing, Upcoming};

    #[test]
    fn playback_walks_through_the_symbols_in_order() {
        assert_eq!(progress("-.", 0.1), [Playing, Upcoming]);
        assert_eq!(progress("-.", 1.0), [Played, Upcoming]);
        assert_eq!(progress("-.", 1.3), [Played, Playing]);
        assert_eq!(progress("-.", 5.0), [Played, Played]);
    }

    #[test]
    fn a_pattern_plays_for_the_sum_of_its_symbols_and_pauses() {
        assert!((playback_length("-.") - (0.9 + 0.3 + 0.3 + 0.3)).abs() < 1e-9);
    }

    #[test]
    fn every_digit_of_the_sheet_has_a_five_symbol_pattern() {
        assert_eq!(ORDER.chars().count(), 10);
        for d in ORDER.chars() {
            assert_eq!(pattern_of(d).len(), 5, "digit {d}");
        }
    }

    #[test]
    fn the_spoken_pattern_names_each_symbol() {
        assert_eq!(spoken("-.."), "dash, dot, dot");
    }
}
