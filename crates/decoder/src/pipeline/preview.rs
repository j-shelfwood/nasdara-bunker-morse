use super::record::shown_digits;
use super::{OpenBurst, Pipeline, TRACE_MARGIN_S};
use crate::bunkers::{Bunker, candidates};
use crate::grid::{DIGITS_PER_RECORD, SHIPPED_TIMING, decode_prefix, longest_digit_s};
use crate::tone::FRAME_SECONDS;
use std::sync::{Mutex, MutexGuard};

/// A preview needs this many tone runs in the open burst; fewer is a stray tone.
const PREVIEW_MIN_RUNS: usize = 6;
/// Recompute the preview only after this many new frames.
/// The decoder's jitter allowance: a digit counts as settled only when the trace extends this far past its window.
const SETTLE_S: f64 = 0.12;
/// A closed burst's preview is dropped after this long even if its burst never arrives.
pub(super) const PREVIEW_HOLD_S: f64 = 3.0;
const PREVIEW_REFRESH_FRAMES: u64 = 60;
/// The preview decodes at most this many seconds of the open burst (the newest): a message is about 90 s.
const PREVIEW_MAX_S: f64 = 100.0;

/// One digit of a live preview. A digit whose last symbol window is still open (`settled == false`) may change on the
/// next preview: a shorter look-alike that fits the audio so far can take its place until the window has passed.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewDigit {
    /// '?' when the support is below `MIN_SUPPORT`, except in a grid that is exactly a real bunker (`shown_digits`).
    pub ch: char,
    pub support: f32,
    pub margin: f32,
    /// Stream seconds of the digit start.
    pub start: f64,
    /// Stream seconds at which the digit's last symbol window ended.
    pub window_end: f64,
    /// False while the digit may still change in later previews: only the trailing digit can be unsettled, and it
    /// is until the trace extends past `window_end` plus the jitter allowance (`SETTLE_S`) and past the longest
    /// digit's length from the digit's start, because a shorter look-alike fits the audio first (4821: the fifth digit
    /// reads 5, 6, 4, 7, 8, 9 before the real 0). A UI should not present an unsettled digit as read.
    pub settled: bool,
}

/// Digits read from the transmission in progress. Digits with `settled == false` (only ever the last) may change.
#[derive(Debug, Clone, PartialEq)]
pub struct Preview {
    /// Stream seconds of the first digit.
    pub start: f64,
    pub digits: Vec<PreviewDigit>,
    /// Bunkers whose grid fits the trustworthy grid digits so far: settled digits at or above `MIN_SUPPORT`
    /// (an unsettled or weak digit counts as unheard). Empty when the heard digits fit no bunker.
    pub candidates: Vec<&'static Bunker>,
}

#[derive(Debug)]
pub(super) struct PreviewCache {
    id: u64,
    at_frame: u64,
    value: Option<Preview>,
}

pub(super) fn preview_max_frames() -> usize {
    (PREVIEW_MAX_S / FRAME_SECONDS).ceil() as usize
}

fn unpoison<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Pipeline {
    /// Digits read from the transmission in progress so far. `None` outside a transmission or before it has
    /// `PREVIEW_MIN_RUNS` tone runs. After the transmission closes the last preview is kept until the push after
    /// its `Burst` was emitted. Cached: recomputed after `PREVIEW_REFRESH_FRAMES` new frames.
    pub fn preview(&self) -> Option<Preview> {
        let open = self
            .open
            .as_ref()
            .or(self.closing.as_ref().map(|c| &c.open))
            .filter(|o| o.on_runs >= PREVIEW_MIN_RUNS)?;
        let mut cache = unpoison(&self.cache);
        if let Some(c) = cache.as_ref()
            && c.id == open.id
            && self.frames_total - c.at_frame < PREVIEW_REFRESH_FRAMES
        {
            return c.value.clone();
        }
        let value = self.compute_preview(open);
        *cache = Some(PreviewCache {
            id: open.id,
            at_frame: self.frames_total,
            value: value.clone(),
        });
        value
    }

    fn compute_preview(&self, open: &OpenBurst) -> Option<Preview> {
        let margin = (TRACE_MARGIN_S / FRAME_SECONDS).ceil() as usize;
        let first = open.first_frame.saturating_sub(self.levels_base) as usize;
        let from = first
            .saturating_sub(margin)
            .max(self.levels.len().saturating_sub(preview_max_frames()));
        let trace = self.levels.get(from..)?;
        let t0 = self.level_time(from);
        let g = decode_prefix(trace, &SHIPPED_TIMING, DIGITS_PER_RECORD)?;
        let trace_end = self.levels_end;
        let longest = longest_digit_s(&SHIPPED_TIMING);
        let n = g.digits.len();
        let shown = shown_digits(&g.digits, &g.support);
        let digits: Vec<PreviewDigit> = (0..n)
            .map(|i| PreviewDigit {
                ch: shown[i],
                support: g.support[i],
                margin: g.margins[i],
                start: t0 + g.starts[i],
                window_end: t0 + g.ends[i],
                settled: i + 1 < n
                    || (t0 + g.ends[i] + SETTLE_S <= trace_end
                        && t0 + g.starts[i] + longest + SETTLE_S <= trace_end),
            })
            .collect();
        let trusted: Vec<char> = digits
            .iter()
            .take(6)
            .map(|d| if d.settled { d.ch } else { '?' })
            .collect();
        Some(Preview {
            start: t0 + g.start,
            candidates: candidates(&trusted),
            digits,
        })
    }
}
