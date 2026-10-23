use std::collections::VecDeque;
use std::time::Instant;

use super::SAMPLE_RATE;

/// Windows delivers no packets while nothing plays, so real samples alone would squeeze every silence out of the
/// stream and bend the timing the decoder relies on. The pacer emits on the wall clock: real samples when there are
/// some, zeros when there are none.
pub struct Pacer {
    started: Instant,
    emitted: u64,
    pending: VecDeque<i16>,
}

/// Chunk size (10 ms) and the jitter buffer kept between the wall clock and the output (20 ms).
const CHUNK: usize = SAMPLE_RATE / 100;
const LATENCY: i64 = (SAMPLE_RATE / 50) as i64;
/// More than this much audio waiting for the wall clock means the device delivered a backlog (after a stall or a
/// suspended stream): keep the newest second. The pacer cannot see the engine; back-pressure is the sink's job.
const MAX_PENDING: usize = SAMPLE_RATE * 2;

impl Pacer {
    pub fn new(now: Instant) -> Pacer {
        Pacer {
            started: now,
            emitted: 0,
            pending: VecDeque::new(),
        }
    }

    pub fn push(&mut self, samples: impl IntoIterator<Item = i16>) {
        self.pending.extend(samples);
        if self.pending.len() > MAX_PENDING {
            let drop = self.pending.len() - SAMPLE_RATE;
            self.pending.drain(..drop);
        }
    }

    /// Whole chunks due at `now`.
    pub fn due(&mut self, now: Instant) -> Vec<i16> {
        let elapsed = now.saturating_duration_since(self.started).as_secs_f64();
        let target = (elapsed * SAMPLE_RATE as f64) as i64 - LATENCY;
        let mut out = Vec::new();
        while target - self.emitted as i64 >= CHUNK as i64 {
            for _ in 0..CHUNK {
                out.push(self.pending.pop_front().unwrap_or(0));
            }
            self.emitted += CHUNK as u64;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn silence_is_filled_on_the_wall_clock() {
        let t0 = Instant::now();
        let mut p = Pacer::new(t0);
        let out = p.due(t0 + Duration::from_secs(2));
        // two seconds minus the 20 ms jitter buffer, in whole 10 ms chunks
        assert_eq!(out.len(), (2 * SAMPLE_RATE - 882) / CHUNK * CHUNK);
        assert!(out.iter().all(|&s| s == 0));
    }

    #[test]
    fn real_samples_keep_their_order_and_gaps_become_zeros() {
        let t0 = Instant::now();
        let mut p = Pacer::new(t0);
        p.push(std::iter::repeat_n(7, 441));
        let out = p.due(t0 + Duration::from_millis(100));
        assert_eq!(&out[..441], &[7; 441][..]);
        assert!(out[441..].iter().all(|&s| s == 0));
    }

    #[test]
    fn a_backlog_from_the_device_keeps_the_newest_audio() {
        let mut p = Pacer::new(Instant::now());
        p.push(std::iter::repeat_n(1, SAMPLE_RATE * 5));
        assert_eq!(p.pending.len(), SAMPLE_RATE);
    }
}
