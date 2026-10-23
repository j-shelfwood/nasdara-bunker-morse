//! The floating card over a pin: chamfered frame, brackets, a leader to the pin and the rows of `content`.

use egui::{Align2, Painter, Pos2, Rect, Stroke, Vec2, vec2};

use super::content::{Content, Kind, Part};
use super::place::attach;
use crate::theme::{self, AMBER, BG, CUT, FAINT, HAIRLINE, STROKE, SURFACE_HI, alpha};
use crate::ui::bunkers::{Reading, reading_color};
use crate::ui::hud::{paint_brackets, paint_frame, paint_leader};
use crate::ui::widgets::{paint_check, paint_question, spaced};

/// Measured card: where each part of each row starts, relative to the card's top-left.
pub(super) struct Laid {
    pub size: Vec2,
    k: f32,
    header: Vec<(f32, usize)>,
    rows: Vec<Vec<(f32, usize)>>,
}

fn font(kind: Kind, k: f32) -> egui::FontId {
    match kind {
        Kind::Caption => theme::bold(9.5 * k),
        Kind::Value => theme::digits(12.5 * k),
        Kind::Lock => theme::digits(14.5 * k),
        Kind::Note => theme::text(11.0 * k),
    }
}

fn shown(p: &Part) -> String {
    if p.kind == Kind::Caption {
        spaced(&p.text)
    } else {
        p.text.clone()
    }
}

const PAD: f32 = 9.0;
const HEADER_H: f32 = 22.0;
const ROW_H: f32 = 17.0;
/// Column where the value of a GRID or LOCK row starts.
const LABEL_W: f32 = 40.0;
const LOCK_ADVANCE: f32 = 14.0;
const MIN_W: f32 = 150.0;

/// Places the parts of a row left to right; returns each part's x and the row's end.
fn flow(p: &Painter, parts: &[Part], k: f32, align_value: bool) -> (Vec<(f32, usize)>, f32) {
    let mut x = PAD * k;
    let mut out = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        let w = if part.kind == Kind::Lock {
            LOCK_ADVANCE * k
        } else {
            p.layout_no_wrap(shown(part), font(part.kind, k), part.color)
                .size()
                .x
        };
        if align_value && i == 1 {
            x = x.max((PAD + LABEL_W) * k);
        }
        out.push((x, i));
        x += w;
        x += match (part.kind, parts.get(i + 1).map(|n| n.kind)) {
            (Kind::Caption, Some(Kind::Value)) => 5.0 * k,
            (Kind::Value, Some(Kind::Caption)) => 14.0 * k,
            (Kind::Caption, Some(Kind::Caption)) => 8.0 * k,
            _ => 0.0,
        };
    }
    (out, x)
}

pub(super) fn layout(p: &Painter, c: &Content, k: f32) -> Laid {
    let (header, mut width) = flow(p, &c.header, k, false);
    if c.reading.is_some() {
        width += 24.0 * k;
    }
    let mut rows = Vec::new();
    for r in &c.rows {
        let aligned = r
            .first()
            .is_some_and(|f| f.text == "GRID" || f.text == "LOCK");
        let (xs, end) = flow(p, r, k, aligned);
        width = width.max(end);
        rows.push(xs);
    }
    let w = (width + PAD * k).max(MIN_W * k);
    let h = (HEADER_H + ROW_H * c.rows.len() as f32 + PAD) * k;
    Laid {
        size: vec2(w.ceil(), h.ceil()),
        k,
        header,
        rows,
    }
}

fn paint_parts(p: &Painter, origin: Pos2, cy: f32, parts: &[Part], xs: &[(f32, usize)], k: f32) {
    for &(x, i) in xs {
        let part = &parts[i];
        let (px, align) = if part.kind == Kind::Lock {
            (x + LOCK_ADVANCE * k / 2.0, Align2::CENTER_CENTER)
        } else {
            (x, Align2::LEFT_CENTER)
        };
        p.text(
            Pos2::new(origin.x + px, cy),
            align,
            shown(part),
            font(part.kind, k),
            part.color,
        );
    }
}

/// The reading shape of the bunker list, small: check in a ring, `1x` badge or `?` ring.
fn paint_reading(p: &Painter, center: Pos2, r: Reading, k: f32) {
    let color = reading_color(r);
    match r {
        Reading::Confirmed => {
            p.circle_stroke(center, 7.0 * k, Stroke::new(1.5, color));
            paint_check(p, center, 7.0 * k, color, 1.7);
        }
        Reading::Once => {
            let b = Rect::from_center_size(center, Vec2::splat(15.0 * k));
            p.rect_stroke(
                b,
                4.0 * k,
                Stroke::new(1.5, color),
                egui::StrokeKind::Inside,
            );
            p.text(
                center,
                Align2::CENTER_CENTER,
                "1×",
                theme::bold(9.5 * k),
                color,
            );
        }
        Reading::Unclear => paint_question(p, center, 16.0 * k, color),
    }
}

/// Draws the card at `rect` (sized by `layout`) with its leader to `pin`. The caller sets the painter's opacity.
pub(super) fn paint(p: &Painter, laid: &Laid, c: &Content, rect: Rect, pin: Pos2) {
    let k = laid.k;
    let edge = if c.heard {
        alpha(AMBER, 0.55)
    } else {
        alpha(FAINT, 0.55)
    };
    let end = attach(rect, pin, CUT);
    paint_leader(p, pin, end, Stroke::new(HAIRLINE, edge));
    paint_frame(p, rect, BG, Stroke::new(HAIRLINE, edge), CUT);
    let bracket = if c.heard { AMBER } else { FAINT };
    paint_brackets(p, rect.expand(2.0), 7.0 * k, Stroke::new(1.5, bracket));
    // header band and rule
    let head_cy = rect.top() + HEADER_H * k / 2.0 + 1.0;
    let band = Rect::from_min_max(
        rect.left_top() + vec2(CUT, 0.0),
        Pos2::new(rect.right(), rect.top() + HEADER_H * k),
    );
    p.rect_filled(band, 0.0, alpha(SURFACE_HI, 0.8));
    p.line_segment(
        [
            Pos2::new(rect.left(), band.bottom()),
            Pos2::new(rect.right(), band.bottom()),
        ],
        Stroke::new(HAIRLINE, STROKE),
    );
    paint_parts(p, rect.min, head_cy, &c.header, &laid.header, k);
    if let Some(r) = c.reading {
        let at = Pos2::new(rect.right() - 16.0 * k, head_cy);
        paint_reading(p, at, r, k);
    }
    for (i, (row, xs)) in c.rows.iter().zip(&laid.rows).enumerate() {
        let cy = rect.top() + (HEADER_H + ROW_H * (i as f32 + 0.5)) * k;
        paint_parts(p, rect.min, cy, row, xs, k);
    }
}
