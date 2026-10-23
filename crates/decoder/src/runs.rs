use crate::tone::{FRAME_SECONDS, ToneFrame};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Run {
    pub on: bool,
    pub start: f64,
    pub dur: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunOptions {
    pub on_db: f32,
    pub off_db: f32,
    pub min_on_s: f64,
    pub min_off_s: f64,
    pub quiet_s: f64,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            on_db: 15.0,
            off_db: 8.0,
            min_on_s: 0.05,
            min_off_s: 0.03,
            quiet_s: 5.0,
        }
    }
}

/// Hysteresis over tone frames, then glitch removal: an "on" run shorter than `min_on_s` or an "off" run
/// shorter than `min_off_s` between two runs of the other polarity is absorbed. Runs are released once three
/// newer runs exist, because a merge can only touch the newest three.
#[derive(Debug)]
pub struct RunTracker {
    opts: RunOptions,
    state: bool,
    run_start: f64,
    last_t: f64,
    started: bool,
    runs: Vec<Run>,
}

impl Default for RunTracker {
    fn default() -> Self {
        Self::new(RunOptions::default())
    }
}

impl RunTracker {
    pub fn new(opts: RunOptions) -> Self {
        Self {
            opts,
            state: false,
            run_start: 0.0,
            last_t: 0.0,
            started: false,
            runs: Vec::new(),
        }
    }

    /// Silence longer than `quiet_s` releases every pending run (a silence that long cannot be a glitch), as
    /// consecutive off runs of `quiet_s` each; the consumer sums them.
    pub fn push(&mut self, frames: &[ToneFrame]) -> Vec<Run> {
        let mut out = Vec::new();
        for f in frames {
            let next = if self.state {
                f.db > self.opts.off_db
            } else {
                f.db >= self.opts.on_db
            };
            if !self.started {
                self.started = true;
                self.state = next;
                self.run_start = f.t - FRAME_SECONDS / 2.0;
            } else if next != self.state {
                self.close(f.t - FRAME_SECONDS / 2.0);
                self.state = next;
            }
            self.last_t = f.t;
            if !self.state && f.t + FRAME_SECONDS / 2.0 - self.run_start >= self.opts.quiet_s {
                self.close(self.run_start + self.opts.quiet_s);
                out.extend(self.release(0));
            }
        }
        out.extend(self.release(3));
        out
    }

    pub fn flush(&mut self) -> Vec<Run> {
        if self.started {
            self.close(self.last_t + FRAME_SECONDS / 2.0);
        }
        self.started = false;
        self.release(0)
    }

    fn close(&mut self, end: f64) {
        self.add(Run {
            on: self.state,
            start: self.run_start,
            dur: end - self.run_start,
        });
        self.run_start = end;
    }

    fn add(&mut self, run: Run) {
        match self.runs.last_mut() {
            Some(last) if last.on == run.on => last.dur += run.dur,
            _ => self.runs.push(run),
        }
        loop {
            let n = self.runs.len();
            if n < 3 {
                break;
            }
            let mid = self.runs[n - 2];
            let limit = if mid.on {
                self.opts.min_on_s
            } else {
                self.opts.min_off_s
            };
            if mid.dur >= limit {
                break;
            }
            let absorbed = mid.dur + self.runs[n - 1].dur;
            self.runs[n - 3].dur += absorbed;
            self.runs.truncate(n - 2);
        }
    }

    fn release(&mut self, keep: usize) -> Vec<Run> {
        let cut = self.runs.len().saturating_sub(keep);
        self.runs.drain(..cut).collect()
    }
}
