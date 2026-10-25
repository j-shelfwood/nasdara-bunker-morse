use crate::bunkers::Snap;
use crate::runs::Run;

#[derive(Debug, Clone, PartialEq)]
pub struct BunkerRecord {
    /// Ten digits, '?' where the tone evidence did not decide the digit.
    pub digits: Vec<char>,
    /// Ten digits as the grid decoder read them, never '?': the ledger votes with these, weighted by `support`.
    pub raw: Vec<char>,
    /// Per digit, 1 = a clean beep pattern; the digit is '?' below the pipeline's threshold.
    pub support: Vec<f32>,
    /// Per digit, how much better the chosen digit explains the trace than the runner-up.
    pub margins: Vec<f32>,
    pub grid: String,
    pub code: String,
    pub snap: Snap,
    /// Stream seconds of the first digit.
    pub start: f64,
    /// Stream seconds of the start of each digit, from the Viterbi alignment.
    pub digit_starts: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Beep {
    pub start: f64,
    pub dur: f64,
}

/// Tone level per detector frame around a burst (rounded dB, clamped to -30..60), attached by the pipeline.
#[derive(Debug, Clone, PartialEq)]
pub struct Trace {
    pub t0: f64,
    pub db: Vec<i8>,
    /// Broadband level per frame in dBFS (rounded, clamped to -100..0), same frames as `db`: tells a quiet game from
    /// a weak beep. Empty when the trace is replayed from a fixture.
    pub rms_db: Vec<i8>,
}

/// What the grid decoder read from a burst before any digit was judged unreadable.
#[derive(Debug, Clone, PartialEq)]
pub struct Guess {
    pub digits: String,
    pub support: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Burst {
    pub start: f64,
    /// Number of beeps.
    pub elements: usize,
    pub records: Vec<BunkerRecord>,
    /// Set when a burst was heard but gave no record (a partial capture, or too few readable digits).
    pub note: Option<String>,
    /// Set when the burst was not decoded at all (too few beeps): logged, never shown.
    pub ignored: Option<String>,
    /// First beep start and last beep end, in stream seconds.
    pub span: (f64, f64),
    pub trace: Option<Trace>,
    /// The raw grid reading, set whenever one was made, also when no record came of it.
    pub guess: Option<Guess>,
    pub beeps: Vec<Beep>,
    /// Silence before the first beep.
    pub first_gap: f64,
}

fn r2(v: f64) -> f64 {
    crate::js_round(v * 100.0) / 100.0
}

impl Burst {
    /// Beep lengths in seconds (2 decimals), for calibration logs.
    pub fn durs(&self) -> Vec<f64> {
        self.beeps.iter().map(|b| r2(b.dur)).collect()
    }

    /// The silence before each beep in seconds (2 decimals), for calibration logs.
    pub fn gaps(&self) -> Vec<f64> {
        let mut prev_end = None;
        self.beeps
            .iter()
            .map(|b| {
                let gap = prev_end.map_or(self.first_gap, |end| b.start - end);
                prev_end = Some(b.start + b.dur);
                r2(gap)
            })
            .collect()
    }
}

/// A message is 10 digits of 5 symbols; fewer beeps than this is a stray tone or a few seconds of one.
const MIN_BURST_ELEMENTS: usize = 20;

/// A burst longer than this is not a message (a real one is about 90 s, three bunkers are separated by 6 s pauses): a
/// periodic chirp would otherwise hold beeps forever. It is emitted as ignored and a new burst starts.
pub const MAX_BURST_SPAN_S: f64 = 200.0;

#[derive(Debug, Clone, Copy)]
pub struct AssemblerOptions {
    pub quiet_s: f64,
}

impl Default for AssemblerOptions {
    fn default() -> Self {
        Self { quiet_s: 5.0 }
    }
}

/// Groups tone runs into bursts: a burst ends after `quiet_s` of silence. Inside a message the silence stays under
/// ~4 s even when a few beeps are lost; between bunkers it is at least 6 s (config pause) plus the tail of the last window.
#[derive(Debug, Default)]
pub struct BurstAssembler {
    opts: AssemblerOptions,
    beeps: Vec<Beep>,
    first_gap: f64,
    off_acc: f64,
}

impl BurstAssembler {
    pub fn new(opts: AssemblerOptions) -> Self {
        Self {
            opts,
            ..Self::default()
        }
    }

    /// Beeps collected for the burst in progress.
    pub fn pending(&self) -> usize {
        self.beeps.len()
    }

    pub fn push(&mut self, runs: &[Run]) -> Vec<Burst> {
        let mut out = Vec::new();
        for r in runs {
            if r.on {
                if self
                    .beeps
                    .first()
                    .is_some_and(|f| r.start - f.start > MAX_BURST_SPAN_S)
                {
                    out.extend(self.finish_with(Some(format!(
                        "longer than {MAX_BURST_SPAN_S} s without a {}-s pause, not a message",
                        self.opts.quiet_s
                    ))));
                }
                if self.beeps.is_empty() {
                    self.first_gap = self.off_acc;
                }
                self.beeps.push(Beep {
                    start: r.start,
                    dur: r.dur,
                });
                self.off_acc = 0.0;
            } else {
                self.off_acc += r.dur;
                if self.off_acc >= self.opts.quiet_s {
                    out.extend(self.finish());
                    self.off_acc = 0.0;
                }
            }
        }
        out
    }

    pub fn flush(&mut self) -> Vec<Burst> {
        self.finish().into_iter().collect()
    }

    fn finish(&mut self) -> Option<Burst> {
        self.finish_with(None)
    }

    /// Ends the burst in progress; `forced_ignore` is the reason when it must not be decoded whatever its length.
    fn finish_with(&mut self, forced_ignore: Option<String>) -> Option<Burst> {
        let beeps = std::mem::take(&mut self.beeps);
        let (first, last) = (*beeps.first()?, *beeps.last()?);
        let ignored = forced_ignore.or_else(|| {
            (beeps.len() < MIN_BURST_ELEMENTS).then(|| {
                format!(
                    "only {} beeps, a message has at least {MIN_BURST_ELEMENTS}",
                    beeps.len()
                )
            })
        });
        Some(Burst {
            start: first.start,
            elements: beeps.len(),
            records: Vec::new(),
            note: None,
            ignored,
            span: (first.start, last.start + last.dur),
            trace: None,
            guess: None,
            beeps,
            first_gap: self.first_gap,
        })
    }
}
