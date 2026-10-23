//! The 23 bunker proxy positions of Nasdara and the map grid they fall in.

use crate::js_round;
use std::sync::OnceLock;

/// The 23 bunker proxy positions [x, y, z] of Nasdara (game metres), in the order of the server config.
pub const POSITIONS: [[f64; 3]; 23] = [
    [7216.415039000014, 185.243851, 5706.95459],
    [9167.879882999987, 211.157135, 11945.569336],
    [11203.510742000013, 176.846359, 9662.226563],
    [6797.303710999986, 244.669296, 10515.37793],
    [10742.790039000014, 150.632004, 4027.940186],
    [7630.182617000013, 248.902024, 8752.492188],
    [11494.507813000004, 165.301834, 11022.930664],
    [10419.642577999999, 168.087982, 8056.37793],
    [4584.328125, 265.411163, 10884.116211],
    [10087.624023000011, 152.086868, 4239.237305],
    [10959.40625, 165.977493, 7783.450195],
    [7381.81005900001, 263.272827, 7383.055664],
    [10555.939452999999, 163.533524, 6135.306641],
    [8830.352539000014, 159.339523, 5132.939453],
    [8541.851563000004, 166.770554, 4795.474609],
    [8741.5625, 181.255325, 7560.641113],
    [13323.453125, 241.828705, 14238.014648],
    [3861.5620120000094, 333.834839, 9775.362305],
    [6559.7690430000075, 246.380325, 7361.540039],
    [9633.549804999988, 166.527985, 5440.727539],
    [2863.215820000012, 436.36615, 6381.550781],
    [5719.367188000004, 298.773956, 8969.555664],
    [6713.5434569999925, 252.133988, 3980.503418],
];

// Grid word as the scheduler builds it: three digits of X then three of Z (BunkerBroadcastScheduler.c:200-218),
// the digits of the in-game map grid cell. Rule taken from iZurvive's Nasdara config (map 16384 x 16384 m,
// gps.flipYAxis true) and checked on its render: bunker 1 (x 7216, z 5707) lies in column 072, row 106, at the
// pixel offset a 100 m cell predicts. So X = floor(x / 100) and the row counts down from the north edge:
// Z = floor((16384 - z) / 100). Inferred that the in-game grid equals iZurvive's; the step (100) is not read
// from the world config (CfgWorlds Nasdara Grid Zoom1 stepX).
const STEP: f64 = 100.0;
pub const MAP_SIZE: f64 = 16384.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Bunker {
    /// 1-based position in the server config.
    pub index: usize,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub grid: String,
}

fn pad3(n: f64) -> String {
    format!("{:03}", (n / STEP).floor().abs() as u64)
}

pub fn grid_of(x: f64, z: f64) -> String {
    pad3(x) + &pad3(MAP_SIZE - z)
}

pub fn bunkers() -> &'static [Bunker] {
    static ALL: OnceLock<Vec<Bunker>> = OnceLock::new();
    ALL.get_or_init(|| {
        POSITIONS
            .iter()
            .enumerate()
            .map(|(i, &[x, y, z])| Bunker {
                index: i + 1,
                x,
                y,
                z,
                grid: grid_of(x, z),
            })
            .collect()
    })
}

const URL_PREFIX: &str = "https://www.izurvive.com/nasdara/#location=";

/// Allow-list for links the window opens in the browser:
/// exactly `https://www.izurvive.com/nasdara/#location=<1-5 digits>;<1-5 digits>;<1-2 digits>`.
pub fn is_izurvive_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix(URL_PREFIX) else {
        return false;
    };
    let parts: Vec<&str> = rest.split(';').collect();
    let [x, z, zoom] = parts[..] else {
        return false;
    };
    let digits =
        |s: &str, max: usize| (1..=max).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit());
    digits(x, 5) && digits(z, 5) && digits(zoom, 2)
}

/// iZurvive centres its Nasdara map on in-game x;z (loc1;loc2, read from its client code and confirmed on a render:
/// the coordinate panel showed "Game: 7216/5707").
pub fn izurvive_url(x: f64, z: f64, zoom: u32) -> String {
    format!(
        "{URL_PREFIX}{};{};{zoom}",
        js_round(x) as i64,
        js_round(z) as i64
    )
}

/// Centre of a grid cell in game metres (for a grid that matches no known bunker).
pub fn cell_center(grid: &str) -> Option<(f64, f64)> {
    if grid.len() != 6 || !grid.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let col: f64 = grid[..3].parse().ok()?;
    let row: f64 = grid[3..].parse().ok()?;
    Some((
        col * STEP + STEP / 2.0,
        MAP_SIZE - (row * STEP + STEP / 2.0),
    ))
}

/// Mismatches on the digits that were heard; '?' (an unreadable digit) matches anything.
fn mismatches(a: &[char], b: &str) -> usize {
    a.iter()
        .zip(b.chars())
        .filter(|&(&c, d)| c != '?' && c != d)
        .count()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapKind {
    Exact,
    Inferred,
    Corrected,
    None,
}

impl SnapKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SnapKind::Exact => "exact",
            SnapKind::Inferred => "inferred",
            SnapKind::Corrected => "corrected",
            SnapKind::None => "none",
        }
    }
}

/// `Exact`: all six digits heard and equal. `Inferred`: some digits unreadable, one bunker fits the rest.
/// `Corrected`: one heard digit disagrees, one bunker fits the others.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Snap {
    Exact(&'static Bunker),
    Inferred(&'static Bunker),
    Corrected(&'static Bunker),
    None,
}

impl Snap {
    pub fn kind(&self) -> SnapKind {
        match self {
            Snap::Exact(_) => SnapKind::Exact,
            Snap::Inferred(_) => SnapKind::Inferred,
            Snap::Corrected(_) => SnapKind::Corrected,
            Snap::None => SnapKind::None,
        }
    }

    pub fn bunker(&self) -> Option<&'static Bunker> {
        match self {
            Snap::Exact(b) | Snap::Inferred(b) | Snap::Corrected(b) => Some(b),
            Snap::None => None,
        }
    }
}

const MIN_HEARD_FOR_INFERRED: usize = 4;
const MIN_HEARD_FOR_CORRECTED: usize = 5;

pub fn snap_grid(grid: &str) -> Snap {
    let chars: Vec<char> = grid.chars().collect();
    if chars.len() != 6 {
        return Snap::None;
    }
    let heard = chars.iter().filter(|&&c| c != '?').count();
    let fit: Vec<&'static Bunker> = bunkers()
        .iter()
        .filter(|b| mismatches(&chars, &b.grid) == 0)
        .collect();
    if let [b] = fit[..] {
        if heard == 6 {
            return Snap::Exact(b);
        }
        if heard >= MIN_HEARD_FOR_INFERRED {
            return Snap::Inferred(b);
        }
    }
    if heard < MIN_HEARD_FOR_CORRECTED {
        return Snap::None;
    }
    let near: Vec<&'static Bunker> = bunkers()
        .iter()
        .filter(|b| mismatches(&chars, &b.grid) <= 1)
        .collect();
    match near[..] {
        [b] => Snap::Corrected(b),
        _ => Snap::None,
    }
}
