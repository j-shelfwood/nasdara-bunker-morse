//! Decodes traces rebuilt from beep lengths and gaps measured on a live Nasdara broadcast (2026-10-23, system audio).
//! The fixture holds numbers only: one burst per line as `lengths;gaps`, in seconds. Bursts that start or end mid-message
//! are partial captures; the full ones are bunker 16 (0870881291) and bunker 17 (1330210169).

use nasdara_decoder::bunkers::{SnapKind, snap_grid};
use nasdara_decoder::grid::{SHIPPED_TIMING, decode_grid};
use nasdara_decoder::tone::FRAME_SECONDS;

const FIXTURE: &str = include_str!("fixtures/live-2026-10-23.txt");
const ON_DB: i8 = 25;
const OFF_DB: i8 = -3;

fn trace(line: &str) -> Vec<i8> {
    let (durs, gaps) = line.split_once(';').expect("lengths;gaps");
    let nums = |s: &str| {
        s.split(',')
            .map(|v| v.parse::<f64>().expect("number"))
            .collect::<Vec<_>>()
    };
    let (durs, gaps) = (nums(durs), nums(gaps));
    let mut db = vec![OFF_DB; (2.0 / FRAME_SECONDS) as usize];
    let mut t = 2.0;
    for (d, g) in durs.iter().zip(&gaps) {
        t += g;
        let (a, b) = (
            (t / FRAME_SECONDS) as usize,
            ((t + d) / FRAME_SECONDS) as usize,
        );
        if db.len() < b + 1 {
            db.resize(b + 1, OFF_DB);
        }
        db[a..b].fill(ON_DB);
        t += d;
    }
    db.resize(db.len() + (3.0 / FRAME_SECONDS) as usize, OFF_DB);
    db
}

/// Lock code known from the live session, per bunker index.
const KNOWN: [(usize, &str, &str); 2] = [(16, "087088", "1291"), (17, "133021", "0169")];

#[test]
fn live_bursts_read_real_bunkers_without_a_wrong_digit() {
    // (beeps in the burst, bunker it must snap to, readable digits at least)
    let expected = [(59, 17, 7), (50, 19, 7), (50, 16, 7)];
    let mut seen = Vec::new();
    for line in FIXTURE.lines() {
        let elements = line.split(';').next().unwrap().split(',').count();
        let Some(&(_, bunker, min_readable)) = expected
            .iter()
            .find(|e| e.0 == elements && !seen.contains(&(e.0, e.1)))
        else {
            continue;
        };
        let g = decode_grid(&trace(line), 10, &SHIPPED_TIMING).expect("decodes");
        let digits: Vec<char> = g
            .digits
            .iter()
            .zip(&g.support)
            .map(|(d, s)| if *s >= 0.35 { *d } else { '?' })
            .collect();
        let text: String = digits.iter().collect();
        let snap = snap_grid(&text[..6]);
        assert_ne!(snap.kind(), SnapKind::None, "{text} did not snap");
        assert_eq!(snap.bunker().map(|b| b.index), Some(bunker), "{text}");
        assert!(
            digits.iter().filter(|c| **c != '?').count() >= min_readable,
            "{text} has too few readable digits"
        );
        if let Some((_, grid, code)) = KNOWN.iter().find(|k| k.0 == bunker) {
            let truth = format!("{grid}{code}");
            assert!(
                digits
                    .iter()
                    .zip(truth.chars())
                    .all(|(d, t)| *d == '?' || *d == t),
                "{text} contradicts {truth}"
            );
        }
        seen.push((elements, bunker));
    }
    assert_eq!(seen.len(), expected.len());
}
