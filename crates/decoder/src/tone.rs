//! Narrowband detector for the Nasdara Morse beep: ~988 Hz fundamental (measured from the game's
//! MorseCode_Short/Long samples), against reference bins that avoid its harmonics and the 2.2 to 2.8 kHz static bed.
//! A frame only counts as tone when its spectral peak lies in the tone bins' pass band: the game's own notification
//! jingle (~926 to 947 Hz), sirens and held notes near 1 kHz are louder at the edges and are not beeps.

use std::f64::consts::PI;

pub const SAMPLE_RATE: u32 = 44100;
const FRAME: usize = 1024;
const HOP: usize = 256;
pub const FRAME_SECONDS: f64 = HOP as f64 / SAMPLE_RATE as f64;

const TONE_HZ: [f64; 5] = [945.0, 965.0, 985.0, 1005.0, 1025.0];
const REF_HZ: [f64; 10] = [
    480.0, 540.0, 600.0, 680.0, 760.0, 1300.0, 1380.0, 1460.0, 1560.0, 1660.0,
];
/// The loudest reference bins left out of the noise estimate: a note, hum or voice partial sitting in a few of them
/// must not raise the estimate and hide the beep.
const REF_DROPPED: usize = 3;
/// Leaving out the loudest bins lowers the estimate and raises every level by this much for white noise;
/// subtracted so a level keeps its meaning (the thresholds and the digit scoring are set in these dB).
const REF_BIAS_DB: f64 = 2.4;
/// Bins just outside the tone bins' pass band. When one of them is louder than every tone bin the peak is at another
/// pitch, and the frame is capped at `OFF_PITCH_DB`.
const OFF_PITCH_HZ: [f64; 6] = [905.0, 925.0, 935.0, 1035.0, 1045.0, 1065.0];
const OFF_PITCH_DB: f64 = 4.0;
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

/// Tone power over the noise estimate in dB.
fn level(tone: f64, reference: f64) -> f64 {
    10.0 * ((tone + 1e-12) / (reference + 1e-12)).log10() - REF_BIAS_DB
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
    ref_c: [f64; REF_HZ.len()],
    off_pitch_c: [f64; OFF_PITCH_HZ.len()],
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
            off_pitch_c: OFF_PITCH_HZ.map(coeff),
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
        let mut refs = self.ref_c.map(|c| goertzel(&self.scratch, c));
        refs.sort_by(f64::total_cmp);
        let kept = &refs[..REF_HZ.len() - REF_DROPPED];
        let reference = kept.iter().sum::<f64>() / kept.len() as f64;
        let off_pitch = self
            .off_pitch_c
            .iter()
            .map(|&c| goertzel(&self.scratch, c))
            .fold(0.0, f64::max);
        let db = if rms < MIN_RMS {
            -99.0
        } else if tone < off_pitch {
            OFF_PITCH_DB.min(level(tone, reference))
        } else {
            level(tone, reference)
        };
        ToneFrame {
            t: (self.samples_seen as f64 - FRAME as f64 / 2.0) / SAMPLE_RATE as f64,
            db: db as f32,
            rms: rms as f32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::Rng;

    const SECONDS: usize = 3;
    /// The beep sits in 1.0 s to 1.5 s of the clip.
    const BEEP: std::ops::Range<usize> = 44_100..66_150;

    fn sine(hz: f64, amp: f64, i: usize) -> f64 {
        amp * (2.0 * PI * hz * i as f64 / f64::from(SAMPLE_RATE)).sin()
    }

    /// Frames fully inside the beep: a 1024-sample frame ends after it starts.
    fn body(frames: &[ToneFrame]) -> Vec<f32> {
        frames
            .iter()
            .filter(|f| {
                let end = (f.t * f64::from(SAMPLE_RATE)) as usize + FRAME / 2;
                end > BEEP.start + FRAME && end - FRAME >= BEEP.start && end <= BEEP.end
            })
            .map(|f| f.db)
            .collect()
    }

    /// Light white noise under `signal(i)`, 3 s long.
    fn clip(signal: impl Fn(usize) -> f64) -> Vec<ToneFrame> {
        let mut rng = Rng::new(9);
        let pcm: Vec<i16> = (0..SECONDS * SAMPLE_RATE as usize)
            .map(|i| {
                let v = signal(i) + 0.003 * (2.0 * rng.next_f64() - 1.0);
                (v.clamp(-1.0, 1.0) * 32767.0) as i16
            })
            .collect();
        ToneDetector::new().push(&pcm)
    }

    fn beep(hz: f64, amp: f64, i: usize) -> f64 {
        if BEEP.contains(&i) {
            sine(hz, amp, i) + sine(2.0 * hz, 0.35 * amp, i)
        } else {
            0.0
        }
    }

    #[test]
    fn a_clean_beep_reads_far_above_the_on_threshold() {
        let b = body(&clip(|i| beep(988.0, 0.15, i)));
        assert!(b.len() > 40, "{}", b.len());
        assert!(b.iter().all(|&d| d > 30.0), "{b:?}");
    }

    #[test]
    fn a_note_inside_the_reference_bins_does_not_hide_the_beep() {
        // 620 Hz and 1380 Hz are musical notes sitting on reference bins; with a plain mean of the bins they raised the
        // noise estimate to the beep's own level (the tone read ~5 dB over it)
        let b = body(&clip(|i| {
            beep(988.0, 0.15, i) + sine(620.0, 0.15, i) + sine(1380.0, 0.15, i)
        }));
        assert!(b.iter().all(|&d| d > 25.0), "{b:?}");
    }

    #[test]
    fn a_steady_tone_at_another_pitch_is_not_a_beep() {
        // 926 to 947 Hz: the game's notification jingle; 1100 Hz: a held note or siren
        for hz in [926.0, 935.0, 1045.0, 1100.0] {
            let frames = clip(|i| sine(hz, 0.15, i));
            let loudest = frames.iter().map(|f| f.db).fold(f32::MIN, f32::max);
            assert!(loudest < 8.0, "{hz} Hz reads {loudest} dB");
        }
    }

    #[test]
    fn beeps_within_the_pass_band_are_still_heard() {
        for hz in [950.0, 975.0, 988.0, 1000.0, 1015.0] {
            let b = body(&clip(|i| beep(hz, 0.15, i)));
            assert!(b.iter().all(|&d| d > 25.0), "{hz} Hz: {b:?}");
        }
    }
}
