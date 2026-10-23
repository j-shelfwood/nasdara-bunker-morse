//! Where a hover card sits relative to its pin: a pure function, so the corner cases are unit tested.

use egui::{Pos2, Rect, Vec2, vec2};

/// Offset of the card's near corner from the pin.
pub(super) const OFFSET: Vec2 = vec2(14.0, 14.0);
/// A card may not cover another pin by less than this many points.
const CLEARANCE: f32 = 10.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Anchor {
    UpRight,
    UpLeft,
    DownRight,
    DownLeft,
    /// No side fits: the card was pushed inside the bounds and may overlap the pin.
    Clamped,
}

impl Anchor {
    /// The card sits above the pin (it slides in from below).
    pub fn is_above(self) -> bool {
        matches!(self, Anchor::UpRight | Anchor::UpLeft)
    }
}

/// Up-right of the pin; flips left, then below, then clamps inside `bounds`.
/// Sides that cover another pin are skipped while a clean side exists.
pub(super) fn place(pin: Pos2, size: Vec2, bounds: Rect, other_pins: &[Pos2]) -> (Rect, Anchor) {
    let at = |left: bool, below: bool| {
        let x = if left {
            pin.x - OFFSET.x - size.x
        } else {
            pin.x + OFFSET.x
        };
        let y = if below {
            pin.y + OFFSET.y
        } else {
            pin.y - OFFSET.y - size.y
        };
        Rect::from_min_size(Pos2::new(x, y), size)
    };
    let sides = [
        (false, false, Anchor::UpRight),
        (true, false, Anchor::UpLeft),
        (false, true, Anchor::DownRight),
        (true, true, Anchor::DownLeft),
    ];
    let inside = |r: Rect| bounds.contains_rect(r);
    let clear = |r: Rect| {
        let zone = r.expand(CLEARANCE);
        !other_pins.iter().any(|p| zone.contains(*p))
    };
    for (left, below, anchor) in sides {
        let r = at(left, below);
        if inside(r) && clear(r) {
            return (r, anchor);
        }
    }
    for (left, below, anchor) in sides {
        let r = at(left, below);
        if inside(r) {
            return (r, anchor);
        }
    }
    (clamp(at(false, false), bounds), Anchor::Clamped)
}

fn clamp(r: Rect, bounds: Rect) -> Rect {
    let x = r.min.x.min(bounds.max.x - r.width()).max(bounds.min.x);
    let y = r.min.y.min(bounds.max.y - r.height()).max(bounds.min.y);
    Rect::from_min_size(Pos2::new(x, y), r.size())
}

/// Where the leader ends on the card: the corner nearest the pin, on the chamfer when that corner is cut.
pub(super) fn attach(card: Rect, pin: Pos2, cut: f32) -> Pos2 {
    let h = cut / 2.0;
    let corners = [
        card.left_top() + vec2(h, h),
        card.right_top(),
        card.right_bottom() - vec2(h, h),
        card.left_bottom(),
    ];
    corners
        .into_iter()
        .min_by(|a, b| a.distance(pin).total_cmp(&b.distance(pin)))
        .unwrap_or(card.center())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARD: Vec2 = vec2(160.0, 100.0);

    fn bounds() -> Rect {
        Rect::from_min_size(Pos2::ZERO, vec2(500.0, 500.0))
    }

    fn at(pin: Pos2) -> (Rect, Anchor) {
        place(pin, CARD, bounds(), &[])
    }

    #[test]
    fn open_space_goes_up_right() {
        let pin = Pos2::new(200.0, 300.0);
        let (r, a) = at(pin);
        assert_eq!(a, Anchor::UpRight);
        assert_eq!(r.min, Pos2::new(214.0, 300.0 - 14.0 - 100.0));
    }

    #[test]
    fn the_four_corners_flip_toward_free_space() {
        let b = bounds();
        let cases = [
            (b.left_top() + vec2(6.0, 6.0), Anchor::DownRight),
            (b.right_top() + vec2(-6.0, 6.0), Anchor::DownLeft),
            (b.left_bottom() + vec2(6.0, -6.0), Anchor::UpRight),
            (b.right_bottom() + vec2(-6.0, -6.0), Anchor::UpLeft),
        ];
        for (pin, want) in cases {
            let (r, a) = at(pin);
            assert_eq!(a, want, "pin {pin:?}");
            assert!(b.contains_rect(r), "pin {pin:?} card {r:?}");
        }
    }

    #[test]
    fn a_card_over_another_pin_takes_the_next_side() {
        let pin = Pos2::new(200.0, 300.0);
        let (up_right, _) = at(pin);
        let blocker = up_right.center();
        let (r, a) = place(pin, CARD, bounds(), &[blocker]);
        assert_eq!(a, Anchor::UpLeft);
        assert!(!r.expand(10.0).contains(blocker));
        let near_edge = up_right.right_top() + vec2(8.0, 0.0);
        let (_, a) = place(pin, CARD, bounds(), &[near_edge]);
        assert_ne!(
            a,
            Anchor::UpRight,
            "a pin 8 px beside the card counts as covered"
        );
    }

    #[test]
    fn a_300_px_map_still_holds_the_card() {
        let b = Rect::from_min_size(Pos2::new(40.0, 20.0), vec2(300.0, 300.0));
        let mut clamped = 0;
        for ix in 0..=10 {
            for iy in 0..=10 {
                let pin = b.min + vec2(ix as f32 * 30.0, iy as f32 * 30.0);
                let (r, a) = place(pin, CARD, b, &[]);
                assert!(b.contains_rect(r), "pin {pin:?} anchor {a:?} card {r:?}");
                clamped += usize::from(a == Anchor::Clamped);
            }
        }
        // only the middle columns (x in 127..173) have no free side; every other pin keeps a real anchor
        assert_eq!(clamped, 11, "one clamped pin per row of the middle column");
    }

    #[test]
    fn the_leader_meets_the_nearest_corner() {
        let pin = Pos2::new(200.0, 300.0);
        let (r, a) = at(pin);
        assert_eq!(a, Anchor::UpRight);
        assert_eq!(attach(r, pin, 8.0), r.left_bottom());
        let pin = Pos2::new(400.0, 300.0);
        let (r, _) = place(pin, CARD, bounds(), &[Pos2::new(430.0, 200.0)]);
        let end = attach(r, pin, 8.0);
        assert_eq!(end, r.right_bottom() - vec2(4.0, 4.0));
    }
}
