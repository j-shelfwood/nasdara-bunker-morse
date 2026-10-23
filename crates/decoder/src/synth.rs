//! Synthetic transmissions, generated here: no game audio is used or shipped.
//! Measured on a live transmission: beep ~985 Hz with harmonics near 1976, 2842 and 3828 Hz, dot ~0.09 s and dash ~0.48 s
//! audible, symbol windows and gaps as `GAPS_REAL`. Assumed: levels, the static bed (broadband around 2.2 to 2.8 kHz), wind.

use crate::bunkers::bunkers;
use crate::js_round;
use crate::tone::SAMPLE_RATE;
use std::f64::consts::PI;

const SR: f64 = SAMPLE_RATE as f64;

/// LCG with the same sequence as the JavaScript original (`Math.imul` is a wrapping multiply).
#[derive(Debug, Clone)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self(seed)
    }

    pub fn next_f64(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        f64::from(self.0) / 4294967296.0
    }
}

fn table(c: char) -> &'static str {
    crate::morse::pattern_of(c)
}

const PARTIALS: [(f64, f64); 4] = [(1.0, 1.0), (2.006, 0.35), (2.885, 0.18), (3.886, 0.1)];

fn beep(dash: bool, rng: &mut Rng, variance: f64) -> Vec<f32> {
    let seconds =
        (if dash { 0.48 } else { 0.09 }) * (1.0 + 0.15 * variance * (2.0 * rng.next_f64() - 1.0));
    let hz = 985.0 * (1.0 + 0.006 * variance * (2.0 * rng.next_f64() - 1.0));
    let n = js_round(seconds * SR) as usize;
    let fade = js_round(0.004 * SR);
    (0..n)
        .map(|i| {
            let i = i as f64;
            let v: f64 = PARTIALS
                .iter()
                .map(|&(r, a)| a * (2.0 * PI * hz * r * i / SR).sin())
                .sum();
            (0.2 * v * 1.0_f64.min(i / fade).min((n as f64 - i) / fade)) as f32
        })
        .collect()
}

fn static_bed(n: usize, rng: &mut Rng) -> Vec<f32> {
    let mut out = vec![0.0f32; n];
    let mut hz = 2200.0;
    while hz <= 2800.0 {
        let phase = rng.next_f64() * 2.0 * PI;
        let amp = 0.004;
        for (i, o) in out.iter_mut().enumerate() {
            *o = (f64::from(*o) + amp * (2.0 * PI * hz * i as f64 / SR + phase).sin()) as f32;
        }
        hz += 25.0;
    }
    out
}

