//! The ten digit slots: six map grid digits and four lock code digits, each a chamfered box with the digit centred,
//! its Morse pattern and, once final, how much of the expected beep pattern was heard.

use egui::{Align2, Color32, Pos2, Rect, Role, Sense, Shape, Stroke, Ui, Vec2, WidgetInfo};
use nasdara_decoder::morse::pattern_of;

use super::view::{DIGITS, Phase, Slot, View};
use crate::theme::{
    self, AMBER, CUT, DIM, FAINT, HAIRLINE, RED, RULE, STROKE, SURFACE_HI, TEAL, TEXT, alpha,
};
use crate::ui::UiState;
use crate::ui::anim::{Motion, easing};
use crate::ui::hud::paint_frame;
use crate::ui::widgets::{paint_caption, paint_symbols, symbols_width};

const GAP: f32 = 5.0;
const GROUP_GAP: f32 = 10.0;
const BLOCK_GAP: f32 = 22.0;
const CAPTION_H: f32 = 12.0;
/// Word and bar under the digit of a final slot, in a tall and in a short slot.
const CONF_H: f32 = 19.0;
const CONF_H_SHORT: f32 = 13.0;
const SHORT_BELOW: f32 = 60.0;
const SYMBOLS_H: f32 = 14.0;
/// Slot height from which the Morse pattern fits under the digit.
const SYMBOLS_FROM: f32 = 80.0;
const POP_S: f32 = 0.33;
const RING_S: f32 = 0.7;

/// Gap after slot `i`: groups of three in the grid, then a wider one before the lock code.
fn gap_after(i: usize) -> f32 {
    match i {
        2 => GROUP_GAP,
        5 => BLOCK_GAP,
        _ => GAP,
    }
}

/// Slot rects centred in `area`, below the caption row. Width follows the area, height the room left (at most 2.6x).
fn slot_rects(area: Rect) -> Vec<Rect> {
    let fixed: f32 = (0..DIGITS - 1).map(gap_after).sum();
    let w = ((area.width() - fixed) / DIGITS as f32).clamp(22.0, 72.0);
    let total = w * DIGITS as f32 + fixed;
    let h = (area.height() - CAPTION_H - 4.0).min(w * 2.6).max(0.0);
    let top = area.top() + CAPTION_H + 4.0 + (area.height() - CAPTION_H - 4.0 - h) / 2.0;
    let mut x = area.center().x - total / 2.0;
    (0..DIGITS)
        .map(|i| {
            let r = Rect::from_min_size(Pos2::new(x, top), Vec2::new(w, h));
            x += w + gap_after(i);
            r
        })
        .collect()
}

pub fn show(ui: &mut Ui, st: &mut UiState, v: &View, area: Rect) {
    let ctx = ui.ctx().clone();
    let motion = Motion::new(st.settings.reduce_motion);
    let now = ctx.input(|i| i.time);
    let painter = ui.painter_at(area.expand(8.0));
    let rects = slot_rects(area);

    for (label, a, b) in [("Map grid", 0, 5), ("Lock code", 6, 9)] {
        let (x0, x1) = (rects[a].left(), rects[b].right());
        let y = area.top() + CAPTION_H / 2.0;
        let mid = (x0 + x1) / 2.0;
        paint_caption(
            &painter,
            Pos2::new(mid, y),
            Align2::CENTER_CENTER,
            label,
            DIM,
        );
        let half = painter
            .layout_no_wrap(label.into(), theme::bold(11.0), DIM)
            .size()
            .x
            / 2.0
            * 1.6
            + 8.0;
        let line = Stroke::new(HAIRLINE, STROKE);
        painter.line_segment([Pos2::new(x0, y), Pos2::new(mid - half, y)], line);
        painter.line_segment([Pos2::new(mid + half, y), Pos2::new(x1, y)], line);
    }

    let blink = motion.reduce || (now * 2.0).fract() < 0.5;
    let breathe = motion.breathe(&ctx, 0.64);
    for (i, slot) in v.slots.iter().enumerate() {
        let seen = &mut st.slot_seen[i];
        seen.observe(slot.ch, slot.phase == Phase::Final, now);
        let anim = Anim {
            pop: motion.pulse(&ctx, seen.changed_at, POP_S, easing::quadratic_out),
            ring: motion.pulse(&ctx, seen.settled_at, RING_S, easing::quadratic_out),
            breathe,
            blink,
        };
        let rect = rects[i];
        let response = ui.interact(rect, ui.make_persistent_id(("slot", i)), Sense::hover());
        response.widget_info(|| WidgetInfo::labeled(Role::Label, true, spoken(i, slot)));
        paint_slot(&painter, rect, slot, &v.live_symbols, anim);
    }
}

/// Screen reader text of a slot, which also carries the confidence word.
fn spoken(i: usize, slot: &Slot) -> String {
    let place = if i < 6 {
        format!("Map grid digit {}", i + 1)
    } else {
        format!("Lock code digit {}", i - 5)
    };
    match (slot.ch, slot.phase) {
        (Some('?'), _) => format!("{place}: unreadable"),
        (Some(c), Phase::Final) => format!("{place}: {c}, {} reading", confidence(slot.support).1),
        (Some(c), _) => format!("{place}: {c}, tentative"),
        (None, Phase::Active) => format!("{place}: being received"),
        (None, _) => format!("{place}: empty"),
    }
}

fn confidence(support: f32) -> (Color32, &'static str) {
    let share = support.clamp(0.0, 1.0);
    if share >= 0.7 {
        (TEAL, "strong")
    } else if share >= 0.45 {
        (AMBER, "ok")
    } else {
        (RED, "weak")
    }
}

