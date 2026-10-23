//! Audio sources. Every source delivers mono 16-bit samples at 44.1 kHz on a steady wall clock,
//! because the decoder reads time from the sample count.

mod demo;
#[cfg(any(windows, test))]
mod pacer;
#[cfg(windows)]
mod win;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError};

use serde::{Deserialize, Serialize};

pub const SAMPLE_RATE: usize = 44_100;

/// Messages the channel from capture to engine holds: about 5 s of 10 ms chunks.
pub const CHANNEL_CAPACITY: usize = 500;

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
    /// Windows flagged `n` packets as discontinuous: audio went missing before them.
    Discontinuity(u32),
    /// The live capture ended (the game exited, the device failed); the stream stops here.
    SessionEnded,
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
    tx: SyncSender<Message>,
    /// Sample chunks thrown away because the engine had fallen behind.
    dropped: Arc<AtomicU64>,
}

impl Sink {
    pub fn new(generation: u64, tx: SyncSender<Message>) -> Sink {
        Sink {
            generation,
            tx,
            dropped: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Live audio never waits for the engine: when the channel is full the chunk is dropped and counted.
    /// `false` when the engine is gone.
    pub fn send_samples(&self, pcm: Vec<i16>) -> bool {
        let message = Message {
            generation: self.generation,
            event: Event::Samples(pcm),
        };
        match self.tx.try_send(message) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                true
            }
            Err(TrySendError::Disconnected(_)) => false,
        }
    }

    /// Waits for room: for status changes and for the demo, which must not lose samples. `false` when the engine is gone.
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
    dropped: Arc<AtomicU64>,
}

impl Capture {
    pub fn live(source: Source, generation: u64, tx: SyncSender<Message>) -> Capture {
        Self::spawn("capture", Sink::new(generation, tx), move |sink, stop| {
            live(source, sink, stop)
        })
    }

    /// Plays `pcm` into the pipeline at `speed` times real time.
    pub fn demo(pcm: Vec<i16>, speed: f64, generation: u64, tx: SyncSender<Message>) -> Capture {
        Self::spawn("demo", Sink::new(generation, tx), move |sink, stop| {
            demo::play(pcm, speed, sink, stop)
        })
    }

    fn spawn(
        name: &str,
        sink: Sink,
        run: impl FnOnce(Sink, Arc<AtomicBool>) + Send + 'static,
    ) -> Capture {
        let stop = Arc::new(AtomicBool::new(false));
        let dropped = sink.dropped.clone();
        let flag = stop.clone();
        let _ = std::thread::Builder::new()
            .name(name.into())
            .spawn(move || run(sink, flag));
        Capture { stop, dropped }
    }

    /// Chunks dropped since the last call.
    pub fn take_dropped(&self) -> u64 {
        self.dropped.swap(0, Ordering::Relaxed)
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
        let (tx, _rx) = std::sync::mpsc::sync_channel(1);
        let capture = Capture::spawn("stuck", Sink::new(0, tx), move |_, stop| {
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

    #[test]
    fn a_full_channel_drops_chunks_and_counts_them() {
        let (tx, rx) = mpsc::sync_channel(2);
        let sink = Sink::new(0, tx);
        let dropped = sink.dropped.clone();
        for _ in 0..5 {
            assert!(sink.send_samples(vec![1; 441]));
        }
        assert_eq!(dropped.load(Ordering::Relaxed), 3);
        assert_eq!(rx.try_iter().count(), 2);
        drop(rx);
        assert!(!sink.send_samples(vec![1]));
    }
}
