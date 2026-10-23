//! Decoder for the Nasdara bunker Morse broadcast: PCM in, bunker records out.

pub mod bunkers;
pub mod bursts;
pub mod grid;
pub mod ledger;
pub mod morse;
pub mod pipeline;
pub mod runs;
pub mod synth;
pub mod tone;

/// `Math.round` of JavaScript: halves round toward positive infinity, unlike `f64::round`.
pub(crate) fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}
