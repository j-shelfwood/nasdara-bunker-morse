//! A Morse cheat sheet for the ten digits, in the same dots and dashes as the live ribbon. Tapping a digit plays its
//! pattern; while a message arrives, the digits that still fit what has been heard so far light up.

use egui::emath::easing::cubic_out;
use egui::{Align2, Id, Margin, Rect, Role, Sense, Stroke, Ui, Vec2, WidgetInfo};
use nasdara_decoder::morse::pattern_of;

use super::anim::Motion;
use super::hud::{Panel, paint_focus, paint_frame, panel};
use super::widgets::{caption, paint_symbols_with, symbols_width};
use super::{UiState, bar};
use crate::engine::Shared;
use crate::theme::{self, AMBER, DIM, RULE, STROKE, SURFACE_HI, TEXT, mix};

/// Five columns by two rows. The tile size follows the room the column plan gives: width 40 to 84 px, height 34 to 64.
const COLUMNS: usize = 5;
const TILE_H_MIN: f32 = 34.0;
const TILE_H_MAX: f32 = 64.0;
/// Vertical margins of the panel, and the height of its caption row with the space under it.
const MARGIN_Y: f32 = 12.0;
const CAPTION_H: f32 = 18.0;
/// The two explanatory notes take about this much; they are left out below `NOTES_FROM` points of panel height.
const NOTES_H: f32 = 86.0;
const NOTES_FROM: f32 = 230.0;
/// Share of the tile height holding the digit; the Morse symbols sit in the rest.
const DIGIT_ZONE: f32 = 0.62;
const GAP: f32 = 3.0;
const TILE_CUT: f32 = 5.0;
/// Seconds a tile takes to light up or cool down.
const EASE_S: f32 = 0.2;
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

/// Tile width for a row `avail` wide.
fn tile_width(avail: f32) -> f32 {
    ((avail - GAP * (COLUMNS - 1) as f32) / COLUMNS as f32).clamp(40.0, 84.0)
}

/// Scale at which the widest pattern (five dashes) fits a tile of width `w` with a margin.
fn symbol_scale(w: f32) -> f32 {
    ((w - 8.0) / symbols_width("-----")).min(1.0)
}

/// The part of the tile that holds the digit.
fn digit_zone(rect: Rect) -> Rect {
    Rect::from_min_size(
        rect.min,
        Vec2::new(rect.width(), rect.height() * DIGIT_ZONE),
    )
}

/// Where the symbols are centred: the middle of what the digit zone leaves.
fn symbols_center(rect: Rect) -> egui::Pos2 {
    let rest = Rect::from_min_max(egui::pos2(rect.left(), digit_zone(rect).bottom()), rect.max);
    rest.center()
}

/// Tile height for a panel `height` tall, with or without the notes.
fn tile_height(height: f32, notes: bool) -> f32 {
    let rest = 2.0 * MARGIN_Y + CAPTION_H + GAP + if notes { NOTES_H } else { 0.0 };
    ((height - rest) / 2.0).clamp(TILE_H_MIN, TILE_H_MAX)
}

/// The panel in `height` points: two rows of tiles that take what the caption and, in a tall panel, the two notes leave.
pub fn show(ui: &mut Ui, st: &mut UiState, s: &Shared, height: f32) {
    let live = bar::view::live_symbols(s);
    let now = ui.input(|i| i.time);
    let look = Panel {
        margin: Margin::symmetric(9, MARGIN_Y as i8),
        ..Panel::default()
    };
    let notes = height >= NOTES_FROM;
    let tile_h = tile_height(height, notes);
    panel(ui, look, |ui| {
        // the spaces are explicit, so the panel is as tall as `tile_height` counted
        ui.spacing_mut().item_spacing.y = 0.0;
        caption(ui, "Morse cheat sheet");
        ui.add_space(4.0);
        if notes {
            note(
                ui,
                "Each digit is five beeps: short is a dot, long is a dash. Tap a digit to play it; digits that \
                 fit what you hear light up.",
            );
            ui.add_space(8.0);
        }
        let w = tile_width(ui.available_width());
        let digits: Vec<char> = ORDER.chars().collect();
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(GAP);
            for row in digits.chunks(COLUMNS) {
                ui.horizontal(|ui| {
                    for &d in row {
                        tile(ui, st, d, &live, now, Vec2::new(w, tile_h));
                    }
                });
            }
        });
        if notes {
            ui.add_space(6.0);
            note(
                ui,
                "1 to 5: that many dots, then dashes. 6 to 9: dashes first, then dots. 0 is five dashes.",
            );
        }
    });
}

