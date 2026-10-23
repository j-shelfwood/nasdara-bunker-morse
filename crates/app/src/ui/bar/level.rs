//! Tone strength over time under the ribbon, on the same time axis. The dashed line is the level a tone counts at.

use egui::epaint::{Mesh, Vertex};
use egui::{Align2, Pos2, Rect, Shape, Stroke};

use super::ribbon::{Axis, WINDOW_S};
use crate::engine::Shared;
use crate::theme::{self, AMBER, CUT, DIM, FAINT, HAIRLINE, STROKE, SURFACE_HI, alpha, mix};
use crate::ui::hud::paint_frame;

const DB_LOW: f32 = -5.0;
const DB_HIGH: f32 = 35.0;
const ON_DB: f32 = 15.0;

pub fn paint(painter: &egui::Painter, s: &Shared, rect: Rect, axis: Axis) {
    paint_frame(
        painter,
        rect,
        alpha(SURFACE_HI, 0.55),
        Stroke::new(HAIRLINE, STROKE),
        CUT * 0.6,
    );
    let painter = &painter.with_clip_rect(rect);
    let base = rect.bottom() - 4.0;
    let y_of = |db: f32| {
        let share = (db.clamp(DB_LOW, DB_HIGH) - DB_LOW) / (DB_HIGH - DB_LOW);
        base - share * (rect.height() - 16.0)
    };
    let threshold = y_of(ON_DB);
    painter.add(Shape::dashed_line(
        &[
            Pos2::new(rect.left() + 8.0, threshold),
            Pos2::new(rect.right() - 8.0, threshold),
        ],
        Stroke::new(HAIRLINE, alpha(AMBER, 0.28)),
        4.0,
        4.0,
    ));

    // seconds ago, at the 5 s ticks
    let first = ((axis.now - WINDOW_S) / 5.0).ceil() as i64;
    let last = (axis.now / 5.0).floor() as i64;
    for k in first..=last {
        let x = axis.x_of(k as f64 * 5.0);
        let ago = (axis.now - k as f64 * 5.0).round() as i64;
        if ago > 0 && x > rect.left() + 24.0 && x < rect.right() {
            painter.text(
                Pos2::new(x + 3.0, rect.bottom() - 8.0),
                Align2::LEFT_CENTER,
                format!("{ago} s"),
                theme::text(10.0),
                alpha(FAINT, axis.fade(x)),
            );
        }
    }

    // peak per pixel column
    let cols = rect.width().ceil() as usize + 1;
    let mut peaks: Vec<Option<f32>> = vec![None; cols];
    for &(t, db) in s.frames.iter().rev() {
        if t < axis.now - WINDOW_S {
            break;
        }
        let i = (axis.x_of(t) - rect.left()).floor();
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
        let x = rect.left() + i as f32;
        let y = y_of(db);
        let fadein = axis.fade(x);
        let hot = db >= ON_DB;
        let top = alpha(if hot { AMBER } else { mix(AMBER, DIM, 0.6) }, 0.5 * fadein);
        let bottom = alpha(AMBER, 0.02 * fadein);
        let first = mesh.vertices.len() as u32;
        for (pos, color) in [(Pos2::new(x, y), top), (Pos2::new(x, base), bottom)] {
            mesh.vertices.push(Vertex {
                pos,
                uv: egui::epaint::WHITE_UV,
                color,
            });
        }
        if have_previous {
            mesh.indices.extend_from_slice(&[
                first - 2,
                first - 1,
                first,
                first - 1,
                first,
                first + 1,
            ]);
        }
        have_previous = true;
        line.push(Pos2::new(x, y));
    }
    painter.add(Shape::mesh(mesh));
    if line.len() > 1 {
        painter.add(Shape::line(line, Stroke::new(1.3, alpha(AMBER, 0.85))));
    }
    painter.text(
        Pos2::new(rect.left() + 10.0, rect.top() + 9.0),
        Align2::LEFT_CENTER,
        "TONE STRENGTH",
        theme::bold(11.0),
        FAINT,
    );
}
