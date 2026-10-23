//! The listening side: takes audio from a source, runs the decoder and keeps the state the window draws.

use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use nasdara_decoder::bunkers::{SnapKind, cell_center, izurvive_url};
use nasdara_decoder::bursts::{BunkerRecord, Burst};
use nasdara_decoder::ledger::BunkerLedger;
use nasdara_decoder::pipeline::{Pipeline, Preview};
use nasdara_decoder::synth;

use crate::capture::{CHANNEL_CAPACITY, Capture, Event, Message, Source, Status};
use crate::log::{DebugLog, LogLine};

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
        self.status == Status::Listening
            && self.burst_beeps >= 4
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
    /// The note about dropped audio is added once per session.
    drops_noted: bool,
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
            drops_noted: false,
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
        let (tx, events) = mpsc::sync_channel(CHANNEL_CAPACITY);
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
            let dropped = self.capture.as_ref().map_or(0, Capture::take_dropped);
            self.note_drops(dropped);
        }
    }

    fn restart(&mut self, tx: &SyncSender<Message>) {
        self.capture = None;
        self.generation += 1;
        self.drops_noted = false;
        self.reset_stream();
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

    fn note_drops(&mut self, dropped: u64) {
        if dropped == 0 || self.drops_noted {
            return;
        }
        self.drops_noted = true;
        let mut s = self.lock();
        s.notes.push(Note::Warn(
            "Audio was dropped because the app fell behind".into(),
        ));
        trim_notes(&mut s.notes);
        drop(s);
        self.ctx.request_repaint();
    }

    /// A fresh decoder and beep tracker for a new stream.
    fn reset_stream(&mut self) {
        self.pipeline = Pipeline::new();
        self.tone_on = false;
        self.open = None;
        self.last_beep_end = f64::NEG_INFINITY;
        self.silent_samples = 0;
    }

    /// The capture ended: close what is open, hand the decoder's pending burst through the normal path and start a
    /// new stream, so nothing stays frozen on the old clock until the next session.
    fn session_ended(&mut self) {
        {
            let shared = self.shared.clone();
            let mut s = shared.lock().unwrap_or_else(|p| p.into_inner());
            let t = s.clock;
            self.close_beep(&mut s, t);
        }
        let bursts = self.pipeline.flush();
        self.bursts(bursts);
        {
            let mut s = self.lock();
            s.frames.clear();
            s.beeps.clear();
            s.burst_beeps = 0;
            s.preview = None;
            s.clock = 0.0;
            s.clock_at = Instant::now();
            s.level = 0.0;
            s.silent_for = 0.0;
        }
        self.reset_stream();
        self.peak = 0.0;
        self.ctx.request_repaint();
    }

    fn command(&mut self, c: Command, tx: &SyncSender<Message>) {
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
            Event::SessionEnded => self.session_ended(),
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
        let mut lines = Vec::new();
        let fresh = if out.bursts.is_empty() {
            Vec::new()
        } else {
            self.digest(&mut s, out.bursts, &mut lines)
        };
        drop(s);
        DebugLog::write(lines);
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
        } else if !on && self.tone_on {
            self.close_beep(s, t);
        }
        self.tone_on = on;
    }

    /// Ends the open beep at `t`; one shorter than a real beep is a glitch and disappears.
    fn close_beep(&mut self, s: &mut Shared, t: f64) {
        if let Some(i) = self.open.take()
            && let Some(b) = s.beeps.get_mut(i)
        {
            b.end = Some(t);
            self.last_beep_end = t;
            if b.length(t) < MIN_BEEP_S {
                s.beeps.remove(i);
                s.burst_beeps = s.burst_beeps.saturating_sub(1);
            }
        }
        self.tone_on = false;
    }

    fn bursts(&mut self, bursts: Vec<Burst>) {
        let shared = self.shared.clone();
        let mut s = shared.lock().unwrap_or_else(|p| p.into_inner());
        let mut lines = Vec::new();
        let fresh = self.digest(&mut s, bursts, &mut lines);
        drop(s);
        DebugLog::write(lines);
        self.after(fresh);
    }

    /// Updates the shared state; the debug log lines go to `lines` for the caller to write once the lock is released.
    fn digest(
        &mut self,
        s: &mut Shared,
        bursts: Vec<Burst>,
        lines: &mut Vec<LogLine>,
    ) -> Vec<String> {
        let mut fresh = Vec::new();
        for b in bursts {
            let system = s.source == Source::System && s.mode == Mode::Live;
            let shown = b.records.iter().filter(|r| accepted(r, system)).count();
            if let Some(log) = &self.log {
                lines.extend(log.burst(&b, shown));
            }
            if b.ignored.is_some() {
                continue;
            }
            if b.records.is_empty() {
                if let Some(note) = &b.note {
                    s.notes.push(Note::Info(format!("A transmission of {} beeps was heard but could not be read ({note}). It repeats, so keep listening.", b.elements)));
                    trim_notes(&mut s.notes);
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
            trim_notes(&mut s.notes);
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

const MAX_NOTES: usize = 4;

fn trim_notes(notes: &mut Vec<Note>) {
    if notes.len() > MAX_NOTES {
        let cut = notes.len() - MAX_NOTES;
        notes.drain(..cut);
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
        let (tx, _rx) = mpsc::sync_channel(CHANNEL_CAPACITY);
        e.restart(&tx);
        let old = e.generation;
        e.command(Command::StopDemo, &tx);
        assert_eq!(e.generation, old + 1);
        e.event(msg(old, Event::Status(Status::Listening)));
        assert_ne!(e.lock().status, Status::Listening);
    }

    /// A burst that was heard but gave no record.
    fn unreadable() -> Burst {
        Burst {
            start: 1.0,
            elements: 12,
            records: Vec::new(),
            note: Some("too few digits".into()),
            ignored: None,
            span: (1.0, 3.0),
            trace: None,
            beeps: Vec::new(),
            first_gap: 0.5,
        }
    }

    #[test]
    fn digest_leaves_the_debug_log_to_the_caller() {
        let path =
            std::env::temp_dir().join(format!("bunker-morse-lock-{}.log", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut e = engine(0);
        e.log = Some(DebugLog::new(path.clone(), false));
        let shared = e.shared.clone();
        let mut lines = Vec::new();
        {
            let mut s = shared.lock().unwrap();
            e.digest(&mut s, vec![unreadable()], &mut lines);
        }
        assert!(
            !path.exists(),
            "digest wrote a file while the lock was held"
        );
        assert_eq!(lines.len(), 1);
        DebugLog::write(lines);
        let written = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(written.contains("\"elements\":12"), "{written}");
    }

    #[test]
    fn notes_stay_trimmed_when_bursts_are_unreadable() {
        let mut e = engine(0);
        let shared = e.shared.clone();
        let mut s = shared.lock().unwrap();
        for _ in 0..50 {
            e.digest(&mut s, vec![unreadable()], &mut Vec::new());
        }
        assert_eq!(s.notes.len(), MAX_NOTES);
    }

    fn open_beep(e: &Engine, beeps: usize) {
        let mut s = e.lock();
        s.status = Status::Listening;
        s.clock = 10.0;
        s.burst_beeps = beeps;
        for i in 0..beeps {
            s.beeps.push_back(Beep {
                start: 1.0 + i as f64,
                end: if i + 1 == beeps {
                    None
                } else {
                    Some(1.5 + i as f64)
                },
            });
        }
        s.preview = None;
    }

    #[test]
    fn receiving_needs_a_listening_status() {
        let e = engine(0);
        open_beep(&e, 5);
        assert!(e.lock().receiving());
        e.lock().status = Status::WaitingForGame;
        assert!(!e.lock().receiving());
    }

    #[test]
    fn a_ended_session_closes_the_open_beep_and_starts_a_clean_stream() {
        let mut e = engine(1);
        open_beep(&e, 5);
        e.open = Some(4);
        e.tone_on = true;
        e.event(msg(1, Event::SessionEnded));
        let s = e.lock();
        assert!(s.beeps.is_empty() && s.frames.is_empty());
        assert_eq!(s.burst_beeps, 0);
        assert!(s.preview.is_none());
        assert!(!s.receiving());
        drop(s);
        assert!(e.open.is_none() && !e.tone_on);
        assert_eq!(e.pipeline.pending(), 0);
    }

    #[test]
    fn a_ended_session_flushes_the_pending_burst() {
        let pcm = synth::demo_transmission(7);
        // stop right after the last loud sample, before the silence that would end the burst by itself
        let end = pcm.iter().rposition(|s| s.unsigned_abs() > 2000).unwrap();
        let mut e = engine(1);
        for c in synth::chunks(&pcm[..end]) {
            e.event(msg(1, Event::Samples(c.to_vec())));
        }
        let heard: usize = e.lock().cards.iter().map(|c| c.count).sum();
        e.event(msg(1, Event::SessionEnded));
        let after: usize = e.lock().cards.iter().map(|c| c.count).sum();
        assert!(
            after > heard,
            "heard {heard} before, {after} after the flush"
        );
    }

    #[test]
    fn dropped_audio_adds_one_note_per_session() {
        let mut e = engine(0);
        e.note_drops(0);
        assert!(e.lock().notes.is_empty());
        e.note_drops(3);
        e.note_drops(7);
        assert_eq!(e.lock().notes.len(), 1);
        let (tx, _rx) = mpsc::sync_channel(CHANNEL_CAPACITY);
        e.restart(&tx);
        e.note_drops(1);
        assert_eq!(e.lock().notes.len(), 2);
    }
}