/// Wind: white noise through a one-pole low-pass (energy piles up below ~600 Hz, where the detector's reference bins sit);
/// `wind_gain` 1 is about 0.03 rms, a beep is about 0.2 rms.
fn wind(n: usize, rng: &mut Rng) -> Vec<f32> {
    let mut y = 0.0;
    (0..n)
        .map(|_| {
            y = 0.92 * y + 0.08 * (2.0 * rng.next_f64() - 1.0);
            (y * 0.25 + 0.003 * (2.0 * rng.next_f64() - 1.0)) as f32
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GapModel {
    pub intra: f64,
    pub char: f64,
    pub word: f64,
    pub bunker: f64,
    /// A symbol occupies a fixed window from its start, with the beep at the start.
    pub window_dot: f64,
    pub window_dash: f64,
}

/// Measured: start-to-start 0.9 s (dot) / 1.9 s (dash) inside a digit, 1.3 s / 2.3 s at a digit end.
pub const GAPS_REAL: GapModel = GapModel {
    intra: 0.4,
    char: 0.8,
    word: 0.8,
    bunker: 6.0,
    window_dot: 0.5,
    window_dash: 1.5,
};

#[derive(Debug, Clone, PartialEq)]
pub struct SynthRecord {
    pub grid: String,
    pub code: String,
}

impl SynthRecord {
    pub fn new(grid: &str, code: &str) -> Self {
        Self {
            grid: grid.into(),
            code: code.into(),
        }
    }
}

/// Where pings may land: before/after the message only, or anywhere (including on top of beeps).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PingZone {
    Outside,
    Anywhere,
    Cluster,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SynthOptions {
    pub records: Vec<SynthRecord>,
    pub gaps: GapModel,
    pub repeats: usize,
    /// Beep length +-15% and pitch +-0.6% at 1.
    pub variance: f64,
    pub static_gain: f64,
    pub wind_gain: f64,
    pub seed: u32,
    pub lead_s: f64,
    /// Unrelated 1 kHz pings dropped at random times (amplitude 0.1).
    pub pings: usize,
    pub ping_zone: PingZone,
    /// Skip the first N elements of the stream (capture starting mid-message).
    pub skip_elements: usize,
    /// Chance per beep of a 30 to 150 ms hole inside it (seen live: the tone level drops out mid-beep).
    pub dropout: f64,
    /// Chance per beep that it is not heard at all.
    pub lose: f64,
}

pub const DEFAULTS: SynthOptions = SynthOptions {
    records: Vec::new(),
    gaps: GAPS_REAL,
    repeats: 2,
    variance: 0.0,
    static_gain: 1.0,
    wind_gain: 0.0,
    seed: 1,
    lead_s: 1.0,
    pings: 0,
    ping_zone: PingZone::Outside,
    skip_elements: 0,
    dropout: 0.0,
    lose: 0.0,
};

impl Default for SynthOptions {
    fn default() -> Self {
        DEFAULTS
    }
}

fn with_hole(d: &[f32], rng: &mut Rng) -> Vec<f32> {
    let mut out = d.to_vec();
    let hole =
        ((js_round((0.03 + 0.12 * rng.next_f64()) * SR)) as usize).min(out.len().saturating_sub(1));
    let from = (rng.next_f64() * (out.len() - hole).max(1) as f64).floor() as usize;
    let to = (from + hole).min(out.len());
    out[from.min(to)..to].fill(0.0);
    out
}

#[derive(Debug, Clone)]
pub struct Rendered {
    pub pcm: Vec<i16>,
    pub seconds: f64,
}

pub fn render(o: &SynthOptions) -> Rendered {
    let mut rng = Rng::new(o.seed);
    let mut place: Vec<(f64, Vec<f32>)> = Vec::new();
    let mut t = o.lead_s;
    let mut skip = o.skip_elements;
    for _ in 0..o.repeats {
        for rec in &o.records {
            for (wi, w) in [&rec.grid, &rec.code].into_iter().enumerate() {
                let chars: Vec<char> = w.chars().collect();
                for (ci, &ch) in chars.iter().enumerate() {
                    let code = table(ch);
                    let symbols: Vec<char> = code.chars().collect();
                    for (ei, &sym) in symbols.iter().enumerate() {
                        let dash = sym == '-';
                        let mut data = beep(dash, &mut rng, o.variance);
                        if o.dropout > 0.0 && rng.next_f64() < o.dropout {
                            data = with_hole(&data, &mut rng);
                        }
                        let lost = o.lose > 0.0 && rng.next_f64() < o.lose;
                        if skip > 0 {
                            skip -= 1;
                        } else if !lost {
                            place.push((t, data));
                        }
                        t += if dash {
                            o.gaps.window_dash
                        } else {
                            o.gaps.window_dot
                        };
                        t += if ei + 1 < symbols.len() {
                            o.gaps.intra
                        } else if ci + 1 < chars.len() {
                            o.gaps.char
                        } else if wi == 0 {
                            o.gaps.word
                        } else {
                            o.gaps.bunker
                        };
                    }
                }
            }
        }
    }
    let seconds = t + 2.0;
    let n = (seconds * SR).ceil() as usize;
    let mut mix = vec![0.0f32; n];
    if o.static_gain != 0.0 {
        let bed = static_bed(n.min(8 * SAMPLE_RATE as usize), &mut rng);
        for (i, m) in mix.iter_mut().enumerate() {
            *m = (f64::from(bed[i % bed.len()]) * o.static_gain) as f32;
        }
    }
    if o.wind_gain != 0.0 {
        let w = wind(n, &mut rng);
        for (m, &x) in mix.iter_mut().zip(&w) {
            *m = (f64::from(*m) + f64::from(x) * o.wind_gain) as f32;
        }
    }
    for (at, data) in &place {
        let s0 = js_round(at * SR) as usize;
        for (i, &d) in data.iter().enumerate() {
            if let Some(m) = mix.get_mut(s0 + i) {
                *m += d;
            }
        }
    }
    for k in 0..o.pings {
        let msg_start = js_round(o.lead_s * SR) as i64;
        let msg_end = js_round((t - 1.0) * SR) as i64;
        let s0 = match o.ping_zone {
            PingZone::Anywhere => (rng.next_f64() * (n as f64 - 20000.0)).floor() as i64,
            PingZone::Cluster => msg_start - js_round((0.7 + 0.7 * (k as f64 + 1.0)) * SR) as i64,
            PingZone::Outside if k % 2 == 0 => msg_start - js_round(2.5 * SR) as i64,
            PingZone::Outside => msg_end + js_round(1.5 * SR) as i64,
        };
        for i in 0..6615usize {
            let idx = s0 + i as i64;
            if let Some(m) = usize::try_from(idx).ok().and_then(|ix| mix.get_mut(ix)) {
                let fi = i as f64;
                let env = 1.0_f64.min(fi / 200.0).min((6615.0 - fi) / 200.0);
                *m = (f64::from(*m) + 0.1 * (2.0 * PI * 1000.0 * fi / SR).sin() * env) as f32;
            }
        }
    }
    let pcm = mix
        .iter()
        .map(|&m| js_round(f64::from(m) * 32767.0).clamp(-32768.0, 32767.0) as i16)
        .collect();
    Rendered { pcm, seconds }
}

/// Default chunk size of `chunks`: 0.1 s of audio.
pub const CHUNK_SAMPLES: usize = 4410;

pub fn chunks(pcm: &[i16]) -> std::slice::Chunks<'_, i16> {
    pcm.chunks(CHUNK_SAMPLES)
}

/// Three bunkers' records with realistic noise (varying beeps, static bed, a few lost beeps), heard twice.
pub fn demo_transmission(seed: u32) -> Vec<i16> {
    let b = bunkers();
    let records = [(0, "4821"), (9, "0075"), (16, "9136")]
        .iter()
        .map(|&(i, code)| SynthRecord {
            grid: b[i].grid.clone(),
            code: code.into(),
        })
        .collect();
    render(&SynthOptions {
        records,
        variance: 1.0,
        lose: 0.03,
        seed,
        ..DEFAULTS
    })
    .pcm
}
