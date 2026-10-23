//! Motion layer: eased tweens, one-shot pulses and a steady breathing signal, all driven by egui time.

// Foundations land before their first caller.
#![allow(dead_code, unused_imports)]

mod tween;

pub(crate) use egui::emath::easing;
pub(crate) use tween::Motion;
