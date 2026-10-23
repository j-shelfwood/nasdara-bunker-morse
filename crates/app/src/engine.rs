//! The listening side: takes audio from a source, runs the decoder and keeps the state the window draws.

use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use nasdara_decoder::bunkers::{SnapKind, cell_center, izurvive_url};
use nasdara_decoder::bursts::{BunkerRecord, Burst};
use nasdara_decoder::ledger::BunkerLedger;
use nasdara_decoder::pipeline::{Pipeline, Preview};
use nasdara_decoder::synth;

use crate::capture::{Capture, Event, Message, Source, Status};
use crate::log::DebugLog;

/// How much history the window can draw.
pub const HISTORY_S: f64 = 60.0;
/// Tone strength (dB over the reference bins) at which a beep starts and stops, as in the decoder.
const ON_DB: f32 = 15.0;
const OFF_DB: f32 = 8.0;
/// Shorter beeps are glitches, a gap shorter than this inside a beep is a dropout.
const MIN_BEEP_S: f64 = 0.04;
const MIN_GAP_S: f64 = 0.06;
/// A transmission is over after this much silence (the decoder uses the same).
const QUIET_S: f64 = 5.0;
const DEMO_SPEED: f64 = 2.0;

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
    pub notes: Vec<Note>,
    pub level: f32,
    pub silent_for: f64,
    /// Cards that appeared or changed since the window last looked.
    pub changed: Vec<String>,
    pub demo_done: bool,
}

impl Shared {
    fn new(source: Source) -> Shared {
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
            notes: Vec::new(),
            level: 0.0,
            silent_for: 0.0,
            changed: Vec::new(),
            demo_done: false,
        }
    }

    /// Stream time now, interpolated between engine updates so animation stays smooth.
    pub fn now(&self) -> f64 {
        self.clock + self.clock_at.elapsed().as_secs_f64().min(0.25) * self.speed
    }

    /// A transmission is coming in: beeps heard recently and enough of them to be a message.
    pub fn receiving(&self) -> bool {
        self.burst_beeps >= 4
            && self
                .beeps
                .back()
                .is_some_and(|b| b.end.is_none() || self.now() - b.end.unwrap_or(0.0) < QUIET_S)
    }
}

pub enum Command {
    SetSource(Source),
    StartDemo,
    StopDemo,
    ClearCards,
}

#[derive(Clone)]
pub struct Handle {
    shared: Arc<Mutex<Shared>>,
    commands: Sender<Command>,
}

impl Handle {
    pub fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn send(&self, c: Command) {
        let _ = self.commands.send(c);
    }
}

pub fn start(source: Source, ctx: egui::Context, log: Option<DebugLog>) -> Handle {
    let shared = Arc::new(Mutex::new(Shared::new(source)));
    let (commands, command_rx) = mpsc::channel();
    let handle = Handle {
        shared: shared.clone(),
        commands,
    };
    let engine = Engine::new(shared, ctx, log, source);
    std::thread::Builder::new()
        .name("engine".into())
        .spawn(move || engine.run(command_rx))
        .expect("engine thread");
    handle
}

struct Engine {
    shared: Arc<Mutex<Shared>>,
    ctx: egui::Context,
    log: Option<DebugLog>,
    pipeline: Pipeline,
    ledger: BunkerLedger,
    capture: Option<Capture>,
    /// Session counter: bumped on every restart; events of an older generation belong to a replaced capture.
    generation: u64,
    source: Source,
    mode: Mode,
    tone_on: bool,
    open: Option<usize>,
    last_beep_end: f64,
    silent_samples: u64,
    peak: f32,
}

impl Engine {
    fn new(
        shared: Arc<Mutex<Shared>>,
        ctx: egui::Context,
        log: Option<DebugLog>,
        source: Source,
    ) -> Engine {
        Engine {
            shared,
            ctx,
            log,
            pipeline: Pipeline::new(),
            ledger: BunkerLedger::new(),
            capture: None,
            generation: 0,
            source,
            mode: Mode::Live,
            tone_on: false,
            open: None,
            last_beep_end: f64::NEG_INFINITY,
            silent_samples: 0,
            peak: 0.0,
        }
    }

