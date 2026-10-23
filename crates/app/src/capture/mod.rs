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

/// Dropping a capture signals its thread to stop and does not wait for it: a thread stuck in a Windows call must
/// not freeze the engine. Whatever it still sends is dropped by the engine, which has moved on to a newer generation.
pub struct Capture {
    stop: Arc<AtomicBool>,
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
        let _ = std::thread::Builder::new()
            .name(name.into())
            .spawn(move || run(flag));
        Capture { stop }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    #[test]
    fn dropping_a_capture_does_not_wait_for_a_stuck_thread() {
        let (release, parked) = mpsc::channel::<()>();
        let (stopped_tx, stopped) = mpsc::channel();
        let capture = Capture::spawn("stuck", move |stop| {
            // a thread blocked in a call that never returns, until the test lets go
            let _ = parked.recv();
            let _ = stopped_tx.send(stop.load(Ordering::Relaxed));
        });
        let t = Instant::now();
        drop(capture);
        assert!(t.elapsed() < Duration::from_millis(500), "drop blocked");
        // the detached thread still sees the stop flag when it wakes
        release.send(()).unwrap();
        assert_eq!(stopped.recv_timeout(Duration::from_secs(5)), Ok(true));
    }
}
