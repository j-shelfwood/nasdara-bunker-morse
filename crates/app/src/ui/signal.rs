//! The live signal: a scrolling Morse ribbon over a tone-level trace. Beeps appear at the right edge and slide left;
//! the decoder's reading of each digit is bracketed underneath, and a beep the decoder expected but did not hear
//! is drawn as an empty outline.

use egui::epaint::{Mesh, Vertex};
use egui::{Align2, Color32, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2};
use nasdara_decoder::grid::SHIPPED_TIMING;
use nasdara_decoder::morse::pattern_of;

use super::slots::DASH_FROM_S;
use crate::engine::Shared;
use crate::theme::{self, AMBER, DIM, FAINT, STROKE, SURFACE_HI, TEAL, TEXT, alpha};

const WINDOW_S: f64 = 24.0;
/// Space kept right of "now" so a starting beep is visible.
const LEAD_PX: f32 = 28.0;
const DOT_W: f32 = 11.0;
const DASH_W: f32 = 32.0;
const BEEP_H: f32 = 11.0;
const DB_LOW: f32 = -5.0;
const DB_HIGH: f32 = 35.0;
const ON_DB: f32 = 15.0;

/// How much room the signal panel gets: the whole window height decides, so a short window keeps the
/// result and the cards in view and gives up the ribbon's height first.
#[derive(Clone, Copy)]
pub struct Size {
    ribbon: f32,
    /// Height of the tone strength trace; 0 hides it and the time axis.
    level: f32,
}

impl Size {
    pub fn for_window_height(h: f32) -> Size {
        if h >= 720.0 {
            Size {
                ribbon: 90.0,
                level: 48.0,
            }
        } else if h >= 600.0 {
            Size {
                ribbon: 72.0,
                level: 36.0,
            }
        } else {
            Size {
                ribbon: 56.0,
                level: 0.0,
            }
        }
    }

    pub fn is_compact(self) -> bool {
        self.level == 0.0
    }
}

