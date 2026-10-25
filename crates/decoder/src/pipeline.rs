//! PCM (s16le mono 44.1 kHz) in, decoded bursts out.

use crate::bunkers::{Snap, snap_grid};
use crate::bursts::{AssemblerOptions, BunkerRecord, Burst, BurstAssembler, Guess, Trace};
use crate::grid::{DIGITS_PER_RECORD, SHIPPED_TIMING, decode_grid, decode_prefix, longest_digit_s};
use crate::js_round;
use crate::runs::{RunOptions, RunTracker};
use crate::tone::{FRAME_SECONDS, ToneDetector, ToneFrame};
use std::sync::{Mutex, MutexGuard};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Live {
    pub on: bool,
    pub db: f32,
}

/// One second of audio: the most `push` analyses before it trims the trace and decodes finished bursts.
const PUSH_STEP_SAMPLES: usize = 44_100;
const TRACE_KEEP_S: f64 = 150.0;
const TRACE_MARGIN_S: f64 = 2.0;
/// Below this a digit is reported as '?': the beeps that would decide it were not (all) heard.
pub const MIN_SUPPORT: f32 = 0.35;
/// A record with fewer readable digits than this is not reported (a partial capture).
const MIN_READABLE: usize = 6;
/// A preview needs this many tone runs in the open burst; fewer is a stray tone.
const PREVIEW_MIN_RUNS: usize = 6;
/// Recompute the preview only after this many new frames.
/// The decoder's jitter allowance: a digit counts as settled only when the trace extends this far past its window.
const SETTLE_S: f64 = 0.12;
/// A closed burst's preview is dropped after this long even if its burst never arrives.
const PREVIEW_HOLD_S: f64 = 3.0;
const PREVIEW_REFRESH_FRAMES: u64 = 60;
/// The preview decodes at most this many seconds of the open burst (the newest): a message is about 90 s.
const PREVIEW_MAX_S: f64 = 100.0;

/// One digit of a live preview. A digit whose last symbol window is still open (`settled == false`) may change on the
/// next preview: a shorter look-alike that fits the audio so far can take its place until the window has passed.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewDigit {
    /// '?' when the support is below `MIN_SUPPORT`.
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
}

#[derive(Debug, Default)]
pub struct PushResult {
    /// Every detector frame produced by this call.
    pub frames: Vec<ToneFrame>,
    pub bursts: Vec<Burst>,
}

/// Transmission in progress, tracked from the live hysteresis: opens on the first on-frame after `quiet_s` of
/// off, closes when off for `quiet_s`.
#[derive(Debug)]
struct OpenBurst {
    id: u64,
    first_frame: u64,
    on_runs: usize,
    off_frames: usize,
}

/// An open burst that has just closed: its preview is still served until the burst itself is emitted.
#[derive(Debug)]
struct Closing {
    open: OpenBurst,
    frames_at_close: u64,
    bursts_at_close: u64,
    /// The burst has been emitted: the preview goes at the next push.
    released: bool,
}

#[derive(Debug)]
struct PreviewCache {
    id: u64,
    at_frame: u64,
    value: Option<Preview>,
}

pub struct Pipeline {
    tone: ToneDetector,
    runs: RunTracker,
    bursts: BurstAssembler,
    opts: RunOptions,
    live: Live,
    /// Consecutive frames the live hysteresis has been on.
    live_run: usize,
    /// Rounded tone levels of the last `TRACE_KEEP_S` seconds: the burst decoder works on these, not on beep counts.
    levels: Vec<i8>,
    /// Broadband level (dBFS) of the same frames, for the debug log only.
    rms_db: Vec<i8>,
    levels_end: f64,
    frames_total: u64,
    /// Absolute frame number of `levels[0]`.
    levels_base: u64,
    open: Option<OpenBurst>,
    closing: Option<Closing>,
    bursts_emitted: u64,
    next_burst_id: u64,
    cache: Mutex<Option<PreviewCache>>,
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}

fn keep_frames() -> usize {
    (TRACE_KEEP_S / FRAME_SECONDS).ceil() as usize
}

fn preview_max_frames() -> usize {
    (PREVIEW_MAX_S / FRAME_SECONDS).ceil() as usize
}

