//! Where everything goes: a pure function from the window size to the regions of the HUD layout.
//!
//! Top to bottom: header, banner strips, centre, bottom bar, footer. The centre holds the map (a centred square) and
//! the right column, which never scrolls: its parts split the height between them (`plan_column`).

use egui::{Pos2, Rect, Vec2};

/// Space between the map, the column and the window edges.
pub const PAD: f32 = 12.0;
/// The map side kept when the window allows it.
const MIN_MAP: f32 = 300.0;

/// The right column is a share of the window width, between these.
pub fn column_width(window_w: f32) -> f32 {
    (window_w * 0.27).clamp(300.0, 420.0)
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
    /// Space the map is centred in.
    pub map_area: Rect,
    pub column: Rect,
}

impl Centre {
    pub fn translated(self, by: Vec2) -> Centre {
        Centre {
            area: self.area.translate(by),
            map: self.map.translate(by),
            map_area: self.map_area.translate(by),
            column: self.column.translate(by),
        }
    }
}

/// Splits `area` (the window minus header, banners, bar and footer) for a window of size `window`.
pub fn centre(area: Rect, window: Vec2) -> Centre {
    let inner = area.shrink(PAD);
    let column = Rect::from_min_max(
        Pos2::new(
            (inner.right() - column_width(window.x)).max(inner.left()),
            inner.top(),
        ),
        inner.max,
    );
    let map_area = Rect::from_min_max(
        inner.min,
        Pos2::new((column.left() - PAD).max(inner.left()), inner.bottom()),
    );
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

/// Height of the slot at the top of the column: the latest result, the selected bunker's card or the empty state.
/// Fixed, so the roster below keeps its size when the slot changes what it shows.
pub const DOSSIER_H: f32 = 240.0;
/// Height of the "Bunkers heard" caption row.
pub const ROSTER_HEADER_H: f32 = 22.0;
/// Space between the parts of the column.
pub const GAP: f32 = 10.0;
/// Bunkers on the map, and so tiles in the roster.
pub const BUNKER_COUNT: usize = 23;
pub const TILE_GAP: f32 = 4.0;
/// Roster tile height range; the roster takes the height the other parts leave, within these.
pub const TILE_H_MIN: f32 = 30.0;
pub const TILE_H_MAX: f32 = 72.0;
/// Least and most height of the cheat sheet; below the least it is left out.
pub const GUIDE_H_MIN: f32 = 138.0;
pub const GUIDE_H_MAX: f32 = 260.0;
/// Shares of the spare height the roster and the cheat sheet take.
const ROSTER_WEIGHT: f32 = 3.0;
const GUIDE_WEIGHT: f32 = 2.0;

/// Columns and rows of the roster in `height` points: 5 by 5 while that keeps the tiles at `TILE_H_MIN`, else 6 by 4.
pub fn roster_grid(height: f32) -> (usize, usize) {
    let five_rows = (height - TILE_GAP * 4.0) / 5.0;
    let cols = if five_rows >= TILE_H_MIN { 5 } else { 6 };
    (cols, BUNKER_COUNT.div_ceil(cols))
}

/// Height of `rows` rows of tiles `tile` high.
fn rows_height(rows: usize, tile: f32) -> f32 {
    let rows = rows as f32;
    tile * rows + TILE_GAP * (rows - 1.0)
}

/// One part of a stack that shares the height left over after every part has its least.
#[derive(Clone, Copy, Debug)]
pub struct Flex {
    pub min: f32,
    pub max: f32,
    pub weight: f32,
}

/// Heights for `parts` in `total`: each at least `min`, the spare height split by weight, a part that hits its `max`
/// handing the rest to the others. Returns `None` when the minimums do not fit.
pub fn flex(total: f32, parts: &[Flex]) -> Option<Vec<f32>> {
    let mut heights: Vec<f32> = parts.iter().map(|p| p.min).collect();
    let mut spare = total - heights.iter().sum::<f32>();
    if spare < -0.001 {
        return None;
    }
    let mut open: Vec<usize> = (0..parts.len()).collect();
    while spare > 0.001 && !open.is_empty() {
        let weights: f32 = open.iter().map(|&i| parts[i].weight).sum();
        if weights <= 0.0 {
            break;
        }
        let mut used = 0.0;
        open.retain(|&i| {
            let share = spare * parts[i].weight / weights;
            let take = share.min(parts[i].max - heights[i]);
            heights[i] += take;
            used += take;
            take >= share - 0.001
        });
        spare -= used;
    }
    Some(heights)
}

/// The parts of the right column, top to bottom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColumnPlan {
    pub dossier: Rect,
    pub header: Rect,
    pub roster: Rect,
    pub guide: Option<Rect>,
}

