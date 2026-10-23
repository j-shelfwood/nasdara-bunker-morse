//! The app mark: a dot, a dash and a dot (Morse for R, as in radio) on a dark tile. Painted with the same shapes in the
//! window header and rasterised for the window icon.

use egui::{Pos2, Rect, Sense, Ui, Vec2};

use crate::theme::{AMBER, SURFACE_HI, alpha};

pub fn paint(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, size * 0.26, SURFACE_HI);
    p.rect_stroke(
        rect,
        size * 0.26,
        egui::Stroke::new(1.0, alpha(AMBER, 0.55)),
        egui::StrokeKind::Inside,
    );
    let h = size * 0.14;
    let y = rect.center().y;
    let dot = |x: f32| p.circle_filled(Pos2::new(x, y), h / 2.0 + 0.2, AMBER);
    dot(rect.left() + size * 0.22);
    p.rect_filled(
        Rect::from_center_size(Pos2::new(rect.center().x, y), Vec2::new(size * 0.28, h)),
        h / 2.0,
        AMBER,
    );
    dot(rect.right() - size * 0.22);
}

/// RGBA pixels of the mark at `px` by `px`, anti-aliased by 4x supersampling.
pub fn rasterize(px: usize) -> Vec<u8> {
    let mut out = vec![0u8; px * px * 4];
    let s = px as f32;
    let (bg, fg) = ([0x19u8, 0x1e, 0x26], [0xffu8, 0xb0, 0x20]);
    let h = s * 0.14;
    let cy = s * 0.5;
    let inside_tile = |x: f32, y: f32| {
        let r = s * 0.26;
        let (dx, dy) = (
            (x - s / 2.0).abs() - (s / 2.0 - r),
            (y - s / 2.0).abs() - (s / 2.0 - r),
        );
        dx.max(0.0).hypot(dy.max(0.0)) <= r
    };
    let capsule = |x: f32, y: f32, cx: f32, w: f32| {
        let half = (w - h).max(0.0) / 2.0;
        let dx = ((x - cx).abs() - half).max(0.0);
        dx.hypot(y - cy) <= h / 2.0
    };
    for py in 0..px {
        for pxl in 0..px {
            let (mut tile, mut mark) = (0.0, 0.0);
            for sy in 0..4 {
                for sx in 0..4 {
                    let x = pxl as f32 + (sx as f32 + 0.5) / 4.0;
                    let y = py as f32 + (sy as f32 + 0.5) / 4.0;
                    if inside_tile(x, y) {
                        tile += 1.0;
                        if capsule(x, y, s * 0.22, h)
                            || capsule(x, y, s * 0.5, s * 0.28)
                            || capsule(x, y, s * 0.78, h)
                        {
                            mark += 1.0;
                        }
                    }
                }
            }
            let i = (py * px + pxl) * 4;
            let cover = tile / 16.0;
            let m = if tile > 0.0 { mark / tile } else { 0.0 };
            for c in 0..3 {
                out[i + c] = (bg[c] as f32 * (1.0 - m) + fg[c] as f32 * m) as u8;
            }
            out[i + 3] = (cover * 255.0) as u8;
        }
    }
    out
}
