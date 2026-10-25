//! group 6: the ledger votes in f64 like the prototype, and its loose map is bounded

use nasdara_decoder::bunkers::bunkers;
use nasdara_decoder::bunkers::snap_grid;
use nasdara_decoder::bursts::BunkerRecord;
use nasdara_decoder::ledger::BunkerLedger;

fn code_record(bunker: usize, code_digit: char, support: f32) -> BunkerRecord {
    let grid = bunkers()[bunker].grid.clone();
    let mut digits: Vec<char> = grid.chars().collect();
    digits.extend([code_digit, '?', '?', '?']);
    let mut sup = vec![1.0; 6];
    sup.extend([support, 0.0, 0.0, 0.0]);
    BunkerRecord {
        code: digits[6..].iter().collect(),
        digits,
        support: sup,
        raw: vec![],
        margins: vec![],
        snap: snap_grid(&grid),
        grid,
        start: 0.0,
        digit_starts: vec![],
    }
}

/// `n` hundredths as the f32 the decoder reports (`Number(s.toFixed(2))`).
fn hundredths(n: u32) -> f32 {
    format!("{}.{:02}", n / 100, n % 100).parse().unwrap_or(0.0)
}

#[test]
fn exact_ratio_ties_agree_with_an_f64_reference() {
    let mut mismatches = Vec::new();
    let mut pairs = 0;
    // below 0.35 the leader is too weak to show at all (`weak_passes_accumulate_until_a_digit_shows`)
    for top in 35..=300u32 {
        for second in 1..=top {
            pairs += 1;
            let mut l = BunkerLedger::new();
            l.add(&code_record(3, '1', hundredths(top)));
            let e = l.add(&code_record(3, '2', hundredths(second)));
            let (t, s) = (f64::from(top) / 100.0, f64::from(second) / 100.0);
            let want = if s > 0.7 * t { '?' } else { '1' };
            let got = e.code.chars().next().unwrap_or('!');
            if got != want {
                mismatches.push((top, second, got, want));
            }
        }
    }
    assert!(pairs > 40_000, "{pairs}");
    assert!(
        mismatches.is_empty(),
        "{} {:?}",
        mismatches.len(),
        &mismatches[..mismatches.len().min(5)]
    );
}

/// A pass the pipeline showed as '?' keeps its raw digit in `raw`: alone it is too weak to show, three agreeing
/// passes are not, a majority wins, and an even split leaves the position unreadable.
#[test]
fn weak_passes_accumulate_until_a_digit_shows() {
    let weak = |digit: char, support: f32| {
        let mut r = code_record(3, '?', 0.0);
        r.raw = r.digits.clone();
        r.raw[6] = digit;
        r.support[6] = support;
        r
    };
    let mut l = BunkerLedger::new();
    assert_eq!(l.add(&weak('7', 0.15)).code.chars().next(), Some('?'));
    assert_eq!(l.add(&weak('7', 0.15)).code.chars().next(), Some('?'));
    assert_eq!(l.add(&weak('7', 0.15)).code.chars().next(), Some('7'));
    let mut l = BunkerLedger::new();
    l.add(&weak('7', 0.2));
    l.add(&weak('7', 0.2));
    assert_eq!(l.add(&weak('3', 0.2)).code.chars().next(), Some('7'));
    l.add(&weak('3', 0.2));
    assert_eq!(l.add(&weak('3', 0.2)).code.chars().next(), Some('3'));
    let mut l = BunkerLedger::new();
    l.add(&weak('7', 0.2));
    l.add(&weak('7', 0.2));
    l.add(&weak('3', 0.2));
    assert_eq!(l.add(&weak('3', 0.2)).code.chars().next(), Some('?'));
    let mut l = BunkerLedger::new();
    assert_eq!(l.add(&weak('7', -0.3)).code.chars().next(), Some('?'));
}

#[test]
fn loose_records_are_capped_and_the_oldest_is_dropped() {
    let loose = |n: usize| BunkerRecord {
        digits: "??????".chars().chain(format!("{n:04}").chars()).collect(),
        support: vec![1.0; 10],
        raw: vec![],
        margins: vec![],
        grid: "??????".into(),
        code: format!("{n:04}"),
        snap: snap_grid("??????"),
        start: 0.0,
        digit_starts: vec![],
    };
    assert!(loose(0).snap.bunker().is_none());
    let mut l = BunkerLedger::new();
    for n in 0..300 {
        assert_eq!(l.add(&loose(n)).count, 1);
    }
    assert_eq!(l.add(&loose(299)).count, 2, "a recent entry is kept");
    assert_eq!(l.add(&loose(0)).count, 1, "the oldest entry was dropped");
}
