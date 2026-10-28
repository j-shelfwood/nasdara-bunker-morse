use std::collections::VecDeque;
use std::time::Instant;

use nasdara_decoder::pipeline::Preview;

use crate::capture::{Source, Status};

/// Tone strength (dB over the reference bins) at which a beep starts and stops, as in the decoder.
pub(super) const ON_DB: f32 = 15.0;
pub(super) const OFF_DB: f32 = 8.0;
/// Shorter beeps are glitches, a gap shorter than this inside a beep is a dropout.
pub(super) const MIN_BEEP_S: f64 = 0.04;
pub(super) const MIN_GAP_S: f64 = 0.06;
/// A transmission is over after this much silence (the decoder uses the same).
pub(super) const QUIET_S: f64 = 5.0;
/// Test mode runs in real time where it can be heard (Windows), so the beeps keep their pitch and length.

#[derive(Clone, Copy, Debug)]
pub struct Beep {
    pub start: f64,
    /// `None` while the tone is still on.
    pub end: Option<f64>,
}

impl Beep {
    pub fn length(&self, now: f64) -> f64 {
        self.end.unwrap_or(now) - self.start
    }
}

#[derive(Clone, Debug)]
pub struct Card {
    pub key: String,
    pub bunker: Option<usize>,
    pub grid: String,
    pub code: String,
    /// Transmissions of this bunker heard so far.
    pub count: usize,
    /// Lock code digits still unreadable after voting over every transmission.
    pub unclear: usize,
    /// One grid digit was corrected to the nearest real bunker.
    pub corrected: bool,
    pub position: Option<(f64, f64)>,
    pub url: Option<String>,
    pub updated: Instant,
}

/// The last finished transmission, kept on screen until the next one starts.
#[derive(Clone, Debug)]
pub struct Finished {
    pub digits: Vec<char>,
    pub support: Vec<f32>,
    pub starts: Vec<f64>,
    pub heard: Instant,
    pub bunker: Option<usize>,
}

#[derive(Clone, Debug)]
pub enum Note {
    Info(String),
    Warn(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Live,
    Demo,
}

pub struct Shared {
    pub status: Status,
    pub mode: Mode,
    pub source: Source,
    /// Stream seconds at the last update; `clock_at` is the wall time of that update.
    pub clock: f64,
    pub clock_at: Instant,
    pub speed: f64,
    /// Tone strength per detector frame, newest last.
    pub frames: VecDeque<(f64, f32)>,
    pub beeps: VecDeque<Beep>,
    pub burst_beeps: usize,
    pub preview: Option<Preview>,
    pub finished: Option<Finished>,
    pub cards: Vec<Card>,
    /// The cards are locked: no transmission adds to or revises them and they cannot be cleared.
    pub frozen: bool,
    pub notes: Vec<Note>,
    pub level: f32,
    pub silent_for: f64,
    /// Cards that appeared or changed since the window last looked.
    pub changed: Vec<String>,
    /// `false` once the engine thread has ended for any reason, a panic included; nothing updates after that.
    pub engine_alive: bool,
}

impl Shared {
    pub(super) fn new(source: Source) -> Shared {
        Shared {
            status: Status::WaitingForGame,
            mode: Mode::Live,
            source,
            clock: 0.0,
            clock_at: Instant::now(),
            speed: 1.0,
            frames: VecDeque::new(),
            beeps: VecDeque::new(),
            burst_beeps: 0,
            preview: None,
            finished: None,
            cards: Vec::new(),
            frozen: false,
            notes: Vec::new(),
            level: 0.0,
            silent_for: 0.0,
            changed: Vec::new(),
            engine_alive: true,
        }
    }

    /// Stream time now, interpolated between engine updates so animation stays smooth.
    pub fn now(&self) -> f64 {
        self.clock + self.clock_at.elapsed().as_secs_f64().min(0.25) * self.speed
    }

    /// A transmission is coming in: beeps heard recently and enough of them to be a message.
    pub fn receiving(&self) -> bool {
        self.status == Status::Listening
            && self.burst_beeps >= 4
            && self
                .beeps
                .back()
                .is_some_and(|b| b.end.is_none() || self.now() - b.end.unwrap_or(0.0) < QUIET_S)
    }
}

pub(super) const MAX_NOTES: usize = 4;

pub(super) fn trim_notes(notes: &mut Vec<Note>) {
    if notes.len() > MAX_NOTES {
        let cut = notes.len() - MAX_NOTES;
        notes.drain(..cut);
    }
}
