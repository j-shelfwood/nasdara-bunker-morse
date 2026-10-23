//! All 23 bunkers as a grid of tiles in bunker order, so a tile never moves: its number, and for a bunker that has
//! been heard its lock code and how sure the reading is. Clicking a heard tile selects it (the card appears in the
//! dossier slot and the map marks it). The tile height follows the room the column plan gives.

use egui::emath::easing::cubic_out;
use egui::text::{LayoutJob, TextFormat};
use egui::{Align2, Id, Rect, Role, Sense, Stroke, Ui, WidgetInfo, vec2};

use super::card::card_spoken;
use super::reading::{Reading, reading, reading_color};
use crate::engine::{Card, Shared};
use crate::theme::{
    self, AMBER, DIM, FAINT, RED, RULE, STROKE, SURFACE, SURFACE_HI, TEAL, TEXT, alpha, mix,
};
use crate::ui::UiState;
use crate::ui::anim::{Motion, egui_time_of};
use crate::ui::hud::{paint_focus, paint_frame};
use crate::ui::layout::{BUNKER_COUNT, TILE_GAP, TILE_H_MAX, roster_grid};
use crate::ui::widgets::{paint_check, paint_question};

const CUT: f32 = 4.0;
/// Seconds a tile's border takes to settle after its bunker was heard again, and to answer the pointer.
const GLOW_S: f32 = 1.6;
const HOVER_S: f32 = 0.16;
/// Below this tile height the code is drawn at its smallest size.
const CODE_MIN: f32 = 13.0;
const CODE_MAX: f32 = 26.0;
/// Height of the number row at the top of a tile.
const NUMBER_ROW: f32 = 12.0;

/// Where tile `index` (0-based) sits in `rect`: the grid `roster_grid` picks for its height, tiles at most `TILE_H_MAX` high.
fn tile_rect(rect: Rect, index: usize) -> Rect {
    let (cols, rows) = roster_grid(rect.height());
    let w = (rect.width() - TILE_GAP * (cols - 1) as f32) / cols as f32;
    let h = ((rect.height() - TILE_GAP * (rows - 1) as f32) / rows as f32).min(TILE_H_MAX);
    let (col, row) = ((index % cols) as f32, (index / cols) as f32);
    Rect::from_min_size(
        rect.min + vec2(col * (w + TILE_GAP), row * (h + TILE_GAP)),
        vec2(w, h),
    )
}

pub fn show(ui: &mut Ui, st: &mut UiState, s: &Shared, rect: Rect) {
    for n in 1..=BUNKER_COUNT {
        let card = s.cards.iter().find(|c| c.bunker == Some(n));
        tile(ui, st, n, tile_rect(rect, n - 1), card);
    }
}

