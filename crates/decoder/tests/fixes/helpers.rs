//! Helpers shared by the topic modules.

use nasdara_decoder::bunkers::bunkers;
use nasdara_decoder::synth::{SynthOptions, SynthRecord};
use nasdara_decoder::tone::ToneDetector;

pub(crate) fn one_record(code: &str, repeats: usize) -> SynthOptions {
    SynthOptions {
        records: vec![SynthRecord::new(&bunkers()[0].grid, code)],
        repeats,
        ..SynthOptions::default()
    }
}

pub(crate) fn levels_of(pcm: &[i16]) -> Vec<i8> {
    let mut d = ToneDetector::new();
    d.push(pcm)
        .iter()
        .map(|f| (f.db.round()).clamp(-30.0, 60.0) as i8)
        .collect()
}

pub(crate) fn chirp(seconds: usize) -> Vec<i16> {
    (0..44100 * seconds)
        .map(|i| {
            if (i / 8820) % 2 == 0 {
                ((i as f64 * 2.0 * std::f64::consts::PI * 985.0 / 44100.0).sin() * 8000.0) as i16
            } else {
                0
            }
        })
        .collect()
}
