use egui::{Rect, Vec2};
use egui_kittest::{Harness, kittest::Queryable};
use nasdara_decoder::bunkers::bunkers;
use std::time::Instant;

use super::*;
use crate::theme;
use crate::ui::Settings;

fn inside(loop_: &[(f32, f32)], x: f32, z: f32) -> bool {
    let mut hit = false;
    for (i, &(ax, az)) in loop_.iter().enumerate() {
        let (bx, bz) = loop_[(i + 1) % loop_.len()];
        if (az > z) != (bz > z) && x < ax + (z - az) / (bz - az) * (bx - ax) {
            hit = !hit;
        }
    }
    hit
}

#[test]
fn every_bunker_lies_on_the_outlined_land() {
    for b in bunkers() {
        let land = coast::COAST
            .iter()
            .any(|l| inside(l, b.x as f32, b.z as f32));
        assert!(land, "bunker {} is outside the island outline", b.index);
    }
}

fn card(n: usize, code: &str, count: usize) -> Card {
    let b = bunkers().iter().find(|b| b.index == n).expect("bunker");
    Card {
        key: format!("b{n}"),
        bunker: Some(n),
        grid: b.grid.clone(),
        code: code.into(),
        count,
        unclear: code.matches('?').count(),
        corrected: false,
        position: Some((b.x, b.z)),
        url: None,
        updated: Instant::now(),
    }
}

fn harness(side: f32) -> Harness<'static, (UiState, Vec<Card>)> {
    let settings = Settings {
        onboarded: true,
        reduce_motion: true,
        ..Settings::default()
    };
    let cards = vec![card(7, "48?1", 3), card(2, "1234", 1)];
    Harness::builder()
        .with_size(Vec2::splat(side + 40.0))
        .with_context_setup(theme::install)
        .build_ui_state(
            move |ui, (st, cards): &mut (UiState, Vec<Card>)| {
                let rect = Rect::from_min_size(ui.max_rect().min, Vec2::splat(side));
                draw(ui, st, cards, rect);
            },
            (UiState::new(settings), cards),
        )
}

fn shape_count(h: &Harness<'_, (UiState, Vec<Card>)>) -> usize {
    h.output().shapes.len()
}

#[test]
fn accesskit_lists_23_pins_and_speaks_the_lock() {
    for side in [300.0, 700.0] {
        let mut h = harness(side);
        h.run_steps(3);
        assert_eq!(
            h.get_all_by_label_contains("Bunker ").count(),
            23,
            "at {side}"
        );
        let b = bunkers().iter().find(|b| b.index == 7).expect("bunker 7");
        h.get_by_label_contains("Bunker 7, lock code 4 8 unclear 1, grid ");
        h.get_by_label_contains(&format!(
            "heard 3 times, coordinates x {} z {}",
            b.x.round(),
            b.z.round()
        ));
        h.get_by_label_contains("Bunker 3, not heard yet, grid ");
    }
}

#[test]
fn hovering_a_pin_floats_its_card_and_leaving_removes_it() {
    for label in ["Bunker 7, lock code", "Bunker 3, not heard"] {
        let mut h = harness(500.0);
        h.run_steps(3);
        let idle = shape_count(&h);
        h.get_by_label_contains(label).hover();
        h.run_steps(3);
        assert!(
            shape_count(&h) > idle + 15,
            "{label}: {} shapes vs {idle} idle",
            shape_count(&h)
        );
        h.hover_at(egui::pos2(2.0, 2.0));
        h.run_steps(3);
        assert_eq!(
            shape_count(&h),
            idle,
            "{label}: the card leaves with the pointer"
        );
    }
}

#[test]
fn click_selects_the_card_and_escape_clears_it() {
    let mut h = harness(500.0);
    h.run_steps(3);
    h.get_by_label_contains("Bunker 7, lock code").click();
    h.run_steps(3);
    assert_eq!(h.state().0.selected.as_deref(), Some("b7"));
    h.get_by_label_contains("Bunker 3, not heard").click();
    h.run_steps(3);
    assert_eq!(
        h.state().0.selected.as_deref(),
        Some("b7"),
        "an unheard pin selects nothing"
    );
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert_eq!(h.state().0.selected, None);
}

#[test]
fn a_selected_card_stays_after_the_pointer_leaves() {
    let mut h = harness(500.0);
    h.run_steps(3);
    let idle = shape_count(&h);
    h.get_by_label_contains("Bunker 7, lock code").click();
    h.hover_at(egui::pos2(2.0, 2.0));
    h.run_steps(3);
    assert!(shape_count(&h) > idle + 15, "selected card is pinned open");
}

#[test]
fn scale_stays_between_the_limits() {
    assert_eq!(scale(300.0), 0.85);
    assert_eq!(scale(700.0), 1.25);
    assert!((scale(420.0) - 1.0).abs() < 1e-6);
}