pub fn show(ui: &mut Ui, s: &Shared, size: Size) {
    let width = ui.available_width();
    let ribbon_h = size.ribbon;
    let level_h = size.level;
    let axis_h = if level_h > 0.0 { 24.0 } else { 0.0 };
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(width, ribbon_h + level_h + axis_h),
        Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    let now = s.now();
    let t = ui.input(|i| i.time) as f32;

    let ribbon = Rect::from_min_size(rect.min, Vec2::new(width, ribbon_h));
    let level = Rect::from_min_size(
        Pos2::new(rect.left(), ribbon.bottom() + 6.0),
        Vec2::new(width, level_h),
    );
    let axis_y = level.bottom() + 12.0;
    let line_bottom = if level_h > 0.0 {
        level.bottom()
    } else {
        ribbon.bottom() - 4.0
    };

    let px_per_s = (width - LEAD_PX) / WINDOW_S as f32;
    let x_of = |tt: f64| rect.right() - LEAD_PX - ((now - tt) as f32) * px_per_s;

    painter.rect_filled(ribbon, 10.0, alpha(SURFACE_HI, 0.55));
    if level_h > 0.0 {
        painter.rect_filled(level, 10.0, alpha(SURFACE_HI, 0.55));
    }

    // fade at the left edge so beeps leave smoothly
    let fade = |x: f32| ((x - rect.left()) / 60.0).clamp(0.0, 1.0);

    // time ticks every 5 s
    let first = ((now - WINDOW_S) / 5.0).ceil() as i64;
    let last = (now / 5.0).floor() as i64;
    for k in first..=last {
        let x = x_of(k as f64 * 5.0);
        if x < rect.left() + 2.0 || x > rect.right() {
            continue;
        }
        painter.line_segment(
            [Pos2::new(x, ribbon.top() + 6.0), Pos2::new(x, line_bottom)],
            Stroke::new(1.0, alpha(STROKE, 0.5 * fade(x))),
        );
        let ago = (now - k as f64 * 5.0).round() as i64;
        if ago > 0 && level_h > 0.0 {
            painter.text(
                Pos2::new(x, axis_y),
                Align2::CENTER_CENTER,
                format!("{ago} s"),
                theme::text(11.0),
                alpha(FAINT, fade(x)),
            );
        }
    }

    let mid = ribbon.top() + ribbon_h * 0.38;
    painter.line_segment(
        [
            Pos2::new(ribbon.left() + 10.0, mid),
            Pos2::new(ribbon.right() - 10.0, mid),
        ],
        Stroke::new(1.0, alpha(STROKE, 0.8)),
    );

    // digit brackets and ghosts, from the decoder's current reading
    let digits: Vec<(char, f64, f64)> = match (&s.preview, &s.finished) {
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
                (
                    *ch,
                    start,
                    f.starts
                        .get(i + 1)
                        .map_or(start + 6.0, |n| n - SHIPPED_TIMING.digit_gap),
                )
            })
            .collect(),
        _ => Vec::new(),
    };
    let tm = &SHIPPED_TIMING;
    for &(ch, start, end) in &digits {
        let (x0, x1) = (x_of(start), x_of(end));
        if x1 < rect.left() || x0 > rect.right() {
            continue;
        }
        let f = fade(x0.max(x1.min(x0 + 1.0)));
        let y = mid + ribbon_h * 0.27;
        let (digit_size, digit_dy) = if ribbon_h >= 80.0 {
            (19.0, 13.0)
        } else {
            (15.0, 11.0)
        };
        let unreadable = ch == '?';
        let c = if unreadable { theme::RED } else { TEAL };
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
        painter.text(
            Pos2::new((x0 + x1 - 4.0) / 2.0, y + digit_dy),
            Align2::CENTER_CENTER,
            ch.to_string(),
            theme::digits(digit_size),
            alpha(if unreadable { theme::RED } else { TEXT }, f),
        );
        if !unreadable {
            let mut at = start;
            for sym in pattern_of(ch).chars() {
                let dash = sym == '-';
                let heard = s.beeps.iter().any(|b| (b.start - at).abs() < 0.3);
                if !heard {
                    let x = x_of(at);
                    let r = Rect::from_min_size(
                        Pos2::new(x, mid - BEEP_H / 2.0),
                        Vec2::new(if dash { DASH_W } else { DOT_W }, BEEP_H),
                    );
                    if r.right() > rect.left() {
                        painter.rect_stroke(
                            r,
                            BEEP_H / 2.0,
                            Stroke::new(1.0, alpha(theme::RED, 0.5 * f)),
                            StrokeKind::Inside,
                        );
                    }
                }
                at += if dash {
                    tm.dash_window + tm.symbol_gap
                } else {
                    tm.dot_window + tm.symbol_gap
                };
            }
        }
    }

    // beeps
    for b in s.beeps.iter() {
        let len = b.length(now);
        let x = x_of(b.start);
        let w = DOT_W + ((len as f32 - 0.09) / (0.48 - 0.09)).clamp(0.0, 1.0) * (DASH_W - DOT_W);
        if x + w < rect.left() || x > rect.right() {
            continue;
        }
        let f = fade(x);
        let r = Rect::from_min_size(Pos2::new(x, mid - BEEP_H / 2.0), Vec2::new(w, BEEP_H));
        let on = b.end.is_none();
        let recent = b
            .end
            .map_or(0.0, |e| (1.0 - ((now - e) / 0.5) as f32).clamp(0.0, 1.0));
        let glow = if on { 1.0 } else { recent };
        if glow > 0.0 {
            painter.rect_filled(
                r.expand(5.0 * glow + 1.0),
                BEEP_H,
                alpha(AMBER, 0.13 * glow * f),
            );
        }
        let col = if len > DASH_FROM_S {
            AMBER
        } else {
            theme::mix(AMBER, Color32::WHITE, 0.45)
        };
        painter.rect_filled(
            r,
            BEEP_H / 2.0,
            alpha(if on { Color32::WHITE } else { col }, f),
        );
    }

    // "now" marker
    let nx = rect.right() - LEAD_PX;
    let pulse = (0.5 + 0.5 * (t * 3.0).sin()) * 0.3 + 0.5;
    painter.line_segment(
        [
            Pos2::new(nx, ribbon.top() + 8.0),
            Pos2::new(nx, line_bottom),
        ],
        Stroke::new(1.5, alpha(AMBER, 0.55 * pulse)),
    );

    if level_h > 0.0 {
        paint_level(&painter, s, level, &x_of, now, rect.left());
        painter.text(
            Pos2::new(level.left() + 10.0, level.top() + 9.0),
            Align2::LEFT_CENTER,
            "TONE STRENGTH",
            theme::bold(11.0),
            FAINT,
        );
    }
    painter.text(
        Pos2::new(ribbon.left() + 10.0, ribbon.top() + 11.0),
        Align2::LEFT_CENTER,
        "MORSE",
        theme::bold(11.0),
        FAINT,
    );
    let _ = DIM;
}