#[derive(Clone, Copy)]
struct Anim {
    /// 1 when the digit changed, easing to 0.
    pop: f32,
    /// 1 when the digit turned final, easing to 0.
    ring: f32,
    breathe: f32,
    blink: bool,
}

/// A digit centred on `centre`: vertically by the font fix, horizontally by the ink of this glyph, so the narrow
/// "1" and the wide "0" both sit in the middle of their box.
pub(super) fn paint_digit(
    painter: &egui::Painter,
    centre: Pos2,
    ch: char,
    size: f32,
    color: Color32,
) {
    let galley = painter.layout_no_wrap(ch.to_string(), theme::digits(size), color);
    let ink_x = galley.mesh_bounds.center().x;
    let at = Pos2::new(centre.x - ink_x, centre.y - galley.size().y / 2.0);
    painter.galley(at, galley, color);
}

fn outline(painter: &egui::Painter, rect: Rect, stroke: Stroke) {
    paint_frame(painter, rect, Color32::TRANSPARENT, stroke, CUT * 0.5);
}

fn paint_slot(painter: &egui::Painter, rect: Rect, slot: &Slot, live: &str, anim: Anim) {
    let phase = slot.phase;
    let cut = CUT * 0.5;
    let (border, fill) = match phase {
        Phase::Empty => (STROKE, alpha(SURFACE_HI, 0.55)),
        Phase::Active => (alpha(AMBER, 0.45 + 0.5 * anim.breathe), SURFACE_HI),
        Phase::Provisional => (alpha(AMBER, 0.28), SURFACE_HI),
        Phase::Final => (alpha(TEAL, 0.38), SURFACE_HI),
    };
    if phase == Phase::Active {
        let glow = alpha(AMBER, 0.05 + 0.07 * anim.breathe);
        paint_frame(painter, rect.expand(4.0), glow, Stroke::NONE, cut);
    }
    if anim.pop > 0.0 {
        let tint = if phase == Phase::Final { TEAL } else { AMBER };
        let glow = alpha(tint, 0.16 * anim.pop);
        paint_frame(
            painter,
            rect.expand(3.0 * anim.pop),
            glow,
            Stroke::NONE,
            cut,
        );
    }
    paint_frame(painter, rect, fill, Stroke::new(HAIRLINE, border), cut);
    if anim.ring > 0.0 {
        let ring = Stroke::new(2.0 * anim.ring + 0.5, alpha(TEAL, 0.9 * anim.ring));
        outline(painter, rect.expand(2.0 + 14.0 * (1.0 - anim.ring)), ring);
    }

    let with_symbols = rect.height() >= SYMBOLS_FROM;
    let short = rect.height() < SHORT_BELOW;
    let conf = if short { CONF_H_SHORT } else { CONF_H };
    let below = conf + if with_symbols { SYMBOLS_H } else { 0.0 };
    let zone = Rect::from_min_size(
        rect.min,
        Vec2::new(rect.width(), (rect.height() - below).max(0.0)),
    );
    let centre = zone.center();
    let font = (zone.height() * 1.05).min(rect.width() * 0.8);
    match (slot.ch, phase) {
        (Some(ch), _) => {
            let unreadable = ch == '?';
            let color = match (phase, unreadable) {
                (_, true) => RED,
                (Phase::Final, _) => TEXT,
                _ => alpha(AMBER, 0.6),
            };
            let scale = 1.0 + 0.16 * anim.pop;
            paint_digit(painter, centre, ch, font * scale, color);
            if with_symbols && !unreadable {
                let pattern = pattern_of(ch);
                let fit = (rect.width() - 8.0) / symbols_width(pattern);
                let at = Pos2::new(centre.x, zone.bottom() + SYMBOLS_H / 2.0);
                paint_symbols(painter, at, pattern, 0.85_f32.min(fit), alpha(DIM, 0.8));
            }
            if phase == Phase::Final {
                paint_confidence(painter, rect, slot.support, short);
            }
        }
        (None, Phase::Active) if live.is_empty() => {
            if anim.blink {
                let bar = Vec2::new(2.0, zone.height() * 0.5);
                painter.rect_filled(Rect::from_center_size(centre, bar), 1.0, AMBER);
            }
        }
        (None, Phase::Active) => {
            let fit = (rect.width() - 8.0) / symbols_width(live);
            paint_symbols(painter, centre, live, 1.5_f32.min(fit), AMBER);
        }
        (None, _) => {
            painter.add(Shape::circle_filled(centre, 2.0, FAINT));
        }
    }
}

/// How much of the expected beep pattern was heard: a bar and a word, so the cue is not only a colour.
fn paint_confidence(painter: &egui::Painter, rect: Rect, support: f32, short: bool) {
    let (color, word) = confidence(support);
    let (bar_h, inset, word_h) = if short {
        (2.0, 3.5, 5.0)
    } else {
        (3.0, 7.0, 7.0)
    };
    let bar = Rect::from_min_size(
        Pos2::new(rect.left() + 6.0, rect.bottom() - inset),
        Vec2::new(rect.width() - 12.0, bar_h),
    );
    painter.rect_filled(bar, 1.5, alpha(RULE, 0.6));
    let fill = Vec2::new(bar.width() * support.clamp(0.0, 1.0), bar.height());
    painter.rect_filled(Rect::from_min_size(bar.min, fill), 1.5, color);
    painter.text(
        Pos2::new(rect.center().x, bar.top() - word_h),
        Align2::CENTER_CENTER,
        word,
        theme::text(if short { 9.5 } else { 10.5 }),
        DIM,
    );
}
