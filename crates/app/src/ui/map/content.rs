//! What a pin says about its bunker: the hover card rows and the screen reader sentence.

use crate::engine::Card;
use crate::theme::{AMBER, DIM, RED, TEXT};
use crate::ui::bunkers::{Reading, reading, reading_color};
use egui::Color32;
use nasdara_decoder::bunkers::Bunker;

/// One run of text on a card row.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Part {
    pub text: String,
    pub color: Color32,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// Small bold caption.
    Caption,
    /// Digits in the numeric face.
    Value,
    /// One lock digit on a fixed advance.
    Lock,
    /// Plain dim sentence.
    Note,
}

pub(super) struct Content {
    pub heard: bool,
    pub reading: Option<Reading>,
    pub header: Vec<Part>,
    pub rows: Vec<Vec<Part>>,
}

fn part(text: impl Into<String>, color: Color32, kind: Kind) -> Part {
    Part {
        text: text.into(),
        color,
        kind,
    }
}

/// `123456` as `123 456`; anything else (a partial grid with `?`) is shown as it is.
pub(super) fn grid_spaced(grid: &str) -> String {
    if grid.len() == 6 {
        format!("{} {}", &grid[..3], &grid[3..])
    } else {
        grid.to_string()
    }
}

/// Game coordinates in metres, rounded.
pub(super) fn metres(b: &Bunker) -> (i64, i64) {
    (b.x.round() as i64, b.z.round() as i64)
}

pub(super) fn build(b: &Bunker, card: Option<&Card>) -> Content {
    let (x, z) = metres(b);
    let coords = vec![
        part("X", DIM, Kind::Caption),
        part(x.to_string(), TEXT, Kind::Value),
        part("Z", DIM, Kind::Caption),
        part(z.to_string(), TEXT, Kind::Value),
    ];
    let name = part(format!("BUNKER {:02}", b.index), TEXT, Kind::Caption);
    let Some(c) = card else {
        let lock = (0..4)
            .map(|_| part("-", DIM, Kind::Lock))
            .collect::<Vec<_>>();
        return Content {
            heard: false,
            reading: None,
            header: vec![name, part("NOT HEARD", DIM, Kind::Caption)],
            rows: vec![
                row("GRID", grid_spaced(&b.grid), TEXT),
                [vec![part("LOCK", DIM, Kind::Caption)], lock].concat(),
                coords,
                vec![part("No code yet", DIM, Kind::Note)],
            ],
        };
    };
    let lock = c
        .code
        .chars()
        .map(|ch| match ch {
            '?' => part("?", RED, Kind::Lock),
            d => part(d.to_string(), AMBER, Kind::Lock),
        })
        .collect::<Vec<_>>();
    let r = reading(c);
    Content {
        heard: true,
        reading: Some(r),
        header: vec![name],
        rows: vec![
            row("GRID", grid_spaced(&c.grid), TEXT),
            [vec![part("LOCK", DIM, Kind::Caption)], lock].concat(),
            coords,
            vec![part(
                format!("heard {}x", c.count),
                reading_color(r),
                Kind::Note,
            )],
        ],
    }
}

fn row(label: &str, value: String, color: Color32) -> Vec<Part> {
    vec![
        part(label, DIM, Kind::Caption),
        part(value, color, Kind::Value),
    ]
}

/// What a screen reader says for a pin.
pub(super) fn spoken(b: &Bunker, card: Option<&Card>) -> String {
    let (x, z) = metres(b);
    let coords = format!("coordinates x {x} z {z}");
    let Some(c) = card else {
        return format!(
            "Bunker {}, not heard yet, grid {}, {coords}",
            b.index,
            grid_spaced(&b.grid)
        );
    };
    let lock = c
        .code
        .chars()
        .map(|ch| {
            if ch == '?' {
                "unclear".into()
            } else {
                ch.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let times = if c.count == 1 {
        "heard 1 time".to_string()
    } else {
        format!("heard {} times", c.count)
    };
    format!(
        "Bunker {}, lock code {lock}, grid {}, {times}, {coords}",
        b.index,
        grid_spaced(&c.grid).replace('?', "unclear"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use nasdara_decoder::bunkers::bunkers;
    use std::time::Instant;

    fn bunker(n: usize) -> &'static Bunker {
        bunkers().iter().find(|b| b.index == n).expect("bunker")
    }

    pub fn card(n: usize, code: &str, count: usize) -> Card {
        let b = bunker(n);
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

    #[test]
    fn spoken_names_lock_grid_count_and_metres() {
        let b = bunker(7);
        let s = spoken(b, Some(&card(7, "48?1", 3)));
        let (x, z) = metres(b);
        let grid = grid_spaced(&b.grid);
        assert_eq!(
            s,
            format!(
                "Bunker 7, lock code 4 8 unclear 1, grid {grid}, heard 3 times, coordinates x {x} z {z}"
            )
        );
    }

    #[test]
    fn spoken_unheard_has_no_lock() {
        let s = spoken(bunker(7), None);
        assert!(s.starts_with("Bunker 7, not heard yet, grid "), "{s}");
        assert!(s.contains("coordinates x ") && !s.contains("lock"), "{s}");
    }

    #[test]
    fn unclear_digit_is_red_and_unheard_lock_is_dashes() {
        let c = build(bunker(7), Some(&card(7, "48?1", 1)));
        let lock = &c.rows[1];
        assert_eq!(lock[3].text, "?");
        assert_eq!(lock[3].color, RED);
        let u = build(bunker(7), None);
        assert_eq!(u.header[1].text, "NOT HEARD");
        assert!(u.rows[1][1..].iter().all(|p| p.text == "-"));
        assert_eq!(u.rows[3][0].text, "No code yet");
    }
}
