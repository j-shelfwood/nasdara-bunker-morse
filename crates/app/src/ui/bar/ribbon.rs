//! The Morse ribbon: beeps appear at the right edge and slide left. The decoder's reading of each digit is
//! bracketed underneath, and a beep it expected but did not hear is drawn as an empty outline.

use egui::{Align2, Color32, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};
use nasdara_decoder::grid::SHIPPED_TIMING;
use nasdara_decoder::morse::pattern_of;

use super::digits::paint_digit;
use super::view::DASH_FROM_S;
use crate::engine::Shared;
use crate::theme::{
    self, AMBER, CUT, FAINT, HAIRLINE, RED, STROKE, SURFACE_HI, TEAL, TEXT, alpha, mix,
};
use crate::ui::hud::paint_frame;

pub const WINDOW_S: f64 = 24.0;
/// Space kept right of "now" so a starting beep is visible.
const LEAD_PX: f32 = 28.0;
const DOT_W: f32 = 11.0;
const DASH_W: f32 = 32.0;
/// Width over which beeps fade in and out at the left edge.
const FADE_PX: f32 = 60.0;

/// Maps stream time to x for a ribbon and the trace under it, so both line up.
#[derive(Clone, Copy)]
pub struct Axis {
    pub left: f32,
    pub right: f32,
    pub now: f64,
}

impl Axis {
    pub fn new(rect: Rect, now: f64) -> Axis {
        Axis {
            left: rect.left(),
            right: rect.right(),
            now,
        }
    }

    fn px_per_s(&self) -> f32 {
        (self.right - self.left - LEAD_PX) / WINDOW_S as f32
    }

    pub fn x_of(&self, t: f64) -> f32 {
        self.right - LEAD_PX - ((self.now - t) as f32) * self.px_per_s()
    }

    /// 0 at the left edge, 1 from `FADE_PX` in.
    pub fn fade(&self, x: f32) -> f32 {
        ((x - self.left) / FADE_PX).clamp(0.0, 1.0)
    }

    pub fn now_x(&self) -> f32 {
        self.right - LEAD_PX
    }
}