fn tile(ui: &mut Ui, st: &mut UiState, n: usize, rect: Rect, card: Option<&Card>) {
    let ctx = ui.ctx().clone();
    let motion = Motion::new(st.settings.reduce_motion);
    let sense = if card.is_some() {
        Sense::click()
    } else {
        Sense::hover()
    };
    let response = ui.interact(rect, ui.make_persistent_id(("roster-tile", n)), sense);
    let selected = card.is_some_and(|c| st.selected.as_deref() == Some(&c.key));
    let key = |what: &str| Id::unique(("roster", n, what));
    let hot = motion.toward(
        &ctx,
        key("hover"),
        f32::from(u8::from(response.hovered() && card.is_some())),
        HOVER_S,
        cubic_out,
    );
    let p = ui.painter();
    let Some(c) = card else {
        paint_frame(p, rect, SURFACE, Stroke::new(1.0, alpha(STROKE, 0.7)), CUT);
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            n,
            theme::bold(13.0),
            FAINT,
        );
        let label = format!("Roster tile, bunker {n}, not heard yet");
        response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &label));
        return;
    };
    let r = reading(c);
    let tone = reading_color(r);
    let fresh = motion.pulse(
        &ctx,
        Some(egui_time_of(&ctx, key("updated"), c.updated)),
        GLOW_S,
        cubic_out,
    );
    let edge = if selected {
        alpha(AMBER, 0.95)
    } else {
        mix(
            mix(STROKE, tone, 0.5),
            mix(RULE, TEAL, fresh),
            hot.max(fresh) * 0.6,
        )
    };
    let fill = if selected {
        mix(SURFACE_HI, AMBER, 0.14)
    } else {
        mix(SURFACE_HI, tone, 0.06 + 0.06 * hot)
    };
    let width = if selected { 1.6 } else { 1.0 + 0.5 * fresh };
    paint_frame(p, rect, fill, Stroke::new(width, edge), CUT);
    p.text(
        rect.left_top() + vec2(7.0, 2.5),
        Align2::LEFT_TOP,
        n,
        theme::bold(10.5),
        if selected { AMBER } else { DIM },
    );
    paint_mark(p, rect.right_top() + vec2(-9.0, 9.0), r, tone);
    let size = ((rect.height() - NUMBER_ROW) * 0.62).clamp(CODE_MIN, CODE_MAX);
    let code = p.layout_job(code_job(&c.code, size));
    let mid = rect.top() + NUMBER_ROW + (rect.height() - NUMBER_ROW) / 2.0 - 1.0;
    p.galley(
        egui::pos2(
            rect.center().x - code.size().x / 2.0,
            mid - code.size().y / 2.0,
        ),
        code,
        TEXT,
    );
    paint_focus(p, &response, CUT);
    let spoken = format!(
        "Roster tile, {}",
        card_spoken(c, selected).replacen("Bunker", "bunker", 1)
    );
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, selected, spoken.clone()));
    if response.clicked() {
        st.selected = if selected { None } else { Some(c.key.clone()) };
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand);
}

/// Lock code digits, each unreadable one in red.
fn code_job(code: &str, size: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    for ch in code.chars() {
        job.append(
            &ch.to_string(),
            0.0,
            TextFormat {
                font_id: theme::digits(size),
                color: if ch == '?' { RED } else { TEXT },
                extra_letter_spacing: 0.5,
                ..Default::default()
            },
        );
    }
    job
}

/// The reading as a small shape in the tile's corner: a check, `1×` or a `?`; colour is never the only cue.
fn paint_mark(p: &egui::Painter, at: egui::Pos2, r: Reading, color: egui::Color32) {
    match r {
        Reading::Confirmed => paint_check(p, at, 7.0, color, 1.6),
        Reading::Once => {
            p.text(at, Align2::CENTER_CENTER, "1×", theme::bold(9.5), color);
        }
        Reading::Unclear => paint_question(p, at, 12.0, color),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tiles_fill_the_rect_in_bunker_order_without_overlap() {
        let rect = Rect::from_min_size(egui::pos2(10.0, 20.0), vec2(300.0, 220.0));
        let tiles: Vec<Rect> = (0..23).map(|i| tile_rect(rect, i)).collect();
        for (i, a) in tiles.iter().enumerate() {
            assert!(rect.expand(0.01).contains_rect(*a), "{i}: {a:?}");
            for b in &tiles[i + 1..] {
                assert!(!a.shrink(0.01).intersects(*b), "{a:?} {b:?}");
            }
        }
        assert_eq!(tiles[0].min, rect.min);
        assert!(tiles[1].left() > tiles[0].right());
        assert!(tiles[5].top() > tiles[0].bottom());
        assert!((tiles[4].right() - rect.right()).abs() < 0.01);
    }

    #[test]
    fn tiles_stop_growing_at_the_maximum_height() {
        let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), vec2(300.0, 900.0));
        assert_eq!(tile_rect(rect, 0).height(), TILE_H_MAX);
    }

    #[test]
    fn the_roster_has_a_tile_for_every_bunker() {
        assert_eq!(BUNKER_COUNT, nasdara_decoder::bunkers::bunkers().len());
    }

    #[test]
    fn a_short_roster_uses_six_columns_and_still_fits() {
        let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), vec2(300.0, 140.0));
        let tiles: Vec<Rect> = (0..23).map(|i| tile_rect(rect, i)).collect();
        assert!(tiles.iter().all(|t| rect.expand(0.01).contains_rect(*t)));
        assert!(tiles[5].left() > tiles[4].right() && tiles[6].top() > tiles[0].bottom());
    }
}
