//! The right column of a wide window and, below 900 points, the drawer that holds the same content.

use egui::{Area, Id, Order, Rect, RichText, ScrollArea, Stroke, Ui, UiBuilder, Vec2};

use crate::engine::Shared;
use crate::theme::{self, DIM, STROKE, SURFACE};
use crate::ui::anim::{Motion, easing};
use crate::ui::layout::{self, COLUMN_W, Centre};
use crate::ui::widgets::{ButtonKind, button, caption};
use crate::ui::{UiState, bunkers, guide, latest};

const SLIDE_S: f32 = 180.0 / 1000.0;

fn heard(s: &Shared) -> usize {
    s.cards.iter().filter(|c| c.bunker.is_some()).count()
}

/// The result strip, then the bunkers list; the cheat sheet closes the scrolling list when `with_guide` and the
/// setting is on. Everything flows top-down: the panels place their content from the top of the space on offer.
fn content(ui: &mut Ui, st: &mut UiState, s: &Shared, with_guide: bool) {
    latest::show(ui, st, s);
    ui.horizontal(|ui| {
        caption(ui, "Bunkers heard");
        ui.label(
            RichText::new(format!("{} of 23", heard(s)))
                .font(theme::text(12.5))
                .color(DIM),
        );
    });
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            bunkers::show(ui, st, s);
            if with_guide && st.settings.show_guide {
                guide::show(ui, st, s);
            }
        });
}

/// The fixed column of a wide window.
pub fn column(ui: &mut Ui, st: &mut UiState, s: &Shared, rect: Rect, window_h: f32) {
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        ui.set_clip_rect(rect);
        content(ui, st, s, layout::show_guide(window_h));
    });
}

/// The button over the map and the sliding drawer of a narrow window. Escape or a click outside closes it.
pub fn drawer(ctx: &egui::Context, st: &mut UiState, s: &Shared, centre: Centre) {
    let was_open = st.drawer_open;
    let motion = Motion::new(st.settings.reduce_motion);
    let slide = motion.toward(
        ctx,
        Id::unique("drawer-slide"),
        f32::from(st.drawer_open),
        SLIDE_S,
        easing::cubic_out,
    );

    if slide < 1.0 {
        Area::new(Id::unique("drawer-toggle"))
            .order(Order::Middle)
            .fixed_pos(centre.map.right_top() + Vec2::new(-8.0, 8.0))
            .pivot(egui::Align2::RIGHT_TOP)
            .show(ctx, |ui| {
                let label = format!("Bunkers {}/23", heard(s));
                if button(ui, &label, ButtonKind::Quiet).clicked() {
                    st.drawer_open = true;
                }
            });
    }
    if slide <= 0.0 {
        return;
    }

    let screen = ctx.content_rect();
    let width = COLUMN_W.min(screen.width() - 40.0);
    let top = centre.area.top();
    let height = screen.bottom() - top;
    let shown = Rect::from_min_size(
        egui::pos2(screen.right() - width * slide, top),
        Vec2::new(width, height),
    );
    let area = Area::new(Id::unique("drawer"))
        .order(Order::Foreground)
        .fixed_pos(shown.min)
        .show(ctx, |ui| {
            ui.set_clip_rect(ui.clip_rect().intersect(screen));
            egui::Frame::new()
                .fill(SURFACE)
                .stroke(Stroke::new(1.0, STROKE))
                .inner_margin(egui::Margin::same(layout::PAD as i8))
                .show(ui, |ui| {
                    ui.set_width(width - 2.0 * layout::PAD);
                    ui.set_height(height - 2.0 * layout::PAD);
                    content(ui, st, s, false);
                });
        });

    let rect = area.response.rect;
    let escape = st.onboarding.is_none() && ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let outside = ctx.input(|i| {
        i.pointer.any_click() && i.pointer.interact_pos().is_some_and(|p| !rect.contains(p))
    });
    if was_open && (escape || outside) {
        st.drawer_open = false;
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::{Harness, kittest::Queryable};

    use crate::capture::Source;
    use crate::engine::{self, Handle};
    use crate::theme;
    use crate::ui::{self, Settings, UiState};

    fn harness(width: f32) -> Harness<'static, (Handle, UiState)> {
        let engine = engine::start(Source::Game, egui::Context::default(), None);
        let settings = Settings {
            onboarded: true,
            reduce_motion: true,
            ..Settings::default()
        };
        Harness::builder()
            .with_size(egui::vec2(width, 520.0))
            .with_context_setup(theme::install)
            .build_ui_state(
                |ui, (engine, st): &mut (Handle, UiState)| ui::draw(ui, engine, st),
                (engine, UiState::new(settings)),
            )
    }

    #[test]
    fn a_narrow_window_opens_the_drawer_and_escape_closes_it() {
        let mut h = harness(760.0);
        h.run_steps(4);
        assert!(!h.state().1.drawer_open);
        h.get_by_label("Bunkers 0/23").click();
        h.run_steps(4);
        assert!(h.state().1.drawer_open, "the button opens the drawer");
        h.key_press(egui::Key::Escape);
        h.run_steps(4);
        assert!(!h.state().1.drawer_open, "Escape closes it");
    }

    #[test]
    fn a_click_outside_closes_the_drawer_and_a_wide_window_has_no_drawer() {
        let mut h = harness(760.0);
        h.run_steps(4);
        h.get_by_label("Bunkers 0/23").click();
        h.run_steps(4);
        assert!(h.state().1.drawer_open);
        h.hover_at(egui::pos2(100.0, 300.0));
        h.event(egui::Event::PointerButton {
            pos: egui::pos2(100.0, 300.0),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        });
        h.event(egui::Event::PointerButton {
            pos: egui::pos2(100.0, 300.0),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        });
        h.run_steps(4);
        assert!(!h.state().1.drawer_open, "a click outside closes it");

        let mut wide = harness(1060.0);
        wide.run_steps(4);
        assert!(wide.query_by_label("Bunkers 0/23").is_none());
    }
}
