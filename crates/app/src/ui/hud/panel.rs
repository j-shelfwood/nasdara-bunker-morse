use egui::{
    Align, Color32, InnerResponse, Layout, Margin, Pos2, Rect, Sense, Shape, Stroke, Ui, UiBuilder,
    Vec2, vec2,
};

use super::chamfer_points;
use crate::theme::{AMBER, BRACKET, CUT, HAIRLINE, STROKE, SURFACE, alpha};

/// Look of one chamfered panel. Defaults: surface fill, 1 px `STROKE` outline, amber corner brackets.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Panel {
    pub fill: Color32,
    pub stroke: Stroke,
    /// Colour of the brackets on the two uncut corners; `None` draws none.
    pub brackets: Option<Color32>,
    pub margin: Margin,
    /// Outer width; `None` takes all the width on offer.
    pub width: Option<f32>,
    /// Outer height the frame keeps when the content is shorter.
    pub min_height: Option<f32>,
}

impl Default for Panel {
    fn default() -> Self {
        Panel {
            fill: SURFACE,
            stroke: Stroke::new(HAIRLINE, STROKE),
            brackets: Some(alpha(AMBER, 0.75)),
            margin: Margin::same(12),
            width: None,
            min_height: None,
        }
    }
}

/// Corners the chamfer leaves intact (top-right, bottom-left) with the direction of their arms. Brackets sit
/// there so they never float in a cut.
pub(crate) fn bracket_corners(rect: Rect) -> [(Pos2, Vec2, Vec2); 2] {
    [
        (rect.right_top(), vec2(-1.0, 0.0), vec2(0.0, 1.0)),
        (rect.left_bottom(), vec2(1.0, 0.0), vec2(0.0, -1.0)),
    ]
}

/// Content inside a chamfered frame with optional corner brackets. The frame is sized after the content and
/// painted underneath it; the panel is as wide as the space on offer and `inner.width()` of content fits inside.
pub(crate) fn panel<R>(
    ui: &mut Ui,
    look: Panel,
    add: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    let slot = ui.painter().add(Shape::Noop);
    let outer = ui.available_rect_before_wrap();
    let (l, r) = (f32::from(look.margin.left), f32::from(look.margin.right));
    let (t, b) = (f32::from(look.margin.top), f32::from(look.margin.bottom));
    let right = outer.min.x + look.width.unwrap_or(outer.width());
    let inner = Rect::from_min_max(outer.min + vec2(l, t), Pos2::new(right - r, outer.max.y));
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::top_down(Align::Min)),
    );
    child.set_width(inner.width());
    let value = add(&mut child);
    let bottom = (child.min_rect().bottom() + b).max(outer.min.y + look.min_height.unwrap_or(0.0));
    let rect = Rect::from_min_max(outer.min, Pos2::new(right, bottom));
    let response = ui.allocate_rect(rect, Sense::hover());
    let painter = ui.painter();
    painter.set(
        slot,
        Shape::convex_polygon(chamfer_points(rect, CUT, CUT), look.fill, look.stroke),
    );
    if let Some(color) = look.brackets {
        let stroke = Stroke::new(HAIRLINE * 1.5, color);
        for (corner, along, down) in bracket_corners(rect) {
            painter.add(Shape::line(
                vec![corner + along * BRACKET, corner, corner + down * BRACKET],
                stroke,
            ));
        }
    }
    InnerResponse::new(value, response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brackets_sit_on_the_uncut_corners() {
        let rect = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 50.0));
        let cut = chamfer_points(rect, CUT, CUT);
        for (corner, along, down) in bracket_corners(rect) {
            assert!(cut.contains(&corner), "{corner:?} is a frame vertex");
            // both arms head into the rect
            assert!(rect.contains(corner + along * BRACKET + down * 0.5));
            assert!(rect.contains(corner + down * BRACKET + along * 0.5));
        }
    }

    #[test]
    fn panel_wraps_its_content_and_spans_the_offered_width() {
        let ctx = egui::Context::default();
        let mut rect = Rect::NOTHING;
        ctx.begin_pass(egui::RawInput::default());
        egui::Area::new(egui::Id::unique("panel-test")).show(&ctx, |ui| {
            let bounds = Rect::from_min_size(Pos2::ZERO, vec2(300.0, 500.0));
            ui.scope_builder(UiBuilder::new().max_rect(bounds), |ui| {
                let r = panel(ui, Panel::default(), |ui| {
                    ui.allocate_space(vec2(10.0, 40.0));
                });
                rect = r.response.rect;
            });
        });
        // a dropped output with unapplied texture deltas panics in debug builds
        ctx.end_pass().textures_delta.clear();
        assert_eq!(rect.height(), 40.0 + 24.0);
        assert!((rect.width() - 300.0).abs() < 0.5, "{}", rect.width());
    }
}
