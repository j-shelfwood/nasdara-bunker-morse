//! PCM (s16le mono 44.1 kHz) in, decoded bursts out.

mod preview;
mod record;

pub use preview::{Preview, PreviewDigit};
pub use record::{MIN_SUPPORT, read_trace};

use crate::bursts::{AssemblerOptions, Burst, BurstAssembler, Trace};
use crate::js_round;
use crate::runs::{RunOptions, RunTracker};
use crate::tone::{FRAME_SECONDS, ToneDetector, ToneFrame};
use preview::{PREVIEW_HOLD_S, PreviewCache};
use record::rms_dbfs;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Live {
    pub on: bool,
    pub db: f32,
}

/// One second of audio: the most `push` analyses before it trims the trace and decodes finished bursts.
const PUSH_STEP_SAMPLES: usize = 44_100;
const TRACE_KEEP_S: f64 = 150.0;
const TRACE_MARGIN_S: f64 = 2.0;

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
