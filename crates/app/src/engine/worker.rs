use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use nasdara_decoder::ledger::BunkerLedger;
use nasdara_decoder::pipeline::Pipeline;
use nasdara_decoder::synth;

use super::Command;
use super::state::{Mode, Note, Shared, trim_notes};
use crate::capture::{CHANNEL_CAPACITY, Capture, Event, Message, Source, Status};
use crate::log::DebugLog;

const DEMO_SPEED: f64 = if cfg!(windows) { 1.0 } else { 2.0 };

pub(super) struct Engine {
    pub(super) shared: Arc<Mutex<Shared>>,
    pub(super) ctx: egui::Context,
    pub(super) log: Option<DebugLog>,
    pub(super) pipeline: Pipeline,
    pub(super) ledger: BunkerLedger,
    pub(super) capture: Option<Capture>,
    /// The note about dropped audio is added once per session.
    pub(super) drops_noted: bool,
    /// Packets Windows flagged as discontinuous this session, and whether the note was added.
    pub(super) gaps: u64,
    /// Session counter: bumped on every restart; events of an older generation belong to a replaced capture.
    pub(super) generation: u64,
    pub(super) source: Source,
    pub(super) mode: Mode,
    pub(super) tone_on: bool,
    pub(super) open: Option<usize>,
    pub(super) last_beep_end: f64,
    pub(super) silent_samples: u64,
    pub(super) peak: f32,
}

impl Engine {
    pub(super) fn new(
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
            gaps: 0,
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

    pub(super) fn run(mut self, commands: Receiver<Command>) {
        let _alive = AliveGuard {
            shared: self.shared.clone(),
            ctx: self.ctx.clone(),
        };
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

    pub(super) fn restart(&mut self, tx: &SyncSender<Message>) {
        self.capture = None;
        self.generation += 1;
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
                !cfg!(test),
                self.generation,
                tx.clone(),
            ),
        });
        self.ctx.request_repaint();
    }

    pub(super) fn note_drops(&mut self, dropped: u64) {
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
    pub(super) fn reset_stream(&mut self) {
        self.pipeline = Pipeline::new();
        self.drops_noted = false;
        self.gaps = 0;
        self.tone_on = false;
        self.open = None;
        self.last_beep_end = f64::NEG_INFINITY;
        self.silent_samples = 0;
    }

    /// The capture ended: close what is open, hand the decoder's pending burst through the normal path and start a
    /// new stream, so nothing stays frozen on the old clock until the next session.
    pub(super) fn session_ended(&mut self) {
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

    pub(super) fn command(&mut self, c: Command, tx: &SyncSender<Message>) {
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
            Command::SetFrozen(frozen) => {
                self.lock().frozen = frozen;
                self.ctx.request_repaint();
            }
            Command::ClearCards => {
                if self.lock().frozen {
                    return;
                }
                self.ledger = BunkerLedger::new();
                let mut s = self.lock();
                s.cards.clear();
                s.finished = None;
                s.notes.clear();
            }
        }
    }

    pub(super) fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(super) fn event(&mut self, m: Message) {
        if m.generation != self.generation {
            return;
        }
        match m.event {
            Event::Status(status) => {
                self.lock().status = status;
                self.ctx.request_repaint();
            }
            Event::Samples(pcm) => self.samples(&pcm),
            Event::Discontinuity(n) => {
                self.gaps += u64::from(n);
                if self.gaps == u64::from(n) {
                    let mut s = self.lock();
                    s.notes.push(Note::Warn(
                        "Windows reported missing audio; a digit may be misread".into(),
                    ));
                    trim_notes(&mut s.notes);
                    drop(s);
                    self.ctx.request_repaint();
                }
            }
            Event::SessionEnded => self.session_ended(),
        }
    }
}

/// Marks the engine as gone when its thread ends, on any path.
pub(super) struct AliveGuard {
    pub(super) shared: Arc<Mutex<Shared>>,
    pub(super) ctx: egui::Context,
}

impl Drop for AliveGuard {
    fn drop(&mut self) {
        self.shared
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .engine_alive = false;
        self.ctx.request_repaint();
    }
}
