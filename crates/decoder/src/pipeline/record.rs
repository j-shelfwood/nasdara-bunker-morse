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
/// from it, or the reason there is none.
///
/// A digit below `MIN_SUPPORT` shows as '?', except in a grid that is exactly one of the 23 bunkers: six digits
/// matching a real cell by chance is about 2 in 100 000, so such a grid is a checksum that outweighs the weak tone
/// evidence of its digits (a quiet or noisy game gave supports of 0.04 to 0.28 on grids that were all right).
pub fn read_trace(trace: &Trace) -> (Option<Guess>, Result<BunkerRecord, String>) {
    let Some(grid) = decode_grid(&trace.db, DIGITS_PER_RECORD, &SHIPPED_TIMING) else {
        return (None, Err("too short for a whole message".into()));
    };
    let guess = Guess {
        digits: grid.digits.iter().collect(),
        support: grid.support.iter().map(|&s| round2(s)).collect(),
    };
    let mut digits: Vec<char> = grid
        .digits
        .iter()
        .zip(&grid.support)
        .map(|(&d, &s)| if s >= MIN_SUPPORT { d } else { '?' })
        .collect();
    let raw_grid: String = grid.digits.iter().take(6).collect();
    if matches!(snap_grid(&raw_grid), Snap::Exact(_)) {
        digits[..6].copy_from_slice(&grid.digits[..6]);
    }
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

/// `Number(s.toFixed(2))`
/// Never returns negative zero.
fn round2(s: f32) -> f32 {
    let r: f32 = format!("{s:.2}").parse().unwrap_or(s);
    if r == 0.0 { 0.0 } else { r }
}

#[cfg(test)]
mod tests {
    use super::round2;

    #[test]
    fn round2_never_returns_negative_zero() {
        assert_eq!(round2(-0.001).to_bits(), 0.0f32.to_bits());
        assert_eq!(round2(-0.0).to_bits(), 0.0f32.to_bits());
        assert_eq!(round2(1.0549), 1.05);
        assert_eq!(round2(-0.4), -0.4);
    }
}
