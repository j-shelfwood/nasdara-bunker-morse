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

pub struct Capture {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Capture {
    pub fn live(source: Source, tx: Sender<Event>) -> Capture {
        Self::spawn("capture", move |stop| live(source, tx, stop))
    }

    /// Plays `pcm` into the pipeline at `speed` times real time.
    pub fn demo(pcm: Vec<i16>, speed: f64, tx: Sender<Event>) -> Capture {
        Self::spawn("demo", move |stop| demo::play(pcm, speed, tx, stop))
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
fn live(source: Source, tx: Sender<Event>, stop: Arc<AtomicBool>) {
    win::run(source, tx, stop);
}

#[cfg(not(windows))]
fn live(_source: Source, tx: Sender<Event>, _stop: Arc<AtomicBool>) {
    let _ = tx.send(Event::Status(Status::Failed(
        "Live capture needs Windows. Use the demo.".into(),
    )));
}
