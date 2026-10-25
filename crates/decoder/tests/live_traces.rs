//! In-game records of 2026-10-23 replayed through the grid decoder. Fixture: tone levels only, one record per line.
//! Before the digit-start window reached 0.45 s these read 4 to 5 `?` each: a digit gap cut short by 0.4 s (two complete
//! digits 1.89 s apart where 2.3 s is nominal) shifted every later digit off its pattern.

use nasdara_decoder::bunkers::{SnapKind, snap_grid};
use nasdara_decoder::bursts::Trace;
use nasdara_decoder::grid::{DIGITS_PER_RECORD, SHIPPED_TIMING, decode_grid};
use nasdara_decoder::pipeline::{MIN_SUPPORT, read_trace};

const FIXTURE: &str = include_str!("fixtures/live-traces-2026-10-23.txt");

fn levels(rle: &str) -> Vec<i8> {
    let mut db = Vec::new();
    for token in rle.split(',') {
        let (v, n) = token.split_once('x').unwrap_or((token, "1"));
        let n: usize = n.parse().expect("count");
        db.resize(db.len() + n, v.parse().expect("level"));
    }
    db
}

/// (digits read, least number of readable digits, bunker the grid must snap to) per fixture line.
const EXPECTED: [(usize, usize); 3] = [(9, 13), (10, 5), (8, 10)];

#[test]
fn in_game_records_read_their_bunker_without_a_wrong_digit() {
    let lines: Vec<&str> = FIXTURE.lines().filter(|l| !l.starts_with('#')).collect();
    assert_eq!(lines.len(), EXPECTED.len());
    for (line, (min_readable, bunker)) in lines.iter().zip(EXPECTED) {
        let (truth, trace) = line.split_once(';').expect("digits;trace");
        let g = decode_grid(&levels(trace), DIGITS_PER_RECORD, &SHIPPED_TIMING).expect("decodes");
        let shown: Vec<char> = g
            .digits
            .iter()
            .zip(&g.support)
            .map(|(d, s)| if *s >= MIN_SUPPORT { *d } else { '?' })
            .collect();
        let text: String = shown.iter().collect();
        assert!(
            shown
                .iter()
                .zip(truth.chars())
                .all(|(d, t)| *d == '?' || t == '?' || *d == t),
            "{text} contradicts {truth}"
        );
        assert!(
            shown.iter().filter(|c| **c != '?').count() >= min_readable,
            "{text} has too few readable digits"
        );
        let grid: String = g.digits[..6].iter().collect();
        let snap = snap_grid(&grid);
        assert_ne!(snap.kind(), SnapKind::None, "{grid} did not snap");
        assert_eq!(snap.bunker().map(|b| b.index), Some(bunker), "{text}");
    }
}

const QUIET: &str = include_str!("fixtures/live-traces-2026-10-26.txt");

/// Two records heard with the game volume turned down: every digit's support fell below `MIN_SUPPORT`, so the
/// pipeline reported nothing, yet the decoder had read both grids right. A grid that is exactly a real bunker now
/// stands without the support.
#[test]
fn quiet_game_records_report_their_exact_grid() {
    let lines: Vec<&str> = QUIET.lines().filter(|l| !l.starts_with('#')).collect();
    assert_eq!(lines.len(), 2);
    for (line, bunker) in lines.iter().zip([20, 4]) {
        let (truth, trace) = line.split_once(';').expect("digits;trace");
        let trace = Trace {
            t0: 0.0,
            db: levels(trace),
            rms_db: Vec::new(),
        };
        let (guess, record) = read_trace(&trace);
        let guess = guess.expect("a reading was made");
        assert!(guess.support.iter().all(|&s| s < MIN_SUPPORT), "{guess:?}");
        let record = record.expect("an exact grid makes a record");
        assert_eq!(record.grid, truth[..6], "{guess:?}");
        assert_eq!(record.snap.kind(), SnapKind::Exact);
        assert_eq!(record.snap.bunker().map(|b| b.index), Some(bunker));
        assert_eq!(record.code, "????");
        assert_eq!(record.raw.iter().collect::<String>(), guess.digits);
    }
}
