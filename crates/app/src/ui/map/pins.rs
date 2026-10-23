//! The 23 pins: where they are, how they look, and what changed since the last frame.

use egui::{Align2, Context, Id, Painter, Pos2, Rect, Stroke, Vec2, vec2};

use super::projection::Projection;
use crate::engine::Card;
use crate::theme::{self, AMBER, BG, RULE, TEXT, alpha};
use crate::ui::anim::{Motion, easing, egui_time_of};
use crate::ui::bunkers::{Reading, reading, reading_color};
use crate::ui::widgets::paint_check;
use nasdara_decoder::bunkers::{Bunker, bunkers};

pub(super) struct Pin {
    pub bunker: &'static Bunker,
    pub pos: Pos2,
    /// The click and hover target, centred on the pin.
    pub hit: Rect,
    pub id: Id,
}

/// One pin per bunker with its own hit rect and id.
pub(super) fn layout(proj: &Projection, base: Id, k: f32) -> Vec<Pin> {
    let side = (20.0 * k).max(20.0);
    bunkers()
        .iter()
        .map(|b| {
            let pos = proj.to_screen(b.x, b.z);
            Pin {
                bunker: b,
                pos,
                hit: Rect::from_center_size(pos, Vec2::splat(side)),
                id: base.with(b.index),
            }
        })
        .collect()
}

pub(super) fn card_of(cards: &[Card], bunker: usize) -> Option<&Card> {
    cards.iter().find(|c| c.bunker == Some(bunker))
}

/// Seconds between the three rings that announce a changed card.
const RING_EVERY: f64 = 1.2;
const RINGS: f64 = 3.0;

/// egui time of the card's last change while it is recent enough to still announce, else `None`.
fn announced_at(ctx: &Context, c: &Card) -> Option<f64> {
    let changed = egui_time_of(ctx, Id::unique(("map-seen", &c.key)), c.updated);
    (ctx.input(|i| i.time) - changed < RING_EVERY * RINGS).then_some(changed)
}

pub(super) fn paint(
    p: &Painter,
    ctx: &Context,
    motion: Motion,
    pin: &Pin,
    card: Option<&Card>,
    (selected, hot): (bool, bool),
    k: f32,
) {
    let pos = pin.pos;
    if hot {
        p.circle_stroke(pos, 10.0 * k, Stroke::new(1.2, alpha(TEXT, 0.7)));
    }
    let Some(c) = card else {
        p.circle_filled(pos, 2.6 * k, RULE);
        return;
    };
    if selected {
        selection(p, ctx, motion, pos, k);
    }
    if let Some(start) = announced_at(ctx, c) {
        let now = ctx.input(|i| i.time);
        let n = ((now - start) / RING_EVERY).floor();
        if n < RINGS {
            let v = motion.pulse(
                ctx,
                Some(start + n * RING_EVERY),
                RING_EVERY as f32,
                easing::cubic_out,
            );
            p.circle_stroke(
                pos,
                (5.0 + 12.0 * (1.0 - v)) * k,
                Stroke::new(1.2, alpha(AMBER, 0.6 * v)),
            );
        }
    }
    let r = reading(c);
    let color = reading_color(r);
    p.circle_filled(pos, 6.5 * k, alpha(color, 0.18));
    match r {
        // unclear: an open ring with a question mark, not a different shade
        Reading::Unclear => {
            p.circle_filled(pos, 7.0 * k, BG);
            p.circle_stroke(pos, 7.0 * k, Stroke::new(1.8, color));
            p.text(
                pos,
                Align2::CENTER_CENTER,
                "?",
                theme::bold(11.0 * k),
                color,
            );
        }
        Reading::Once => {
            p.circle_filled(pos, 4.2 * k, color);
        }
        Reading::Confirmed => {
            p.circle_filled(pos, 5.2 * k, color);
            paint_check(p, pos, 5.0 * k, BG, 1.4);
        }
    }
}

/// A breathing ring and a range-style reticle of four ticks around the selected pin.
fn selection(p: &Painter, ctx: &Context, motion: Motion, pos: Pos2, k: f32) {
    let b = motion.breathe(ctx, 0.5);
    p.circle_stroke(
        pos,
        10.5 * k,
        Stroke::new(1.3, alpha(AMBER, 0.35 + 0.45 * b)),
    );
    let tick = Stroke::new(1.4, alpha(AMBER, 0.9));
    for d in [
        vec2(1.0, 0.0),
        vec2(-1.0, 0.0),
        vec2(0.0, 1.0),
        vec2(0.0, -1.0),
    ] {
        p.line_segment([pos + d * 13.0 * k, pos + d * 18.0 * k], tick);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Rect;
    use std::collections::HashSet;

    #[test]
    fn twenty_three_pins_with_distinct_ids_and_hit_rects() {
        for side in [300.0, 500.0, 700.0] {
            let rect = Rect::from_min_size(Pos2::new(10.0, 10.0), vec2(side, side));
            let proj = Projection::new(rect.shrink(16.0));
            let pins = layout(&proj, Id::unique("map"), 1.0);
            assert_eq!(pins.len(), 23);
            let ids: HashSet<_> = pins.iter().map(|p| p.id).collect();
            assert_eq!(ids.len(), 23, "ids at {side}");
            let rects: HashSet<_> = pins
                .iter()
                .map(|p| (p.hit.min.x.to_bits(), p.hit.min.y.to_bits()))
                .collect();
            assert_eq!(rects.len(), 23, "hit rects at {side}");
            assert!(
                pins.iter().all(|p| rect.contains_rect(p.hit)),
                "inside at {side}"
            );
        }
    }
}