fn paint_level(
    painter: &egui::Painter,
    s: &Shared,
    area: Rect,
    x_of: &dyn Fn(f64) -> f32,
    now: f64,
    left_edge: f32,
) {
    let y_of = |db: f32| {
        area.bottom()
            - 4.0
            - ((db.clamp(DB_LOW, DB_HIGH) - DB_LOW) / (DB_HIGH - DB_LOW)) * (area.height() - 22.0)
    };
    let threshold = y_of(ON_DB);
    painter.add(Shape::dashed_line(
        &[
            Pos2::new(area.left() + 8.0, threshold),
            Pos2::new(area.right() - 8.0, threshold),
        ],
        Stroke::new(1.0, alpha(AMBER, 0.28)),
        4.0,
        4.0,
    ));

    // peak per pixel column
    let cols = area.width().ceil() as usize + 1;
    let mut peaks: Vec<Option<f32>> = vec![None; cols];
    let horizon = now - WINDOW_S;
    for &(t, db) in s.frames.iter().rev() {
        if t < horizon {
            break;
        }
        let i = (x_of(t) - area.left()).floor();
        if i < 0.0 || i as usize >= cols {
            continue;
        }
        let slot = &mut peaks[i as usize];
        *slot = Some(slot.map_or(db, |v| v.max(db)));
    }
    let mut mesh = Mesh::default();
    let mut line: Vec<Pos2> = Vec::new();
    let mut have_previous = false;
    for (i, p) in peaks.iter().enumerate() {
        let Some(db) = *p else {
            have_previous = false;
            continue;
        };
        let x = area.left() + i as f32;
        let y = y_of(db);
        let fadein = ((x - left_edge) / 60.0).clamp(0.0, 1.0);
        let hot = db >= ON_DB;
        let top = alpha(
            if hot {
                AMBER
            } else {
                theme::mix(AMBER, DIM, 0.6)
            },
            0.5 * fadein,
        );
        let bottom = alpha(AMBER, 0.02 * fadein);
        let base = mesh.vertices.len() as u32;
        mesh.vertices.push(Vertex {
            pos: Pos2::new(x, y),
            uv: egui::epaint::WHITE_UV,
            color: top,
        });
        mesh.vertices.push(Vertex {
            pos: Pos2::new(x, area.bottom() - 4.0),
            uv: egui::epaint::WHITE_UV,
            color: bottom,
        });
        if have_previous {
            mesh.indices
                .extend_from_slice(&[base - 2, base - 1, base, base - 1, base, base + 1]);
        }
        have_previous = true;
        line.push(Pos2::new(x, y));
    }
    painter.add(Shape::mesh(mesh));
    if line.len() > 1 {
        painter.add(Shape::line(line, Stroke::new(1.3, alpha(AMBER, 0.85))));
    }
}
