//! Window chrome: the header, the banner strips under it, the footer and the options menu.

mod footer;
mod header;
mod options;

use egui::{Frame, Margin, Panel, Ui};

use crate::engine::{Handle, Shared};
use crate::theme;
use crate::ui::UiState;

/// Heights the three chrome panels took, for the layout.
#[derive(Clone, Copy)]
pub struct Bands {
    pub header: f32,
    pub banners: f32,
    pub footer: f32,
}

fn panel_frame(x: i8, y: i8) -> Frame {
    Frame::new()
        .fill(theme::BG)
        .inner_margin(Margin::symmetric(x, y))
}

/// Shows header, banner strips (only when there is something to say) and footer.
pub fn show(ui: &mut Ui, engine: &Handle, st: &mut UiState, s: &Shared) -> Bands {
    let header = Panel::top("header")
        .frame(panel_frame(20, 12))
        .show(ui, |ui| header::show(ui, engine, st, s))
        .response
        .rect
        .height();
    let strips = options::strips(s);
    let banners = if strips.is_empty() {
        0.0
    } else {
        Panel::top("banners")
            .frame(panel_frame(20, 4))
            .show_separator_line(false)
            .show(ui, |ui| options::banner(ui, engine, &strips))
            .response
            .rect
            .height()
    };
    let footer = Panel::bottom("footer")
        .frame(panel_frame(20, 8))
        .show(ui, |ui| footer::show(ui, s))
        .response
        .rect
        .height();
    Bands {
        header,
        banners,
        footer,
    }
}