/// The decoder's reading as (digit, start, end of window): the preview while receiving, else the last result.
fn readings(s: &Shared) -> Vec<(char, f64, f64)> {
    match (&s.preview, &s.finished) {
        (Some(p), _) => p
            .digits
            .iter()
            .map(|d| (d.ch, d.start, d.window_end))
            .collect(),
        (None, Some(f)) if !s.receiving() => f
            .digits
            .iter()
            .zip(&f.starts)
            .enumerate()
            .map(|(i, (ch, &start))| {
                let end = f
                    .starts
                    .get(i + 1)
                    .map_or(start + 6.0, |n| n - SHIPPED_TIMING.digit_gap);
                (*ch, start, end)
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Paints the ribbon into `rect`.
pub fn paint(painter: &egui::Painter, s: &Shared, rect: Rect, axis: Axis, still: bool) {
    let now = axis.now;
    let beep_h = (rect.height() * 0.18).clamp(7.0, 11.0);
    let mid = rect.top() + rect.height() * 0.36;
    paint_frame(
        painter,
        rect,
        alpha(SURFACE_HI, 0.55),
        Stroke::new(HAIRLINE, STROKE),
        CUT * 0.6,
    );
    let painter = &painter.with_clip_rect(rect);

    let first = ((now - WINDOW_S) / 5.0).ceil() as i64;
    let last = (now / 5.0).floor() as i64;
    for k in first..=last {
        let x = axis.x_of(k as f64 * 5.0);
        if x < rect.left() + 2.0 || x > rect.right() {
            continue;
        }
        let ends = [
            Pos2::new(x, rect.top() + 4.0),
            Pos2::new(x, rect.bottom() - 4.0),
        ];
        painter.line_segment(
            ends,
            Stroke::new(HAIRLINE, alpha(STROKE, 0.5 * axis.fade(x))),
        );
    }
    painter.line_segment(
        [
            Pos2::new(rect.left() + 10.0, mid),
            Pos2::new(rect.right() - 10.0, mid),
        ],
        Stroke::new(HAIRLINE, alpha(STROKE, 0.8)),
    );

    for (ch, start, end) in readings(s) {
        let (x0, x1) = (axis.x_of(start), axis.x_of(end));
        if x1 < rect.left() || x0 > rect.right() {
            continue;
        }
        let f = axis.fade(x0.max(x1.min(x0 + 1.0)));
        paint_reading(painter, rect, ch, (x0, x1), f);
        if ch != '?' {
            paint_ghosts(painter, s, ch, start, (mid, beep_h), axis, f);
        }
    }

    for b in s.beeps.iter() {
        let len = b.length(now);
        let x = axis.x_of(b.start);
        let w = DOT_W + ((len as f32 - 0.09) / (0.48 - 0.09)).clamp(0.0, 1.0) * (DASH_W - DOT_W);
        if x + w < rect.left() || x > rect.right() {
            continue;
        }
        let f = axis.fade(x);
        let r = Rect::from_min_size(Pos2::new(x, mid - beep_h / 2.0), Vec2::new(w, beep_h));
        let on = b.end.is_none();
        let recent = b
            .end
            .map_or(0.0, |e| (1.0 - ((now - e) / 0.5) as f32).clamp(0.0, 1.0));
        let glow = if still {
            0.0
        } else if on {
            1.0
        } else {
            recent
        };
        if glow > 0.0 {
            painter.rect_filled(
                r.expand(5.0 * glow + 1.0),
                beep_h,
                alpha(AMBER, 0.13 * glow * f),
            );
        }
        let col = if len > DASH_FROM_S {
            AMBER
        } else {
            mix(AMBER, Color32::WHITE, 0.45)
        };
        painter.rect_filled(
            r,
            beep_h / 2.0,
            alpha(if on { Color32::WHITE } else { col }, f),
        );
    }

    // the "now" marker breathes on a plain sine so the 33 ms listening repaint tier stays in charge
    let phase = if still {
        0.0
    } else {
        painter.ctx().input(|i| i.time) as f32
    };
    let pulse = (0.5 + 0.5 * (phase * 3.0).sin()) * 0.3 + 0.5;
    painter.line_segment(
        [
            Pos2::new(axis.now_x(), rect.top() + 4.0),
            Pos2::new(axis.now_x(), rect.bottom() - 4.0),
        ],
        Stroke::new(1.5, alpha(AMBER, 0.55 * pulse)),
    );
    painter.text(
        Pos2::new(rect.left() + 10.0, rect.top() + 9.0),
        Align2::LEFT_CENTER,
        "MORSE",
        theme::bold(11.0),
        FAINT,
    );
}

/// Bracket under the digit's window with the digit centred beneath it.
fn paint_reading(painter: &egui::Painter, rect: Rect, ch: char, (x0, x1): (f32, f32), f: f32) {
    let h = rect.height();
    let y = rect.top() + h * 0.66;
    let size = (h * 0.26).clamp(12.0, 19.0);
    let unreadable = ch == '?';
    let c = if unreadable { RED } else { TEAL };
    let bracket = [
        Pos2::new(x0.max(rect.left() + 6.0), y),
        Pos2::new(x1 - 4.0, y),
    ];
    if unreadable {
        // an unreadable digit gets a dashed bracket, not only another colour
        painter.add(Shape::dashed_line(
            &bracket,
            Stroke::new(1.5, alpha(c, 0.7 * f)),
            4.0,
            3.0,
        ));
    } else {
        painter.line_segment(bracket, Stroke::new(1.5, alpha(c, 0.45 * f)));
    }
    let at = Pos2::new((x0 + x1 - 4.0) / 2.0, rect.top() + h * 0.84);
    paint_digit(
        painter,
        at,
        ch,
        size,
        alpha(if unreadable { RED } else { TEXT }, f),
    );
}

/// An empty outline where the digit's pattern expected a beep that was not heard.
fn paint_ghosts(
    painter: &egui::Painter,
    s: &Shared,
    ch: char,
    start: f64,
    (mid, beep_h): (f32, f32),
    axis: Axis,
    f: f32,
) {
    let tm = &SHIPPED_TIMING;
    let mut at = start;
    for sym in pattern_of(ch).chars() {
        let dash = sym == '-';
        if !s.beeps.iter().any(|b| (b.start - at).abs() < 0.3) {
            let r = Rect::from_min_size(
                Pos2::new(axis.x_of(at), mid - beep_h / 2.0),
                Vec2::new(if dash { DASH_W } else { DOT_W }, beep_h),
            );
            if r.right() > axis.left {
                let stroke = Stroke::new(1.0, alpha(RED, 0.5 * f));
                painter.rect_stroke(r, beep_h / 2.0, stroke, StrokeKind::Inside);
            }
        }
        at += if dash {
            tm.dash_window + tm.symbol_gap
        } else {
            tm.dot_window + tm.symbol_gap
        };
    }
}
