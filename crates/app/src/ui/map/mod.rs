//! The 23 bunkers on a square plan of the map, drawn inside a rect the caller allocates.
//! Each pin is a keyboard-focusable button; hovering, focusing or selecting one floats its card over the pin.

mod built_up;
mod coast;
mod content;
mod hover;
mod paint;
mod pins;
mod place;
mod projection;

use egui::{
    Context, CursorIcon, Id, Key, LayerId, Order, Rect, Response, Role, Sense, Ui, WidgetInfo, vec2,
};

use super::UiState;
use crate::engine::{Card, Shared};
use crate::ui::anim::{Motion, easing};
use crate::ui::widgets::focus_ring;
use pins::Pin;
use projection::Projection;

/// Scale of pins, hit targets, ticks and fonts: 0.85 at a 300 px map, capped at 1.25 for 700 px.
fn scale(side: f32) -> f32 {
    (side / 420.0).clamp(0.85, 1.25)
}

const CARD_IN: f32 = 0.14;
const CARD_OUT: f32 = 0.09;

/// Pins for the cards in `s`; a click selects the card, shared with the bunker list.
pub fn show(ui: &mut Ui, st: &mut UiState, s: &Shared, rect: Rect) {
    draw(ui, st, &s.cards, rect);
}

fn draw(ui: &mut Ui, st: &mut UiState, cards: &[Card], rect: Rect) {
    let ctx = ui.ctx().clone();
    let k = scale(rect.width().min(rect.height()));
    let motion = Motion::new(st.settings.reduce_motion);
    let p = ui.painter_at(rect);
    paint::frame(&p, rect, k);
    let inner = rect.shrink(paint::margin(k));
    paint::grid(&p, rect, inner, k);
    let proj = Projection::new(inner);
    paint::outlines(&p, &proj);

    let pins = pins::layout(&proj, ui.make_persistent_id("map-pin"), k);
    let responses = interact(ui, st, cards, &pins);

    let any_focus = responses.iter().any(Response::has_focus);
    if st.selected.is_some()
        && (any_focus || ui.rect_contains_pointer(rect))
        && ui.input(|i| i.key_pressed(Key::Escape))
    {
        st.selected = None;
    }
    let chosen = st.selected.clone();
    let selected = |c: Option<&Card>| c.is_some_and(|c| chosen.as_deref() == Some(&c.key));
    for (pin, resp) in pins.iter().zip(&responses) {
        let card = pins::card_of(cards, pin.bunker.index);
        let hot = resp.hovered() || resp.has_focus();
        pins::paint(&p, &ctx, motion, pin, card, (selected(card), hot), k);
        focus_ring(&p, resp, 10.0 * k);
    }
    let shown: Vec<bool> = pins
        .iter()
        .zip(&responses)
        .map(|(pin, r)| {
            let card = pins::card_of(cards, pin.bunker.index);
            r.hovered() || r.has_focus() || selected(card)
        })
        .collect();
    float_cards(&ctx, motion, rect, k, cards, &pins, &shown, &responses);
}

/// One interact per pin. Pins are registered farthest-from-the-pointer first, so where hit
/// rects overlap the closest pin is on top and wins.
fn interact(ui: &mut Ui, st: &mut UiState, cards: &[Card], pins: &[Pin]) -> Vec<Response> {
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let mut order: Vec<usize> = (0..pins.len()).collect();
    if let Some(at) = pointer {
        order.sort_by(|&a, &b| {
            pins[b]
                .pos
                .distance(at)
                .total_cmp(&pins[a].pos.distance(at))
        });
    }
    let mut out: Vec<Option<Response>> = pins.iter().map(|_| None).collect();
    for i in order {
        let pin = &pins[i];
        let card = pins::card_of(cards, pin.bunker.index);
        let is_selected = card.is_some_and(|c| st.selected.as_deref() == Some(&c.key));
        let resp = ui.interact(pin.hit, pin.id, Sense::click());
        let spoken = content::spoken(pin.bunker, card);
        resp.widget_info(|| WidgetInfo::selected(Role::Button, true, is_selected, spoken.clone()));
        if let Some(c) = card {
            if resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if resp.clicked() {
                st.selected = if is_selected {
                    None
                } else {
                    Some(c.key.clone())
                };
            }
        }
        out[i] = Some(resp);
    }
    out.into_iter().flatten().collect()
}

/// The cards, on an unclipped layer above everything and without widgets, so they never take a click.
#[allow(clippy::too_many_arguments)]
fn float_cards(
    ctx: &Context,
    motion: Motion,
    bounds: Rect,
    k: f32,
    cards: &[Card],
    pins: &[Pin],
    shown: &[bool],
    responses: &[Response],
) {
    let layer = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::unique("map-cards")));
    let positions: Vec<_> = pins.iter().map(|p| p.pos).collect();
    let mut visible: Vec<(usize, f32)> = Vec::new();
    for (i, pin) in pins.iter().enumerate() {
        let secs = if shown[i] { CARD_IN } else { CARD_OUT };
        let t = motion.toward(
            ctx,
            pin.id.with("card"),
            f32::from(u8::from(shown[i])),
            secs,
            easing::cubic_out,
        );
        if t > 0.001 {
            visible.push((i, t));
        }
    }
    // the pointed-at card is drawn last, over the others
    visible.sort_by_key(|&(i, _)| responses[i].hovered() || responses[i].has_focus());
    for (i, t) in visible {
        let pin = &pins[i];
        let c = content::build(pin.bunker, pins::card_of(cards, pin.bunker.index));
        let laid = hover::layout(&layer, &c, k);
        let others: Vec<_> = positions
            .iter()
            .enumerate()
            .filter_map(|(j, q)| (j != i).then_some(*q))
            .collect();
        let (rect, anchor) = place::place(pin.pos, laid.size, bounds.shrink(2.0), &others);
        let slide = (1.0 - t) * 8.0 * k * if anchor.is_above() { 1.0 } else { -1.0 };
        let mut p = layer.clone();
        p.set_opacity(t);
        hover::paint(&p, &laid, &c, rect.translate(vec2(0.0, slide)), pin.pos);
    }
}

#[cfg(test)]
mod tests;