fn note(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(egui::RichText::new(text).font(theme::text(12.5)).color(DIM)).wrap());
}

fn tile(ui: &mut Ui, st: &mut UiState, digit: char, live: &str, now: f64, size: Vec2) {
    let pattern = pattern_of(digit);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let still = st.settings.reduce_motion;
    let motion = Motion::new(still);
    let ctx = ui.ctx().clone();
    let id = |what: &str, i: usize| Id::unique(("cheat-tile", digit, what, i));
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
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(STILL_S - t));
        } else {
            ctx.request_repaint();
        }
    }
    let heard = live.chars().count();
    let fits = heard > 0 && pattern.starts_with(live);
    let ease = |what: &str, i: usize, target: f32| {
        motion.toward(&ctx, id(what, i), target, EASE_S, cubic_out)
    };
    let lit = ease("fit", 0, f32::from(u8::from(fits)));
    let full = ease(
        "full",
        0,
        f32::from(u8::from(fits && heard == pattern.len())),
    );
    let hot = ease("hover", 0, f32::from(u8::from(response.hovered())));
    let busy = ease("busy", 0, f32::from(u8::from(playing.is_some())));
    let fill = mix(SURFACE_HI, mix(SURFACE_HI, AMBER, 0.14), full);
    let edge_rest = mix(STROKE, RULE, hot);
    let edge = Stroke::new(
        1.0 + 0.5 * lit,
        mix(edge_rest, mix(edge_rest, AMBER, 0.7), lit),
    );
    let p = ui.painter();
    paint_frame(p, rect, fill, edge, TILE_CUT);
    p.text(
        digit_zone(rect).center(),
        Align2::CENTER_CENTER,
        digit,
        theme::digits(24.0),
        mix(TEXT, AMBER, lit),
    );
    let base = mix(DIM, RULE, busy);
    let symbols = symbols_center(rect);
    paint_symbols_with(p, symbols, pattern, symbol_scale(rect.width()), |i| {
        let target = match steps.as_ref().map(|s| s[i]) {
            Some(Step::Playing) => 1.0,
            Some(Step::Played) => 0.55,
            Some(Step::Upcoming) => 0.0,
            None if fits && i < heard => 1.0,
            None => 0.0,
        };
        mix(base, AMBER, ease("sym", i, target))
    });
    paint_focus(p, &response, TILE_CUT);
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

    #[test]
    fn five_columns_fit_the_300_px_column() {
        let w = tile_width(282.0);
        assert!((w - 54.0).abs() < 1e-4, "{w}");
        assert!(w * 5.0 + GAP * 4.0 <= 282.0 + 1e-3);
        assert_eq!(tile_width(1000.0), 84.0);
    }

    #[test]
    fn the_tiles_follow_the_panel_height_within_their_range() {
        use crate::ui::layout::{GUIDE_H_MAX, GUIDE_H_MIN};
        assert!(tile_height(GUIDE_H_MIN, false) >= TILE_H_MIN);
        assert!(tile_height(GUIDE_H_MAX, true) <= TILE_H_MAX);
        assert!(tile_height(200.0, false) > tile_height(150.0, false));
        assert_eq!(tile_height(1000.0, true), TILE_H_MAX);
        // the notes appear before the panel is at its tallest
        const { assert!(GUIDE_H_MAX >= NOTES_FROM) };
    }

    #[test]
    fn the_widest_pattern_fits_inside_its_tile() {
        for w in [40.0, 54.0, 64.0] {
            assert!(
                symbols_width("-----") * symbol_scale(w) <= w - 8.0 + 1e-3,
                "{w}"
            );
        }
    }

    #[test]
    fn digit_and_symbols_split_the_tile_without_overlap() {
        let rect = Rect::from_min_size(egui::pos2(10.0, 20.0), Vec2::new(54.0, TILE_H_MAX));
        let zone = digit_zone(rect);
        assert_eq!(zone.center().x, rect.center().x);
        assert!(zone.bottom() < symbols_center(rect).y);
        assert!(symbols_center(rect).y < rect.bottom());
    }
}