/// Cuts `column` into the dossier slot, the roster (caption and 23 tiles) and, when the height allows and `guide` is
/// set, the cheat sheet. The roster is the part that gives: its tiles shrink to `TILE_H_MIN` (then it turns to six
/// columns) and grow to `TILE_H_MAX`.
pub fn plan_column(column: Rect, guide: bool) -> ColumnPlan {
    // the least is the 6 by 4 grid of minimum tiles, the most 5 by 5 of maximum ones
    let roster = Flex {
        min: rows_height(4, TILE_H_MIN),
        max: rows_height(5, TILE_H_MAX),
        weight: ROSTER_WEIGHT,
    };
    let guide_part = Flex {
        min: GUIDE_H_MIN,
        max: GUIDE_H_MAX,
        weight: GUIDE_WEIGHT,
    };
    let fixed = DOSSIER_H + GAP + ROSTER_HEADER_H + 6.0;
    let room = column.height() - fixed;
    let with_guide = guide
        .then(|| flex(room - GAP, &[roster, guide_part]))
        .flatten();
    let (roster_h, guide_h) = match with_guide {
        Some(h) => (h[0], Some(h[1])),
        None => (room.max(0.0).min(roster.max), None),
    };
    let top = column.top();
    let dossier = Rect::from_min_size(column.min, Vec2::new(column.width(), DOSSIER_H));
    let header = Rect::from_min_size(
        Pos2::new(column.left(), dossier.bottom() + GAP),
        Vec2::new(column.width(), ROSTER_HEADER_H),
    );
    let roster = Rect::from_min_size(
        Pos2::new(column.left(), header.bottom() + 6.0),
        Vec2::new(column.width(), roster_h),
    );
    let guide = guide_h.map(|h| {
        Rect::from_min_size(
            Pos2::new(column.left(), roster.bottom() + GAP),
            Vec2::new(column.width(), h),
        )
    });
    debug_assert!(dossier.top() == top);
    ColumnPlan {
        dossier,
        header,
        roster,
        guide,
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

    /// About what the header and footer panels measure.
    const HEADER: f32 = 64.0;
    const FOOTER: f32 = 36.0;

    fn at(w: f32, h: f32) -> Regions {
        regions(Vec2::new(w, h), HEADER, 0.0, FOOTER)
    }

    fn all(r: &Regions, w: f32, h: f32) -> Vec<(&'static str, Rect)> {
        vec![
            ("header", band(w, 0.0, HEADER)),
            ("bar", r.bar),
            ("footer", band(w, h - FOOTER, FOOTER)),
            ("centre", r.centre.area),
            ("column", r.centre.column),
        ]
    }

    const SIZES: [(f32, f32); 5] = [
        (900.0, 700.0),
        (1060.0, 800.0),
        (1500.0, 900.0),
        (1920.0, 1080.0),
        (2560.0, 1440.0),
    ];

    #[test]
    fn bar_height_follows_the_window() {
        for (w, h, bar) in [
            (1060.0, 800.0, 160.0),
            (900.0, 700.0, 140.0),
            (1500.0, 900.0, 180.0),
            (1920.0, 1080.0, 190.0),
        ] {
            assert_eq!(at(w, h).bar.height(), bar, "{w}x{h}");
        }
    }

    #[test]
    fn regions_fit_the_window_without_overlap() {
        for (w, h) in SIZES {
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
            assert!(r.centre.area.contains_rect(r.centre.column));
            assert!(
                !r.centre.column.intersects(r.centre.map),
                "column overlaps map at {w}x{h}"
            );
        }
    }

    #[test]
    fn map_is_a_square_of_at_least_300() {
        for (w, h) in SIZES {
            let m = at(w, h).centre.map;
            assert!(
                (m.width() - m.height()).abs() < 0.01,
                "{m:?} not square at {w}x{h}"
            );
            assert!(m.width() >= 300.0, "{} at {w}x{h}", m.width());
        }
    }

    #[test]
    fn the_column_scales_with_the_window_between_300_and_420() {
        assert_eq!(column_width(900.0), 300.0);
        assert_eq!(column_width(1060.0), 300.0);
        assert!((column_width(1500.0) - 405.0).abs() < 1e-3);
        assert_eq!(column_width(2560.0), 420.0);
    }

    #[test]
    fn banners_push_the_centre_down() {
        let r = regions(Vec2::new(1060.0, 800.0), HEADER, 40.0, FOOTER);
        assert_eq!(r.centre.area.top(), HEADER + 40.0);
        assert!(r.centre.map.top() >= r.centre.area.top());
    }

    fn parts() -> [Flex; 2] {
        [
            Flex {
                min: 100.0,
                max: 200.0,
                weight: 3.0,
            },
            Flex {
                min: 50.0,
                max: 80.0,
                weight: 1.0,
            },
        ]
    }

    #[test]
    fn flex_gives_each_its_minimum_and_splits_the_rest_by_weight() {
        assert_eq!(flex(150.0, &parts()).unwrap(), [100.0, 50.0]);
        let h = flex(190.0, &parts()).unwrap();
        assert!(
            (h[0] - 130.0).abs() < 1e-3 && (h[1] - 60.0).abs() < 1e-3,
            "{h:?}"
        );
    }

    #[test]
    fn flex_hands_what_a_capped_part_cannot_take_to_the_others() {
        // both stop at their maximum once there is room enough
        let h = flex(300.0, &parts()).unwrap();
        assert!(
            (h[1] - 80.0).abs() < 1e-3 && (h[0] - 200.0).abs() < 1e-3,
            "{h:?}"
        );
        // the heavier second part stops at 60, so the first takes what is left of the 100 spare
        let capped = [
            Flex {
                min: 100.0,
                max: 300.0,
                weight: 1.0,
            },
            Flex {
                min: 50.0,
                max: 60.0,
                weight: 3.0,
            },
        ];
        let h = flex(250.0, &capped).unwrap();
        assert!(
            (h[1] - 60.0).abs() < 1e-3 && (h[0] - 190.0).abs() < 1e-3,
            "{h:?}"
        );
    }

    #[test]
    fn flex_reports_when_the_minimums_do_not_fit() {
        assert!(flex(149.0, &parts()).is_none());
    }

    #[test]
    fn the_column_plan_tiles_the_column_top_down_without_overlap() {
        for (w, h) in SIZES {
            let column = at(w, h).centre.column;
            for guide in [true, false] {
                let p = plan_column(column, guide);
                let mut stack = vec![p.dossier, p.header, p.roster];
                stack.extend(p.guide);
                for pair in stack.windows(2) {
                    assert!(
                        pair[0].bottom() <= pair[1].top() + 0.01,
                        "{pair:?} at {w}x{h}"
                    );
                }
                for r in &stack {
                    assert!(
                        column.contains_rect(r.expand(-0.01)),
                        "{r:?} outside {column:?} at {w}x{h}"
                    );
                    assert_eq!(r.width(), column.width());
                }
            }
        }
    }

    /// Height of one tile in the roster of `plan`.
    fn tile_h(plan: &ColumnPlan) -> f32 {
        let (_, rows) = roster_grid(plan.roster.height());
        (plan.roster.height() - TILE_GAP * (rows as f32 - 1.0)) / rows as f32
    }

    #[test]
    fn the_smallest_window_still_gives_the_roster_tiles_of_at_least_30() {
        let p = plan_column(at(900.0, 700.0).centre.column, true);
        assert!(tile_h(&p) >= TILE_H_MIN - 0.01, "{}", tile_h(&p));
        assert_eq!(roster_grid(p.roster.height()), (6, 4));
        assert!(p.guide.is_none(), "no room for the cheat sheet at 900x700");
    }

    #[test]
    fn a_default_window_has_five_columns_of_tiles() {
        let p = plan_column(at(1060.0, 800.0).centre.column, true);
        assert_eq!(roster_grid(p.roster.height()), (5, 5));
        assert!(tile_h(&p) >= TILE_H_MIN);
    }

    #[test]
    fn a_banner_over_the_smallest_window_squeezes_the_tiles_a_little_but_keeps_them_readable() {
        let banner = regions(Vec2::new(900.0, 700.0), HEADER, 38.0, FOOTER);
        let p = plan_column(banner.centre.column, true);
        assert_eq!(roster_grid(p.roster.height()), (6, 4));
        assert!(tile_h(&p) >= 26.0, "{}", tile_h(&p));
    }

    #[test]
    fn the_roster_grid_holds_every_bunker() {
        for h in [100.0, 132.0, 166.0, 200.0, 400.0] {
            let (cols, rows) = roster_grid(h);
            assert!(cols * rows >= BUNKER_COUNT, "{h}: {cols}x{rows}");
            assert!((rows - 1) * cols < BUNKER_COUNT, "{h}: {cols}x{rows}");
        }
    }

    #[test]
    fn a_tall_window_adds_the_cheat_sheet_and_grows_the_tiles() {
        let small = plan_column(at(1060.0, 800.0).centre.column, true);
        let tall = plan_column(at(1920.0, 1080.0).centre.column, true);
        assert!(tall.guide.is_some());
        assert!(tall.roster.height() > small.roster.height());
        let g = tall.guide.unwrap().height();
        assert!((GUIDE_H_MIN..=GUIDE_H_MAX).contains(&g), "{g}");
    }

    #[test]
    fn a_hidden_cheat_sheet_leaves_its_height_to_the_roster() {
        let column = at(1920.0, 1080.0).centre.column;
        let with = plan_column(column, true);
        let without = plan_column(column, false);
        assert!(without.guide.is_none());
        assert!(without.roster.height() >= with.roster.height());
    }
}
