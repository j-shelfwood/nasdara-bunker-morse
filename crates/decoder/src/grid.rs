//! Whole-digit decoding of the tone level trace against the server's fixed symbol grid.

use crate::js_round;
use crate::morse::DIGITS;
use crate::tone::FRAME_SECONDS;

// The server plays every symbol on a fixed time grid (measured on a live transmission): a symbol owns a window
// (dot 0.5 s, dash 1.5 s), the beep sample sits at the start of it (dot ~0.09 s, dash ~0.48 s audible), and a gap
// follows (0.4 s inside a digit, 0.8 s after the last symbol of a digit). A record is 10 digits with no longer
// pause between grid and code. So instead of counting beeps, each digit is scored as a whole against the tone
// level trace: a beep lost to noise or split by a dropout still leaves the digit decidable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timing {
    pub dot_window: f64,
    pub dash_window: f64,
    pub symbol_gap: f64,
    pub digit_gap: f64,
    pub dot_tone: f64,
    pub dash_tone: f64,
}

pub const SHIPPED_TIMING: Timing = Timing {
    dot_window: 0.5,
    dash_window: 1.5,
    symbol_gap: 0.4,
    digit_gap: 0.8,
    dot_tone: 0.09,
    dash_tone: 0.48,
};

pub const DIGITS_PER_RECORD: usize = 10;
/// Each digit may start this far (seconds) from where the previous one predicts: clock jitter between server and client.
const JITTER_S: f64 = 0.12;
const JITTER_PENALTY: f64 = 0.5;
/// Tone level (dB over the reference bins) at which a frame counts as evidence for a beep; ~25 dB is a beep, ~-3 dB is not.
const EVIDENCE_DB: f64 = 11.0;
const EVIDENCE_SCALE: f64 = 0.3;
const DB_FLOOR: f64 = -15.0;
const DB_CEIL: f64 = 35.0;
const NEG: f64 = -1e18;

struct Pattern {
    digit: char,
    on: Vec<(usize, usize)>,
    len: usize,
    tail: usize,
}

fn to_frames(s: f64) -> usize {
    js_round(s / FRAME_SECONDS) as usize
}

