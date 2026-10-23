use egui::{Painter, Pos2, Rect, Shape, Stroke, vec2};

use crate::theme::HAIRLINE;

/// Corners of `rect` with the top-left and bottom-right corners cut diagonally by `cut_tl` and `cut_br`.
/// Clockwise from the top edge, 6 vertices (fewer when a cut is 0).
pub(crate) fn chamfer_points(rect: Rect, cut_tl: f32, cut_br: f32) -> Vec<Pos2> {
    let (l, t, r, b) = (rect.left(), rect.top(), rect.right(), rect.bottom());
    let mut pts = Vec::with_capacity(6);
    if cut_tl > 0.0 {
        pts.push(Pos2::new(l + cut_tl, t));
    } else {
        pts.push(Pos2::new(l, t));
    }
    pts.push(Pos2::new(r, t));
    if cut_br > 0.0 {
        pts.push(Pos2::new(r, b - cut_br));
        pts.push(Pos2::new(r - cut_br, b));
    } else {
        pts.push(Pos2::new(r, b));
    }
    pts.push(Pos2::new(l, b));
    if cut_tl > 0.0 {
        pts.push(Pos2::new(l, t + cut_tl));
    }
    pts
}

/// Filled chamfered panel (top-left and bottom-right cut by `cut`) with an outline.
pub(crate) fn paint_frame(
    painter: &Painter,
    rect: Rect,
    fill: egui::Color32,
    stroke: Stroke,
    cut: f32,
) {
    // egui fills only convex paths correctly. A rect with two opposite diagonal cuts is convex,
    // so `convex_polygon` is valid here.
    painter.add(Shape::convex_polygon(
        chamfer_points(rect, cut, cut),
        fill,
        stroke,
    ));
}

/// Four corner brackets with arms of `len`.
pub(crate) fn paint_brackets(painter: &Painter, rect: Rect, len: f32, stroke: Stroke) {
    for (corner, dx, dy) in [
        (rect.left_top(), 1.0, 1.0),
        (rect.right_top(), -1.0, 1.0),
        (rect.right_bottom(), -1.0, -1.0),
        (rect.left_bottom(), 1.0, -1.0),
    ] {
        painter.add(Shape::line(
            vec![
                corner + vec2(dx * len, 0.0),
                corner,
                corner + vec2(0.0, dy * len),
            ],
            stroke,
        ));
    }
}

/// Thin line from `from` to `to` ending in a small diamond at `to`.
pub(crate) fn paint_leader(painter: &Painter, from: Pos2, to: Pos2, stroke: Stroke) {
    painter.line_segment([from, to], stroke);
    let d = 2.5 * stroke.width.max(HAIRLINE);
    painter.add(Shape::convex_polygon(
        vec![
            to + vec2(0.0, -d),
            to + vec2(d, 0.0),
            to + vec2(0.0, d),
            to + vec2(-d, 0.0),
        ],
        stroke.color,
        Stroke::NONE,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cross(a: Pos2, b: Pos2, c: Pos2) -> f32 {
        (b - a).x * (c - b).y - (b - a).y * (c - b).x
    }

    #[test]
    fn chamfer_has_six_convex_vertices_inside_the_rect() {
        let rect = Rect::from_min_max(Pos2::new(10.0, 20.0), Pos2::new(110.0, 70.0));
        let p = chamfer_points(rect, 8.0, 8.0);
        assert_eq!(p.len(), 6);
        let signs: Vec<f32> = (0..6)
            .map(|i| cross(p[i], p[(i + 1) % 6], p[(i + 2) % 6]))
            .collect();
        assert!(signs.iter().all(|s| *s > 0.0), "{signs:?}");
        assert_eq!(Rect::from_points(&p), rect);
        assert!(!p.contains(&rect.left_top()) && !p.contains(&rect.right_bottom()));
    }

    #[test]
    fn zero_cuts_give_the_plain_rect() {
        let rect = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(40.0, 20.0));
        assert_eq!(chamfer_points(rect, 0.0, 0.0).len(), 4);
        assert_eq!(chamfer_points(rect, 6.0, 0.0).len(), 5);
    }
}