fn unpoison<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Pipeline {
    pub fn new() -> Self {
        Self::with_options(RunOptions::default())
    }

    pub fn with_options(opts: RunOptions) -> Self {
        Self {
            tone: ToneDetector::new(),
            runs: RunTracker::new(opts),
            bursts: BurstAssembler::new(AssemblerOptions {
                quiet_s: opts.quiet_s,
            }),
            opts,
            live: Live {
                on: false,
                db: -99.0,
            },
            live_run: 0,
            levels: Vec::new(),
            rms_db: Vec::new(),
            levels_end: 0.0,
            frames_total: 0,
            levels_base: 0,
            open: None,
            closing: None,
            bursts_emitted: 0,
            next_burst_id: 0,
            cache: Mutex::new(None),
        }
    }

    pub fn live(&self) -> Live {
        self.live
    }

    /// Beeps collected for the burst in progress.
    pub fn pending(&self) -> usize {
        self.bursts.pending()
    }

    /// Feeds samples. A long slice is processed in steps of `PUSH_STEP_SAMPLES`, so the tone trace is trimmed and the
    /// finished bursts are decoded while their frames are still held, however much is pushed at once.
    pub fn push(&mut self, samples: &[i16]) -> PushResult {
        let mut out = PushResult::default();
        for step in samples.chunks(PUSH_STEP_SAMPLES) {
            let r = self.push_step(step);
            out.frames.extend(r.frames);
            out.bursts.extend(r.bursts);
        }
        out
    }

    fn push_step(&mut self, samples: &[i16]) -> PushResult {
        if self.closing.as_ref().is_some_and(|c| c.released) {
            self.closing = None;
        }
        let frames = self.tone.push(samples);
        for f in &frames {
            let on = if self.live.on {
                f.db > self.opts.off_db
            } else {
                f.db >= self.opts.on_db
            };
            self.live = Live { on, db: f.db };
            self.levels
                .push(js_round(f64::from(f.db)).clamp(-30.0, 60.0) as i8);
            self.rms_db.push(rms_dbfs(f.rms));
            self.levels_end = f.t;
            self.track_open(on);
            self.frames_total += 1;
        }
        let keep = keep_frames();
        if self.levels.len() > 2 * keep {
            let cut = self.levels.len() - keep;
            self.levels.drain(..cut);
            self.rms_db.drain(..cut);
            self.levels_base += cut as u64;
        }
        let runs = self.runs.push(&frames);
        let bursts: Vec<Burst> = self
            .bursts
            .push(&runs)
            .into_iter()
            .map(|b| self.decode(b))
            .collect();
        self.bursts_emitted += bursts.len() as u64;
        let hold = (PREVIEW_HOLD_S / FRAME_SECONDS).ceil() as u64;
        if let Some(c) = &mut self.closing {
            c.released |= self.bursts_emitted > c.bursts_at_close;
            if self.frames_total - c.frames_at_close > hold {
                self.closing = None;
            }
        }
        PushResult { frames, bursts }
    }

    /// Back to the state of `Pipeline::with_options` with the same options: a fresh detector, levels, runs and
    /// assembler, so a capture can restart without constructing a new pipeline.
    pub fn reset(&mut self) {
        *self = Self::with_options(self.opts);
    }

    pub fn flush(&mut self) -> Vec<Burst> {
        let runs = self.runs.flush();
        let mut bursts = self.bursts.push(&runs);
        bursts.extend(self.bursts.flush());
        self.open = None;
        self.closing = None;
        bursts.into_iter().map(|b| self.decode(b)).collect()
    }

    /// A frame counts as on for the open burst only once the live hysteresis has been on for `min_on_s` in a row, the
    /// glitch rule the run tracker applies: a stray spike neither opens a burst nor keeps one open.
    fn track_open(&mut self, on: bool) {
        let quiet_frames = (self.opts.quiet_s / FRAME_SECONDS).ceil() as usize;
        let min_on_frames = (self.opts.min_on_s / FRAME_SECONDS).ceil() as usize;
        self.live_run = if on { self.live_run + 1 } else { 0 };
        if self.live_run >= min_on_frames {
            let run_start = self.frames_total + 1 - self.live_run as u64;
            let open = self.open.get_or_insert_with(|| {
                self.closing = None;
                self.next_burst_id += 1;
                OpenBurst {
                    id: self.next_burst_id,
                    first_frame: run_start,
                    on_runs: 0,
                    off_frames: 0,
                }
            });
            open.off_frames = 0;
            if self.live_run == min_on_frames {
                open.on_runs += 1;
            }
        } else if let Some(open) = &mut self.open {
            open.off_frames += 1;
            if open.off_frames >= quiet_frames {
                self.closing = self.open.take().map(|open| Closing {
                    open,
                    frames_at_close: self.frames_total,
                    bursts_at_close: self.bursts_emitted,
                    released: false,
                });
            }
        }
    }

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
        let digits = (0..n)
            .map(|i| PreviewDigit {
                ch: if g.support[i] >= MIN_SUPPORT {
                    g.digits[i]
                } else {
                    '?'
                },
                support: g.support[i],
                margin: g.margins[i],
                start: t0 + g.starts[i],
                window_end: t0 + g.ends[i],
                settled: i + 1 < n
                    || (t0 + g.ends[i] + SETTLE_S <= trace_end
                        && t0 + g.starts[i] + longest + SETTLE_S <= trace_end),
            })
            .collect();
        Some(Preview {
            start: t0 + g.start,
            digits,
        })
    }

    /// Stream seconds of the level at index `i` of `levels`.
    fn level_time(&self, i: usize) -> f64 {
        let first = self.levels_end - (self.levels.len() as f64 - 1.0) * FRAME_SECONDS;
        first + i as f64 * FRAME_SECONDS
    }

    fn decode(&self, mut b: Burst) -> Burst {
        if b.ignored.is_some() {
            return b;
        }
        let first = self.level_time(0);
        let len = self.levels.len() as f64;
        let from = ((b.span.0 - TRACE_MARGIN_S - first) / FRAME_SECONDS)
            .floor()
            .clamp(0.0, len) as usize;
        let to = ((b.span.1 + TRACE_MARGIN_S - first) / FRAME_SECONDS)
            .ceil()
            .clamp(0.0, len) as usize;
        if to <= from {
            b.note = Some("the tone trace of this burst is no longer available".into());
            return b;
        }
        let trace = Trace {
            t0: first + from as f64 * FRAME_SECONDS,
            db: self.levels[from..to].to_vec(),
            rms_db: self.rms_db[from..to].to_vec(),
        };
        let (guess, record) = read_trace(&trace);
        b.guess = guess;
        match record {
            Ok(r) => b.records = vec![r],
            Err(note) => b.note = Some(note),
        }
        b.trace = Some(trace);
        b
    }
}

fn rms_dbfs(rms: f32) -> i8 {
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
