use egui::{Color32, Pos2, Rect, Response, Role, Sense, Ui, Vec2, WidgetInfo};

use super::focus_ring;
use crate::theme::{self, AMBER, RULE, STROKE, TEXT};

/// A switch with its label to the right. `still` skips the slide of the knob.
pub fn toggle(ui: &mut Ui, value: &mut bool, label: &str, still: bool) -> Response {
    const TRACK: Vec2 = Vec2::new(34.0, 18.0);
    let galley = ui
        .painter()
        .layout_no_wrap(label.into(), theme::text(13.0), TEXT);
    let size = Vec2::new(TRACK.x + 10.0 + galley.size().x, 28.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    let on = *value;
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id, on, if still { 0.0 } else { 0.12 });
    let track = Rect::from_min_size(rect.left_center() - Vec2::new(0.0, TRACK.y / 2.0), TRACK);
    let p = ui.painter();
    p.rect_filled(
        track,
        TRACK.y / 2.0,
        theme::mix(if response.hovered() { RULE } else { STROKE }, AMBER, t),
    );
    let knob_x = track.left() + TRACK.y / 2.0 + t * (TRACK.x - TRACK.y);
    p.circle_filled(
        Pos2::new(knob_x, track.center().y),
        TRACK.y / 2.0 - 3.0,
        theme::mix(TEXT, Color32::from_rgb(0x1a, 0x12, 0x02), t),
    );
    p.galley(
        Pos2::new(
            track.right() + 10.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        TEXT,
    );
    focus_ring(p, &response, 9.0);
    response.widget_info(|| WidgetInfo::selected(Role::Switch, true, on, label));
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