fn patterns_of(t: &Timing) -> Vec<Pattern> {
    DIGITS
        .iter()
        .map(|&(symbols, digit)| {
            let mut at = 0.0;
            let mut on = Vec::new();
            let last = symbols.len() - 1;
            for (k, c) in symbols.chars().enumerate() {
                let dash = c == '-';
                let tone = if dash { t.dash_tone } else { t.dot_tone };
                on.push((to_frames(at), to_frames(at + tone)));
                let window = if dash { t.dash_window } else { t.dot_window };
                at += window + if k == last { t.digit_gap } else { t.symbol_gap };
            }
            Pattern {
                digit,
                on,
                len: to_frames(at),
                tail: to_frames(t.digit_gap),
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct GridDecode {
    pub digits: Vec<char>,
    /// Per digit: how much better the chosen digit explains the trace than the runner-up (0 = tie), in beep-lengths of tone.
    pub margins: Vec<f32>,
    /// Per digit: tone evidence inside the chosen digit's beeps, as a share of what a clean digit gives (<= 1, negative: no tone).
    pub support: Vec<f32>,
    /// Seconds from the trace start to the start of each digit.
    pub starts: Vec<f64>,
    /// Seconds from the trace start to the end of each digit's last symbol window (the point a digit counts as heard).
    pub ends: Vec<f64>,
    /// Seconds from the trace start to the first digit.
    pub start: f64,
    pub score: f64,
}

/// Viterbi table: `best[j][f]` = best score with `j` digits placed and the next one starting near frame `f`.
struct Dp {
    patterns: Vec<Pattern>,
    cum: Vec<f64>,
    timing: Timing,
    /// Row stride: frames plus the longest pattern.
    stride: usize,
    best: Vec<f64>,
    back_v: Vec<u8>,
    back_start: Vec<u32>,
    back_prev: Vec<u32>,
}

impl Dp {
    fn frames(&self) -> usize {
        self.cum.len() - 1
    }

    fn at(&self, f: i64) -> f64 {
        self.cum[f.clamp(0, self.frames() as i64) as usize]
    }

    fn score(&self, p: &Pattern, f: i64) -> f64 {
        p.on.iter()
            .map(|&(a, b)| self.at(f + b as i64) - self.at(f + a as i64))
            .sum()
    }

    fn new(db: &[i8], count: usize, timing: &Timing) -> Option<Self> {
        let frames = db.len();
        if frames < to_frames(2.0) {
            return None;
        }
        let patterns = patterns_of(timing);
        let max_len = patterns.iter().map(|p| p.len).max().unwrap_or(0);
        let mut cum = vec![0.0; frames + 1];
        for (i, &v) in db.iter().enumerate() {
            cum[i + 1] =
                cum[i] + EVIDENCE_SCALE * (f64::from(v).clamp(DB_FLOOR, DB_CEIL) - EVIDENCE_DB);
        }
        let stride = frames + max_len + 2;
        let cells = (count + 1) * stride;
        let mut dp = Self {
            patterns,
            cum,
            timing: *timing,
            stride,
            best: vec![NEG; cells],
            back_v: vec![0; cells],
            back_start: vec![0; cells],
            back_prev: vec![0; cells],
        };
        dp.best[..stride].fill(0.0);
        dp.fill(count);
        Some(dp)
    }

    fn fill(&mut self, count: usize) {
        let frames = self.frames();
        let stride = self.stride;
        let jitter = to_frames(JITTER_S);
        let np = self.patterns.len();
        // Pattern scores per start frame do not depend on the digit index.
        let mut pat_score = vec![0.0; frames * np];
        for start in 0..frames {
            for (k, p) in self.patterns.iter().enumerate() {
                pat_score[start * np + k] = self.score(p, start as i64);
            }
        }
        for j in 0..count {
            for start in 0..frames {
                let mut base = NEG;
                let mut from = None;
                for f in start.saturating_sub(jitter)..=(start + jitter).min(stride - 1) {
                    let b = self.best[j * stride + f];
                    let v = if b == NEG {
                        NEG
                    } else if j == 0 {
                        b
                    } else {
                        b - JITTER_PENALTY * f.abs_diff(start) as f64
                    };
                    if v > base {
                        base = v;
                        from = Some(f);
                    }
                }
                let Some(from) = from else { continue };
                for (k, p) in self.patterns.iter().enumerate() {
                    if start + p.len - p.tail > frames {
                        continue;
                    }
                    let end = start + p.len;
                    let sc = base + pat_score[start * np + k];
                    let cell = (j + 1) * stride + end;
                    if sc > self.best[cell] {
                        self.best[cell] = sc;
                        self.back_v[cell] = k as u8;
                        self.back_start[cell] = start as u32;
                        self.back_prev[cell] = from as u32;
                    }
                }
            }
        }
    }

    fn trace_back(&self, count: usize, end: usize) -> GridDecode {
        let t = &self.timing;
        let clean_unit = EVIDENCE_SCALE * (25.0 - EVIDENCE_DB);
        let margin_unit = to_frames(t.dash_tone) as f64 * clean_unit;
        let mut out = GridDecode {
            digits: Vec::with_capacity(count),
            margins: Vec::with_capacity(count),
            support: Vec::with_capacity(count),
            starts: Vec::with_capacity(count),
            ends: Vec::with_capacity(count),
            start: 0.0,
            score: self.best[count * self.stride + end],
        };
        let mut pos = end;
        for j in (1..=count).rev() {
            let cell = j * self.stride + pos;
            let k = usize::from(self.back_v[cell]);
            let p = &self.patterns[k];
            let s = self.back_start[cell] as usize;
            let mut runner_up = NEG;
            for (q_idx, q) in self.patterns.iter().enumerate() {
                if q_idx == k {
                    continue;
                }
                for d in -3..=3 {
                    runner_up = runner_up.max(self.score(q, s as i64 + d));
                }
            }
            let clean = p.on.iter().map(|&(a, b)| (b - a) as f64).sum::<f64>() * clean_unit;
            let own = self.score(p, s as i64);
            out.digits.push(p.digit);
            out.margins.push(((own - runner_up) / margin_unit) as f32);
            out.support.push((own / clean) as f32);
            out.starts.push(s as f64 * FRAME_SECONDS);
            out.ends.push((s + p.len - p.tail) as f64 * FRAME_SECONDS);
            pos = self.back_prev[cell] as usize;
        }
        out.digits.reverse();
        out.margins.reverse();
        out.support.reverse();
        out.starts.reverse();
        out.ends.reverse();
        out.start = out.starts.first().copied().unwrap_or(0.0);
        out
    }
}

/// Decodes exactly `count` digits that end the best-scoring path through the whole trace.
pub fn decode_grid(db: &[i8], count: usize, timing: &Timing) -> Option<GridDecode> {
    if count == 0 {
        return None;
    }
    let dp = Dp::new(db, count, timing)?;
    let row = &dp.best[count * dp.stride..(count + 1) * dp.stride];
    let mut end = None;
    for (f, &v) in row.iter().enumerate() {
        if v > end.map_or(NEG, |e| row[e]) {
            end = Some(f);
        }
    }
    Some(dp.trace_back(count, end?))
}

/// Same Viterbi table as `decode_grid`, but the number of digits is free (1..=`max_count`): the best total score
/// over (digit count, end frame) picks the prefix the audio supports, because a digit placed on frames without tone
/// adds a negative score. For a live preview while a transmission is still arriving. `None` when nothing scores above 0.
pub fn decode_prefix(db: &[i8], timing: &Timing, max_count: usize) -> Option<GridDecode> {
    if max_count == 0 {
        return None;
    }
    let dp = Dp::new(db, max_count, timing)?;
    let mut pick: Option<(usize, usize)> = None;
    let mut top = 0.0;
    for j in 1..=max_count {
        for f in 0..dp.stride {
            let v = dp.best[j * dp.stride + f];
            if v > top {
                top = v;
                pick = Some((j, f));
            }
        }
    }
    let (j, end) = pick?;
    Some(dp.trace_back(j, end))
}
