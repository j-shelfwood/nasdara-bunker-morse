//! Where everything goes: a pure function from the window size to the regions of the HUD layout.
//!
//! Top to bottom: header, banner strips, centre, bottom bar, footer. The centre holds the map (a centred square) and,
//! in a wide window, the 300 point right column.

use egui::{Pos2, Rect, Vec2};

/// Window width from which the right column is shown; narrower windows use the drawer.
pub const WIDE_FROM: f32 = 900.0;
/// Window height from which the cheat sheet fits under the bunkers.
pub const GUIDE_FROM: f32 = 700.0;
pub const COLUMN_W: f32 = 300.0;
/// Space between the map, the column and the window edges.
pub const PAD: f32 = 12.0;
/// The map side kept when the window allows it.
const MIN_MAP: f32 = 300.0;

pub fn is_wide(window_w: f32) -> bool {
    window_w >= WIDE_FROM
}

pub fn show_guide(window_h: f32) -> bool {
    window_h >= GUIDE_FROM
}

/// Height of the bottom bar: a fifth of the window, between 104 and 190 points.
pub fn bar_height(window_h: f32) -> f32 {
    (window_h * 0.20).clamp(104.0, 190.0)
}

/// The centre area split into map and column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Centre {
    /// The area that was split.
    pub area: Rect,
    /// The square the map is drawn in.
    pub map: Rect,
    /// Space the map is centred in (the whole centre in a narrow window).
    pub map_area: Rect,
    pub column: Option<Rect>,
}

impl Centre {
    pub fn translated(self, by: Vec2) -> Centre {
        Centre {
            area: self.area.translate(by),
            map: self.map.translate(by),
            map_area: self.map_area.translate(by),
            column: self.column.map(|c| c.translate(by)),
        }
    }
}

/// Splits `area` (the window minus header, banners, bar and footer) for a window of size `window`.
pub fn centre(area: Rect, window: Vec2) -> Centre {
    let inner = area.shrink(PAD);
    let (map_area, column) = if is_wide(window.x) {
        let column = Rect::from_min_max(
            Pos2::new((inner.right() - COLUMN_W).max(inner.left()), inner.top()),
            inner.max,
        );
        let map_area = Rect::from_min_max(
            inner.min,
            Pos2::new((column.left() - PAD).max(inner.left()), inner.bottom()),
        );
        (map_area, Some(column))
    } else {
        (inner, None)
    };
    // a window too short for the padded square gives up the padding before the side
    let room = map_area.size().min_elem();
    let side = if room >= MIN_MAP {
        room
    } else {
        MIN_MAP.min(area.size().min_elem())
    };
    let map = Rect::from_center_size(map_area.center(), Vec2::splat(side));
    let map = map.intersect(area);
    Centre {
        area,
        map,
        map_area,
        column,
    }
}

/// What the draw code needs from the window size.
#[derive(Clone, Copy, Debug)]
pub struct Regions {
    pub bar: Rect,
    pub centre: Centre,
}

fn band(window_w: f32, top: f32, h: f32) -> Rect {
    Rect::from_min_size(Pos2::new(0.0, top), Vec2::new(window_w, h))
}

/// `header_h`, `banners_h` and `footer_h` are the heights the panels measured.
pub fn regions(window: Vec2, header_h: f32, banners_h: f32, footer_h: f32) -> Regions {
    let footer_top = (window.y - footer_h).max(0.0);
    let bar_h = bar_height(window.y);
    let bar = band(window.x, (footer_top - bar_h).max(0.0), bar_h);
    let top = header_h + banners_h;
    let area = Rect::from_min_max(Pos2::new(0.0, top), Pos2::new(window.x, bar.top().max(top)));
    Regions {
        bar,
        centre: centre(area, window),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: f32 = 56.0;
    const FOOTER: f32 = 32.0;

    fn at(w: f32, h: f32) -> Regions {
        regions(Vec2::new(w, h), HEADER, 0.0, FOOTER)
    }

    fn all(r: &Regions, w: f32, h: f32) -> Vec<(&'static str, Rect)> {
        let mut v = vec![
            ("header", band(w, 0.0, HEADER)),
            ("bar", r.bar),
            ("footer", band(w, h - FOOTER, FOOTER)),
            ("centre", r.centre.area),
        ];
        if let Some(c) = r.centre.column {
            v.push(("column", c));
        }
        v
    }

    #[test]
    fn bar_height_follows_the_window() {
        for (w, h, bar) in [
            (1060.0, 800.0, 160.0),
            (760.0, 520.0, 104.0),
            (1500.0, 900.0, 180.0),
            (1920.0, 1080.0, 190.0),
        ] {
            assert_eq!(at(w, h).bar.height(), bar, "{w}x{h}");
        }
    }

    #[test]
    fn regions_fit_the_window_without_overlap() {
        for (w, h) in [
            (1060.0, 800.0),
            (760.0, 520.0),
            (1500.0, 900.0),
            (1920.0, 1080.0),
        ] {
            let window = Rect::from_min_size(Pos2::ZERO, Vec2::new(w, h));
            let r = at(w, h);
            let mut rects = all(&r, w, h);
            rects.push(("map", r.centre.map));
            for (name, rect) in &rects {
                assert!(
                    window.contains_rect(*rect),
                    "{name} outside {w}x{h}: {rect:?}"
                );
            }
            // the bands are disjoint, and the column and the map sit inside the centre area without touching
            let bands: Vec<Rect> = all(&r, w, h)
                .into_iter()
                .map(|(_, r)| r)
                .filter(|r| r.width() == w)
                .collect();
            for (i, a) in bands.iter().enumerate() {
                for b in &bands[i + 1..] {
                    assert!(
                        !a.shrink(0.01).intersects(*b),
                        "{a:?} overlaps {b:?} at {w}x{h}"
                    );
                }
            }
            assert!(r.centre.area.contains_rect(r.centre.map));
            if let Some(c) = r.centre.column {
                assert!(r.centre.area.contains_rect(c));
                assert!(
                    !c.intersects(r.centre.map),
                    "column overlaps map at {w}x{h}"
                );
            }
        }
    }

    #[test]
    fn map_is_a_square_of_at_least_300_where_the_window_allows() {
        for (w, h) in [
            (1060.0, 800.0),
            (760.0, 520.0),
            (1500.0, 900.0),
            (1920.0, 1080.0),
        ] {
            let m = at(w, h).centre.map;
            assert!(
                (m.width() - m.height()).abs() < 0.01,
                "{m:?} not square at {w}x{h}"
            );
            assert!(m.width() >= 300.0, "{} at {w}x{h}", m.width());
        }
    }

    #[test]
    fn the_column_exists_only_from_900_wide() {
        assert!(at(1060.0, 800.0).centre.column.is_some());
        assert_eq!(at(1060.0, 800.0).centre.column.unwrap().width(), COLUMN_W);
        assert!(at(899.0, 800.0).centre.column.is_none());
        assert!(at(760.0, 520.0).centre.column.is_none());
        assert!(is_wide(900.0) && !is_wide(899.9));
        assert!(show_guide(700.0) && !show_guide(699.0));
    }

    #[test]
    fn banners_push_the_centre_down() {
        let r = regions(Vec2::new(1060.0, 800.0), HEADER, 40.0, FOOTER);
        assert_eq!(r.centre.area.top(), HEADER + 40.0);
        assert!(r.centre.map.top() >= r.centre.area.top());
    }
}
