//! The right column. It never scrolls: `layout::plan_column` cuts it into the dossier slot (the latest result, or the
//! card of the bunker selected in the roster), the roster of all 23 bunkers and, when the window is tall enough, the
//! Morse cheat sheet.

use egui::{Align, Layout, Rect, RichText, Ui, UiBuilder};

use super::layout::{self, ColumnPlan};
use super::{UiState, bunkers, guide, latest};
use crate::engine::Shared;
use crate::theme::{self, DIM};
use crate::ui::widgets::caption;

pub fn show(ui: &mut Ui, st: &mut UiState, s: &Shared, column: Rect) {
    let ColumnPlan {
        dossier,
        header,
        roster,
        guide: guide_rect,
    } = layout::plan_column(column, st.settings.show_guide);
    part(ui, dossier, |ui| show_dossier(ui, st, s, dossier.height()));
    part(ui, header, |ui| {
        ui.horizontal_centered(|ui| {
            caption(ui, "Bunkers heard");
            let heard = s.cards.iter().filter(|c| c.bunker.is_some()).count();
            ui.label(
                RichText::new(format!("{heard} of 23"))
                    .font(theme::text(12.5))
                    .color(DIM),
            );
        });
    });
    bunkers::roster(ui, st, s, roster);
    if let Some(rect) = guide_rect {
        part(ui, rect, |ui| guide::show(ui, st, s, rect.height()));
    }
}

/// Runs `add` in `rect`, top-down from its top-left corner, clipped to it.
fn part(ui: &mut Ui, rect: Rect, add: impl FnOnce(&mut Ui)) {
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::top_down(Align::Min)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    add(&mut child);
}

/// What the top slot holds: the selected bunker's card, else the latest result, else a placeholder saying why
/// there is none.
fn show_dossier(ui: &mut Ui, st: &mut UiState, s: &Shared, height: f32) {
    let selected = st
        .selected
        .as_deref()
        .and_then(|key| s.cards.iter().find(|c| c.key == key));
    if let Some(c) = selected {
        bunkers::card(ui, st, c);
    } else if !latest::show(ui, st, s) {
        let (title, text) = if s.receiving() {
            (
                "Receiving a transmission",
                "The result appears here when the broadcast ends.",
            )
        } else if s.cards.is_empty() {
            (
                "No bunker heard yet",
                "Each bunker that transmits gets a tile below, with its lock code and a map link.",
            )
        } else {
            (
                "Pick a bunker",
                "Select a tile below to see its grid, lock code and map link.",
            )
        };
        bunkers::empty(ui, height, title, text);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use egui_kittest::{Harness, kittest::Queryable};
    use nasdara_decoder::bunkers::{bunkers, izurvive_url};

    use crate::capture::Source;
    use crate::engine::{self, Card, Handle};
    use crate::theme;
    use crate::ui::layout::DOSSIER_H;
    use crate::ui::{self, MIN_WINDOW, Settings, UiState};

    fn card(n: usize, code: &str, count: usize, unclear: usize, corrected: bool) -> Card {
        let b = bunkers().iter().find(|b| b.index == n).expect("bunker");
        Card {
            key: format!("b{n}"),
            bunker: Some(n),
            grid: b.grid.clone(),
            code: code.into(),
            count,
            unclear,
            corrected,
            position: Some((b.x, b.z)),
            url: Some(izurvive_url(b.x, b.z, 7)),
            updated: Instant::now(),
        }
    }

    fn harness(size: egui::Vec2, cards: Vec<Card>) -> Harness<'static, (Handle, UiState)> {
        let engine = engine::start(Source::Game, egui::Context::default(), None);
        engine.lock().cards = cards;
        let settings = Settings {
            onboarded: true,
            reduce_motion: true,
            ..Settings::default()
        };
        Harness::builder()
            .with_size(size)
            .with_context_setup(theme::install)
            .build_ui_state(
                |ui, (engine, st): &mut (Handle, UiState)| ui::draw(ui, engine, st),
                (engine, UiState::new(settings)),
            )
    }

    #[test]
    fn the_worst_card_fits_the_dossier_slot_in_the_narrowest_column() {
        let size = MIN_WINDOW;
        let worst = card(8, "7??2", 2, 2, true);
        let mut h = harness(size, vec![worst]);
        h.run_steps(4);
        h.get_by_label_contains("Roster tile, bunker 8,").click();
        h.run_steps(6);
        let card = h.get_by_label_contains("Bunker 8, grid").rect();
        assert!(
            card.height() <= DOSSIER_H + 0.5,
            "card is {} tall, the slot {DOSSIER_H}",
            card.height()
        );
    }

    #[test]
    fn clicking_a_heard_tile_selects_it_and_clicking_again_clears_it() {
        let mut h = harness(
            egui::vec2(1060.0, 800.0),
            vec![card(5, "7??2", 2, 2, false)],
        );
        h.run_steps(4);
        assert!(h.state().1.selected.is_none());
        h.get_by_label_contains("Roster tile, bunker 5,").click();
        h.run_steps(4);
        assert_eq!(h.state().1.selected.as_deref(), Some("b5"));
        h.get_by_label_contains("Roster tile, bunker 5,").click();
        h.run_steps(4);
        assert!(h.state().1.selected.is_none());
    }

    #[test]
    fn clicking_the_dossier_card_opens_a_menu_with_copy_and_the_web_map() {
        let mut h = harness(
            egui::vec2(1060.0, 800.0),
            vec![card(5, "7??2", 2, 2, false)],
        );
        h.run_steps(4);
        h.get_by_label_contains("Roster tile, bunker 5,").click();
        h.run_steps(6);
        assert!(h.query_by_label_contains("Copy code").is_none());
        h.get_by_label_contains("Bunker 5, grid").click();
        h.run_steps(4);
        h.get_by_label("Open on web map");
        assert_eq!(
            h.state().1.selected.as_deref(),
            Some("b5"),
            "the card stays"
        );
        h.get_by_label_contains("Copy code").click();
        h.run_steps(4);
        let copied = h.state().1.copied.clone();
        assert_eq!(copied.map(|(k, _)| k).as_deref(), Some("b5"));
        assert!(
            h.query_by_label_contains("Copy code").is_none(),
            "the menu closes"
        );
        assert!(h.query_by_label("Open map").is_none());
    }

    #[test]
    fn a_card_without_a_map_link_offers_only_copy() {
        let mut c = card(5, "7??2", 2, 2, false);
        c.url = None;
        let mut h = harness(egui::vec2(1060.0, 800.0), vec![c]);
        h.run_steps(4);
        h.get_by_label_contains("Roster tile, bunker 5,").click();
        h.run_steps(6);
        h.get_by_label_contains("Bunker 5, grid").click();
        h.run_steps(4);
        h.get_by_label_contains("Copy code");
        assert!(h.query_by_label("Open on web map").is_none());
    }

    #[test]
    fn every_bunker_has_a_tile_and_an_unheard_one_is_not_a_button() {
        let mut h = harness(MIN_WINDOW, vec![card(5, "7??2", 2, 2, false)]);
        h.run_steps(4);
        assert_eq!(
            h.get_all_by_label_contains("Roster tile, bunker ").count(),
            23
        );
        h.get_by_label_contains("Roster tile, bunker 3, not heard yet");
        h.get_by_label_contains("Roster tile, bunker 5, grid");
    }
}
