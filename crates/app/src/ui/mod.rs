pub(crate) mod anim;
mod bar;
mod bunkers;
mod chrome;
mod guide;
pub(crate) mod hud;
mod latest;
mod layout;
mod map;
mod onboarding;
mod state;
mod widgets;

use std::time::Duration;

use egui::{CentralPanel, Frame, Margin, Panel, Ui};

pub use state::{MIN_WINDOW, Settings, UiState};

use crate::capture::Status;
use crate::engine::Handle;
use crate::theme;
// the cheat sheet reads the symbols heard so far through its old path until the guide lane switches to `bar`

pub fn draw(ui: &mut Ui, engine: &Handle, st: &mut UiState) {
    let ctx = ui.ctx().clone();
    let mut s = engine.lock();

    // animation drives its own repaints; idle windows repaint a few times a second so clocks and ages stay current
    // a listening window scrolls the signal strip, so it needs a steady rate before a transmission is recognised
    if s.receiving() {
        ctx.request_repaint_after(Duration::from_millis(16));
    } else if s.status == Status::Listening && !st.settings.reduce_motion {
        ctx.request_repaint_after(Duration::from_millis(33));
    } else {
        ctx.request_repaint_after(Duration::from_millis(250));
    }

    st.size_window(&ctx);
    let to_open = st.react_to_new_bunkers(&ctx, &mut s);
    st.sync_window_level(&ctx);

    let bands = chrome::show(ui, engine, st, &s);
    // the space the root ui really has (a test harness insets it), not the viewport size
    let root = ui.max_rect();
    let window = root.size();
    let regions = layout::regions(window, bands.header, bands.banners, bands.footer);
    let wide = layout::is_wide(window.x);

    Panel::bottom("bar")
        .exact_size(regions.bar.height())
        .frame(
            Frame::new()
                .fill(theme::BG)
                .inner_margin(Margin::symmetric(12, 4)),
        )
        .show_separator_line(false)
        .show(ui, |ui| bar::show(ui, st, &s, wide));

    CentralPanel::default()
        .frame(Frame::new().fill(theme::BG))
        .show(ui, |ui| {
            let centre = regions.centre.translated(root.min.to_vec2());
            map::show(ui, st, &s, centre.map);
            match centre.column {
                Some(column) => {
                    st.drawer_open = false;
                    chrome::column(ui, st, &s, column, window.y);
                }
                None => chrome::drawer(&ctx, st, &s, centre),
            }
        });

    drop(s);
    // opening a link can be slow: never while the engine waits for the lock
    for url in to_open {
        let _ = open::that(url);
    }
    if let Some(page) = st.onboarding {
        let mut page = page;
        match onboarding::show(&ctx, &st.modal_ids, &mut page, st.settings.reduce_motion) {
            onboarding::Outcome::Open => st.onboarding = Some(page),
            onboarding::Outcome::Close => st.finish_onboarding(),
        }
    }
}
