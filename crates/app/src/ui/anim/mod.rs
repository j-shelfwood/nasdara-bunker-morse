//! Motion layer: eased tweens, one-shot pulses and a steady breathing signal, all driven by egui time.

mod clock;
mod tween;

pub(crate) use clock::{changed_at, egui_time_of, first_seen};
pub(crate) use egui::emath::easing;
pub(crate) use tween::Motion;
