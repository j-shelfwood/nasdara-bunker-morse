//! The bottom bar: a label row, the Morse ribbon over the tone trace, and the ten digit slots.
//!
//! The ribbon and the trace sit on the left, the slots on the right.

mod digits;
mod level;
mod ribbon;
pub(super) mod view;

use egui::{Pos2, Rect, Role, Sense, Stroke, Ui, Vec2, WidgetInfo};

use crate::engine::Shared;
use crate::theme::{self, AMBER, BRACKET, CUT, HAIRLINE, STROKE, SURFACE, TEXT, alpha};
use crate::ui::UiState;
use crate::ui::hud::{paint_brackets, paint_frame};
use ribbon::Axis;

const LABEL_H: f32 = 14.0;
const LEFT_SHARE: f32 = 0.55;
const GAP: f32 = 16.0;

pub(super) fn show(ui: &mut Ui, st: &mut UiState, s: &Shared) {
    let v = view::view(s);
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    let painter = ui.painter_at(rect.expand(8.0));
    let frame = rect.shrink(1.0);
    paint_frame(&painter, frame, SURFACE, Stroke::new(HAIRLINE, STROKE), CUT);
    paint_brackets(
        &painter,
        frame.shrink(3.0),
        BRACKET * 0.7,
        Stroke::new(HAIRLINE, alpha(AMBER, 0.55)),
    );

    let inner = frame.shrink2(Vec2::new(18.0, 6.0));
    let label = Rect::from_min_size(inner.min, Vec2::new(inner.width(), LABEL_H));
    headline(ui, &painter, label, &v);
    let body = Rect::from_min_max(Pos2::new(inner.left(), label.bottom() + 1.0), inner.max);

    let still = st.settings.reduce_motion;
    let left = Rect::from_min_size(
        body.min,
        Vec2::new((body.width() * LEFT_SHARE).round(), body.height()),
    );
    let right = Rect::from_min_max(Pos2::new(left.right() + GAP, body.top()), body.max);
    let ribbon_h = ((left.height() - 6.0) * 0.58).round();
    let ribbon = Rect::from_min_size(left.min, Vec2::new(left.width(), ribbon_h));
    let trace = Rect::from_min_max(Pos2::new(left.left(), ribbon.bottom() + 6.0), left.max);
    let axis = Axis::new(ribbon, s.now());
    ribbon::paint(&painter, s, ribbon, axis, still);
    level::paint(&painter, s, trace, axis);
    describe_ribbon(ui, ribbon, &v);
    digits::show(ui, st, &v, right);
}

/// The small status line at the top of the bar; the hint sentence is its hover text.
fn headline(ui: &mut Ui, painter: &egui::Painter, rect: Rect, v: &view::View) {
    let galley = painter.layout_no_wrap(v.headline.clone(), theme::bold(12.5), TEXT);
    let size = galley.size();
    let at = Pos2::new(rect.left(), rect.center().y - size.y / 2.0);
    painter.galley(at, galley, TEXT);
    let hover = Rect::from_min_size(rect.min, Vec2::new(size.x.max(40.0), rect.height()));
    let response = ui.interact(hover, ui.make_persistent_id("bar-headline"), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Status, true, v.headline.clone()));
    response.on_hover_text(v.hint.clone());
}

fn describe_ribbon(ui: &mut Ui, rect: Rect, v: &view::View) {
    let heard = v
        .live_symbols
        .chars()
        .map(|c| if c == '-' { "dash" } else { "dot" })
        .collect::<Vec<_>>()
        .join(", ");
    let text = if heard.is_empty() {
        "Morse ribbon of the last 24 seconds".to_string()
    } else {
        format!("Morse ribbon of the last 24 seconds. Heard of this digit: {heard}")
    };
    let response = ui.interact(
        rect,
        ui.make_persistent_id(("ribbon", rect.min.x as i32)),
        Sense::hover(),
    );
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, text.clone()));
}
