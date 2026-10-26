use crate::bunkers::{Snap, snap_grid};
use crate::bursts::{BunkerRecord, Guess, Trace};
use crate::grid::{DIGITS_PER_RECORD, SHIPPED_TIMING, decode_grid};

/// Below this a digit is reported as '?': the beeps that would decide it were not (all) heard.
pub const MIN_SUPPORT: f32 = 0.35;
/// A record with fewer readable digits than this is not reported (a partial capture).
const MIN_READABLE: usize = 6;

pub(super) fn rms_dbfs(rms: f32) -> i8 {
    (20.0 * f64::from(rms).max(1e-5).log10())
        .round()
        .clamp(-100.0, 0.0) as i8
}

/// Reads one burst's tone trace: the raw grid reading (when the trace is long enough for one) and the record built
/// from it, or the reason there is none. Digits show as `shown_digits` decides.
pub fn read_trace(trace: &Trace) -> (Option<Guess>, Result<BunkerRecord, String>) {
    let Some(grid) = decode_grid(&trace.db, DIGITS_PER_RECORD, &SHIPPED_TIMING) else {
        return (None, Err("too short for a whole message".into()));
    };
    let guess = Guess {
        digits: grid.digits.iter().collect(),
        support: grid.support.iter().map(|&s| round2(s)).collect(),
    };
    let digits = shown_digits(&grid.digits, &grid.support);
    let readable = digits.iter().filter(|&&d| d != '?').count();
    if readable < MIN_READABLE {
        let note = format!("only {readable} of {DIGITS_PER_RECORD} digits readable");
        return (Some(guess), Err(note));
    }
    let g: String = digits.iter().take(6).collect();
    let record = BunkerRecord {
        snap: snap_grid(&g),
        code: digits.iter().skip(6).collect(),
        grid: g,
        raw: grid.digits.clone(),
        support: grid.support.iter().map(|&s| round2(s)).collect(),
        margins: grid.margins.clone(),
        start: trace.t0 + grid.start,
        digit_starts: grid.starts.iter().map(|s| trace.t0 + s).collect(),
        digits,
    };
    (Some(guess), Ok(record))
}

/// The digits to show: '?' below `MIN_SUPPORT`, except in a grid that is exactly one of the 23 bunkers. Six digits
/// matching a real cell by chance is about 2 in 100 000, so such a grid is a checksum that outweighs the weak tone
/// evidence of its digits (a quiet or noisy game gave supports of 0.04 to 0.28 on grids that were all right).
/// The final record and the live preview both go through here, so they cannot disagree.
pub(super) fn shown_digits(digits: &[char], support: &[f32]) -> Vec<char> {
    let mut shown: Vec<char> = digits
        .iter()
        .zip(support)
        .map(|(&d, &s)| if s >= MIN_SUPPORT { d } else { '?' })
        .collect();
    if let Some(grid) = digits.get(..6)
        && matches!(snap_grid(&grid.iter().collect::<String>()), Snap::Exact(_))
    {
        shown[..6].copy_from_slice(grid);
    }
    shown
}

/// `Number(s.toFixed(2))`
/// Never returns negative zero.
fn round2(s: f32) -> f32 {
    let r: f32 = format!("{s:.2}").parse().unwrap_or(s);
    if r == 0.0 { 0.0 } else { r }
}

#[cfg(test)]
mod tests {
    use super::{round2, shown_digits};
    use crate::bunkers::bunkers;

    #[test]
    fn round2_never_returns_negative_zero() {
        assert_eq!(round2(-0.001).to_bits(), 0.0f32.to_bits());
        assert_eq!(round2(-0.0).to_bits(), 0.0f32.to_bits());
        assert_eq!(round2(1.0549), 1.05);
        assert_eq!(round2(-0.4), -0.4);
    }

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn a_weak_grid_that_is_a_real_bunker_stays_and_its_code_does_not() {
        let grid = &bunkers()[19].grid;
        let digits = chars(&format!("{grid}7596"));
        let shown = shown_digits(&digits, &[0.1; 10]);
        assert_eq!(shown.iter().collect::<String>(), format!("{grid}????"));
    }

    #[test]
    fn a_weak_grid_that_is_no_bunker_is_masked() {
        let shown = shown_digits(&chars("1234567890"), &[0.1; 10]);
        assert_eq!(shown, vec!['?'; 10]);
    }

    #[test]
    fn digits_at_or_above_the_cut_off_show_and_a_short_preview_has_no_grid_to_check() {
        let shown = shown_digits(&chars("12345"), &[0.9, 0.35, 0.34, 0.9, -0.2]);
        assert_eq!(shown.iter().collect::<String>(), "12?4?");
    }
}
