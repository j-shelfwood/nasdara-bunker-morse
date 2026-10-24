//! The menu a click on a bunker card opens at the pointer: copy the lock code, or open the bunker on the web map.

use egui::{Button, Popup, Response, RichText, SetOpenCommand, Ui};
use nasdara_decoder::bunkers::is_izurvive_url;

use super::copied::mark_copied;
use crate::theme::{self, TEXT};
use crate::ui::UiState;

/// What the menu acts on. `key` names the card for the "copied" confirmation.
pub(in crate::ui) struct Target<'a> {
    pub key: &'a str,
    pub code: &'a str,
    pub url: Option<&'a str>,
}

/// Spoken after a card's description, so a screen reader announces what a click does.
pub(in crate::ui) const SPOKEN_HINT: &str = "Opens a menu to copy the code or open the web map";

/// The map link, when it is the iZurvive address the decoder builds (anything else is never opened).
pub(in crate::ui) fn map_link(url: Option<&str>) -> Option<&str> {
    url.filter(|u| is_izurvive_url(u))
}

/// Opens on a click or right click of `response`, where the pointer is; a keyboard activation has no pointer, so
/// the menu hangs from the card instead.
pub(in crate::ui) fn menu(response: &Response, st: &mut UiState, target: &Target) {
    let at_pointer = response
        .ctx
        .pointer_hover_pos()
        .is_some_and(|p| response.rect.contains(p));
    let popup = if at_pointer {
        let open = (response.clicked() || response.secondary_clicked())
            .then_some(SetOpenCommand::Bool(true));
        Popup::menu(response).open_memory(open).at_pointer_fixed()
    } else {
        Popup::menu(response)
    };
    popup.show(|ui| rows(ui, st, target));
}

fn rows(ui: &mut Ui, st: &mut UiState, target: &Target) {
    ui.set_min_width(190.0);
    let row = |text: &str| Button::new(RichText::new(text).font(theme::text(13.5)).color(TEXT));
    if ui
        .add(row("Copy code").shortcut_text(RichText::new(target.code).font(theme::digits(13.5))))
        .clicked()
    {
        ui.ctx().copy_text(target.code.to_string());
        mark_copied(st, target.key);
        ui.close();
    }
    if let Some(url) = map_link(target.url)
        && ui.add(row("Open on web map")).clicked()
    {
        st.open_later.push(url.to_string());
        ui.close();
    }
}
