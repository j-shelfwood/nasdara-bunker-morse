//! Narrowband detector for the Nasdara Morse beep: ~985 Hz fundamental (measured from the game's
//! MorseCode_Short/Long samples), against reference bins that avoid its harmonics and the 2.2 to 2.8 kHz static bed.

use std::f64::consts::PI;

pub const SAMPLE_RATE: u32 = 44100;
const FRAME: usize = 1024;
const HOP: usize = 256;
pub const FRAME_SECONDS: f64 = HOP as f64 / SAMPLE_RATE as f64;

const TONE_HZ: [f64; 5] = [945.0, 965.0, 985.0, 1005.0, 1025.0];
const REF_HZ: [f64; 6] = [500.0, 620.0, 760.0, 1300.0, 1450.0, 1650.0];
const MIN_RMS: f64 = 0.002;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToneFrame {
    /// Stream seconds at the centre of the analysed window.
    pub t: f64,
    /// Tone level in dB over the reference bins; -99 below the rms floor.
    pub db: f32,
    pub rms: f32,
}

fn coeff(hz: f64) -> f64 {
    2.0 * (2.0 * PI * hz / SAMPLE_RATE as f64).cos()
}

fn goertzel(buf: &[f64], c: f64) -> f64 {
    let (mut s1, mut s2) = (0.0, 0.0);
    for &x in buf {
        let s0 = x + c * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    s1 * s1 + s2 * s2 - c * s1 * s2
}

pub struct ToneDetector {
    ring: Vec<f64>,
    filled: usize,
    since_hop: usize,
    samples_seen: u64,
    scratch: Vec<f64>,
    window: Vec<f64>,
    tone_c: [f64; 5],
    ref_c: [f64; 6],
}

impl Default for ToneDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl ToneDetector {
    pub fn new() -> Self {
        Self {
            ring: vec![0.0; FRAME],
            filled: 0,
            since_hop: 0,
            samples_seen: 0,
            scratch: vec![0.0; FRAME],
            window: (0..FRAME)
                .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / (FRAME - 1) as f64).cos())
                .collect(),
            tone_c: TONE_HZ.map(coeff),
            ref_c: REF_HZ.map(coeff),
        }
    }

    /// Feed mono samples; returns one frame per HOP samples.
    pub fn push(&mut self, samples: &[i16]) -> Vec<ToneFrame> {
        let mut out = Vec::new();
        for &s in samples {
            self.ring[(self.samples_seen % FRAME as u64) as usize] = f64::from(s) / 32768.0;
            self.samples_seen += 1;
            if self.filled < FRAME {
                self.filled += 1;
            }
            self.since_hop += 1;
            if self.since_hop < HOP || self.filled < FRAME {
                continue;
            }
            self.since_hop = 0;
            out.push(self.analyse());
        }
        out
    }

    fn analyse(&mut self) -> ToneFrame {
        let start = (self.samples_seen - FRAME as u64) as usize;
        let mut sq = 0.0;
        for i in 0..FRAME {
            let v = self.ring[(start + i) % FRAME];
            self.scratch[i] = v * self.window[i];
            sq += v * v;
        }
        let rms = (sq / FRAME as f64).sqrt();
        let tone = self
            .tone_c
            .iter()
            .map(|&c| goertzel(&self.scratch, c))
            .fold(f64::NEG_INFINITY, f64::max);
        let reference = self
            .ref_c
            .iter()
            .map(|&c| goertzel(&self.scratch, c))
            .sum::<f64>()
            / self.ref_c.len() as f64;
        let db = if rms < MIN_RMS {
            -99.0
        } else {
            10.0 * ((tone + 1e-12) / (reference + 1e-12)).log10()
        };
        ToneFrame {
            t: (self.samples_seen as f64 - FRAME as f64 / 2.0) / SAMPLE_RATE as f64,
            db: db as f32,
            rms: rms as f32,
        }
    }
}