    fn run(mut self, commands: Receiver<Command>) {
        let (tx, events) = mpsc::channel();
        self.restart(&tx);
        loop {
            while let Ok(c) = commands.try_recv() {
                self.command(c, &tx);
            }
            match events.recv_timeout(Duration::from_millis(20)) {
                Ok(e) => self.event(e),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    fn restart(&mut self, tx: &Sender<Message>) {
        self.capture = None;
        self.generation += 1;
        self.pipeline = Pipeline::new();
        self.tone_on = false;
        self.open = None;
        self.last_beep_end = f64::NEG_INFINITY;
        self.silent_samples = 0;
        let (mode, speed) = (
            self.mode,
            if self.mode == Mode::Demo {
                DEMO_SPEED
            } else {
                1.0
            },
        );
        {
            let mut s = self.lock();
            s.mode = mode;
            s.speed = speed;
            s.source = self.source;
            s.frames.clear();
            s.beeps.clear();
            s.burst_beeps = 0;
            s.preview = None;
            s.clock = 0.0;
            s.clock_at = Instant::now();
            s.demo_done = false;
            s.status = if mode == Mode::Demo {
                Status::Listening
            } else {
                Status::WaitingForGame
            };
        }
        self.capture = Some(match mode {
            Mode::Live => Capture::live(self.source, self.generation, tx.clone()),
            Mode::Demo => Capture::demo(
                synth::demo_transmission(7),
                DEMO_SPEED,
                self.generation,
                tx.clone(),
            ),
        });
        self.ctx.request_repaint();
    }

    fn command(&mut self, c: Command, tx: &Sender<Message>) {
        match c {
            Command::SetSource(source) => {
                self.source = source;
                self.mode = Mode::Live;
                self.restart(tx);
            }
            Command::StartDemo => {
                self.mode = Mode::Demo;
                self.restart(tx);
            }
            Command::StopDemo => {
                self.mode = Mode::Live;
                self.restart(tx);
            }
            Command::ClearCards => {
                self.ledger = BunkerLedger::new();
                let mut s = self.lock();
                s.cards.clear();
                s.finished = None;
                s.notes.clear();
            }
        }
    }

    fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn event(&mut self, m: Message) {
        if m.generation != self.generation {
            return;
        }
        match m.event {
            Event::Status(status) => {
                self.lock().status = status;
                self.ctx.request_repaint();
            }
            Event::Samples(pcm) => self.samples(&pcm),
            Event::DemoFinished => {
                // the demo clip ends in silence, so a final burst may still be open
                let bursts = self.pipeline.flush();
                self.bursts(bursts);
                self.lock().demo_done = true;
            }
        }
    }

    fn samples(&mut self, pcm: &[i16]) {
        let peak = pcm.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0) as f32 / 32768.0;
        self.peak = peak.max(self.peak * 0.97);
        if peak > 0.0001 {
            self.silent_samples = 0;
        } else {
            self.silent_samples += pcm.len() as u64;
        }
        let out = self.pipeline.push(pcm);
        let preview = self.pipeline.preview();
        let shared = self.shared.clone();
        let mut s = shared.lock().unwrap_or_else(|p| p.into_inner());
        for f in &out.frames {
            self.track_beeps(&mut s, f.t, f.db);
            s.frames.push_back((f.t, f.db));
            s.clock = f.t;
        }
        s.clock_at = Instant::now();
        let horizon = s.clock - HISTORY_S;
        while s.frames.front().is_some_and(|&(t, _)| t < horizon) {
            s.frames.pop_front();
        }
        while s
            .beeps
            .front()
            .is_some_and(|b| b.end.is_some_and(|e| e < horizon))
        {
            s.beeps.pop_front();
            self.open = self.open.map(|i| i.saturating_sub(1));
        }
        s.level = self.peak;
        s.silent_for = self.silent_samples as f64 / crate::capture::SAMPLE_RATE as f64;
        s.preview = preview;
        let fresh = if out.bursts.is_empty() {
            Vec::new()
        } else {
            self.digest(&mut s, out.bursts)
        };
        drop(s);
        self.after(fresh);
    }

    /// Provisional beeps for the live ribbon, from the same thresholds the decoder uses.
    fn track_beeps(&mut self, s: &mut Shared, t: f64, db: f32) {
        let on = if self.tone_on {
            db > OFF_DB
        } else {
            db >= ON_DB
        };
        if on && !self.tone_on {
            if t - self.last_beep_end > QUIET_S {
                s.burst_beeps = 0;
            }
            if self.open.is_none() {
                // a dropout shorter than MIN_GAP_S reopens the previous beep instead of starting a new one
                match s.beeps.back_mut() {
                    Some(prev) if prev.end.is_some_and(|e| t - e < MIN_GAP_S) => {
                        prev.end = None;
                        self.open = Some(s.beeps.len() - 1);
                    }
                    _ => {
                        s.beeps.push_back(Beep {
                            start: t,
                            end: None,
                        });
                        self.open = Some(s.beeps.len() - 1);
                        s.burst_beeps += 1;
                    }
                }
            }
        } else if !on
            && self.tone_on
            && let Some(i) = self.open.take()
            && let Some(b) = s.beeps.get_mut(i)
        {
            b.end = Some(t);
            self.last_beep_end = t;
            if b.length(t) < MIN_BEEP_S {
                s.beeps.remove(i);
                s.burst_beeps = s.burst_beeps.saturating_sub(1);
            }
        }
        self.tone_on = on;
    }

    fn bursts(&mut self, bursts: Vec<Burst>) {
        let shared = self.shared.clone();
        let mut s = shared.lock().unwrap_or_else(|p| p.into_inner());
        let fresh = self.digest(&mut s, bursts);
        drop(s);
        self.after(fresh);
    }

    fn digest(&mut self, s: &mut Shared, bursts: Vec<Burst>) -> Vec<String> {
        let mut fresh = Vec::new();
        for b in bursts {
            let system = s.source == Source::System && s.mode == Mode::Live;
            let shown = b.records.iter().filter(|r| accepted(r, system)).count();
            if let Some(log) = &self.log {
                log.burst(&b, shown);
            }
            if b.ignored.is_some() {
                continue;
            }
            if b.records.is_empty() {
                if let Some(note) = &b.note {
                    s.notes.push(Note::Info(format!("A transmission of {} beeps was heard but could not be read ({note}). It repeats, so keep listening.", b.elements)));
                }
                continue;
            }
            for r in b.records.iter().filter(|r| accepted(r, system)) {
                let e = self.ledger.add(r);
                let position = e
                    .bunker
                    .map(|b| (b.x, b.z))
                    .or_else(|| cell_center(&e.grid));
                let card = Card {
                    key: e.key.clone(),
                    bunker: e.bunker.map(|b| b.index),
                    grid: e.grid.clone(),
                    code: e.code.clone(),
                    count: e.count,
                    unclear: e.unclear,
                    corrected: e.corrected,
                    position,
                    url: position.map(|(x, z)| izurvive_url(x, z, 7)),
                    updated: Instant::now(),
                };
                match s.cards.iter_mut().find(|c| c.key == card.key) {
                    Some(c) => *c = card,
                    None => s.cards.push(card),
                }
                s.finished = Some(Finished {
                    digits: r.digits.clone(),
                    support: r.support.clone(),
                    starts: r.digit_starts.clone(),
                    heard: Instant::now(),
                    bunker: e.bunker.map(|b| b.index),
                });
                s.preview = None;
                s.burst_beeps = 0;
                fresh.push(e.key);
            }
            if s.notes.len() > 4 {
                let cut = s.notes.len() - 4;
                s.notes.drain(..cut);
            }
        }
        s.changed.extend(fresh.iter().cloned());
        fresh
    }

    fn after(&self, fresh: Vec<String>) {
        if !fresh.is_empty() {
            self.ctx.request_repaint();
        }
    }
}

fn accepted(r: &BunkerRecord, system: bool) -> bool {
    // system-wide audio holds music and voice: only a message whose grid is a real bunker counts
    let known = r.snap.kind() != SnapKind::None;
    known || (!system && r.grid.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine(generation: u64) -> Engine {
        let shared = Arc::new(Mutex::new(Shared::new(Source::Game)));
        let mut e = Engine::new(shared, egui::Context::default(), None, Source::Game);
        e.generation = generation;
        e
    }

    fn msg(generation: u64, event: Event) -> Message {
        Message { generation, event }
    }

    #[test]
    fn events_of_a_replaced_capture_are_dropped() {
        let mut e = engine(3);
        e.event(msg(2, Event::Status(Status::Listening)));
        assert_eq!(e.lock().status, Status::WaitingForGame);
        e.event(msg(2, Event::Samples(synth::demo_transmission(7))));
        assert!(e.lock().frames.is_empty());
        e.event(msg(2, Event::DemoFinished));
        assert!(!e.lock().demo_done);
    }

    #[test]
    fn events_of_the_current_capture_are_applied() {
        let mut e = engine(3);
        e.event(msg(3, Event::Status(Status::Listening)));
        assert_eq!(e.lock().status, Status::Listening);
        e.event(msg(3, Event::Samples(vec![0; 4410])));
        assert!(!e.lock().frames.is_empty());
        e.event(msg(3, Event::DemoFinished));
        assert!(e.lock().demo_done);
    }

    #[test]
    fn a_restart_retires_the_queued_events_of_the_old_capture() {
        let mut e = engine(0);
        let (tx, _rx) = mpsc::channel();
        e.restart(&tx);
        let old = e.generation;
        e.command(Command::StopDemo, &tx);
        assert_eq!(e.generation, old + 1);
        e.event(msg(old, Event::Status(Status::Listening)));
        assert_ne!(e.lock().status, Status::Listening);
    }
}
