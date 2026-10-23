//! Audio sources. Every source delivers mono 16-bit samples at 44.1 kHz on a steady wall clock,
//! because the decoder reads time from the sample count.

mod demo;
#[cfg(any(windows, test))]
mod pacer;
#[cfg(windows)]
mod win;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;

use serde::{Deserialize, Serialize};

pub const SAMPLE_RATE: usize = 44_100;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    /// Only the audio of the DayZ process.
    #[default]
    Game,
    /// Everything the PC plays, for setups where Windows does not attribute the game's sound to DayZ.
    System,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    WaitingForGame,
    Listening,
    Failed(String),
}

pub enum Event {
    Status(Status),
    Samples(Vec<i16>),
    /// A demo clip ran to its end.
    DemoFinished,
}

/// An event tagged with the session (generation) of the capture that produced it. The engine drops events of
/// captures it has already replaced, because their thread may still be running and its events still queued.
pub struct Message {
    pub generation: u64,
    pub event: Event,
}

/// What a capture thread sends through: tags every event with the generation the capture was created with.
#[derive(Clone)]
pub struct Sink {
    generation: u64,
    tx: Sender<Message>,
}

impl Sink {
    pub fn new(generation: u64, tx: Sender<Message>) -> Sink {
        Sink { generation, tx }
    }

    /// `false` when the engine is gone.
    pub fn send(&self, event: Event) -> bool {
        self.tx
            .send(Message {
                generation: self.generation,
                event,
            })
            .is_ok()
    }
}

pub struct Capture {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Capture {
    pub fn live(source: Source, generation: u64, tx: Sender<Message>) -> Capture {
        let sink = Sink::new(generation, tx);
        Self::spawn("capture", move |stop| live(source, sink, stop))
    }

    /// Plays `pcm` into the pipeline at `speed` times real time.
    pub fn demo(pcm: Vec<i16>, speed: f64, generation: u64, tx: Sender<Message>) -> Capture {
        let sink = Sink::new(generation, tx);
        Self::spawn("demo", move |stop| demo::play(pcm, speed, sink, stop))
    }

    fn spawn(name: &str, run: impl FnOnce(Arc<AtomicBool>) + Send + 'static) -> Capture {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new()
            .name(name.into())
            .spawn(move || run(flag))
            .ok();
        Capture { stop, thread }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(windows)]
fn live(source: Source, sink: Sink, stop: Arc<AtomicBool>) {
    win::run(source, sink, stop);
}

#[cfg(not(windows))]
fn live(_source: Source, sink: Sink, _stop: Arc<AtomicBool>) {
    sink.send(Event::Status(Status::Failed(
        "Live capture needs Windows. Use the demo.".into(),
    )));
}
