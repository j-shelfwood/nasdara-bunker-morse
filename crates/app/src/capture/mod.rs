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
use std::time::{Duration, Instant};

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

/// Allows one attempt at a time. A Windows call that never returns would otherwise leak one blocked helper thread
/// per retry; while it is pending, new attempts are refused.
#[cfg(any(windows, test))]
#[derive(Default)]
pub struct InFlight(AtomicBool);

#[cfg(any(windows, test))]
impl InFlight {
    pub const fn new() -> InFlight {
        InFlight(AtomicBool::new(false))
    }

    /// `None` while a previous attempt has not finished; the guard clears the flag when dropped.
    pub fn begin(&'static self) -> Option<InFlightGuard> {
        (!self.0.swap(true, Ordering::AcqRel)).then_some(InFlightGuard(self))
    }
}

#[cfg(any(windows, test))]
pub struct InFlightGuard(&'static InFlight);

#[cfg(any(windows, test))]
impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.0.0.store(false, Ordering::Release);
    }
}

/// The status text for a capture that could not start. Capturing one process needs Windows build 20348 or later,
/// so the game source points at the system source, which works everywhere.
#[cfg(any(windows, test))]
fn failure_message(source: Source, reason: &str) -> String {
    match source {
        Source::Game => {
            format!("Capturing DayZ's audio failed ({reason}). Switch to System audio.")
        }
        Source::System => format!("No audio device: {reason}"),
    }
}

/// How long to wait before trying again after a failed start.
#[cfg(any(windows, test))]
fn retry_after(source: Source) -> std::time::Duration {
    match source {
        Source::Game => std::time::Duration::from_secs(5),
        Source::System => std::time::Duration::from_secs(2),
    }
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

    /// Delivers `event`, retrying until there is room. Gives up when `stop` is set (the engine has moved on to a newer
    /// capture then) and returns `false` only when the engine is gone. For events that must arrive: status changes,
    /// `SessionEnded`, and the demo, which must not lose samples.
    pub fn send(&self, event: Event, stop: &AtomicBool) -> bool {
        self.retry(event, stop, None)
    }

    /// Like `send`, but gives up after `budget` so a slow engine cannot stall a loop that must keep draining.
    pub fn send_within(&self, event: Event, stop: &AtomicBool, budget: Duration) -> bool {
        self.retry(event, stop, Some(Instant::now() + budget))
    }

    fn retry(&self, event: Event, stop: &AtomicBool, until: Option<Instant>) -> bool {
        let mut message = Message {
            generation: self.generation,
            event,
        };
        loop {
            match self.tx.try_send(message) {
                Ok(()) => return true,
                Err(TrySendError::Disconnected(_)) => return false,
                Err(TrySendError::Full(m)) => message = m,
            }
            if stop.load(Ordering::Relaxed) || until.is_some_and(|u| Instant::now() >= u) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
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
fn live(_source: Source, sink: Sink, stop: Arc<AtomicBool>) {
    sink.send(
        Event::Status(Status::Failed(
            "Live capture needs Windows. Use the demo.".into(),
        )),
        &stop,
    );
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

    #[test]
    fn a_failed_game_capture_points_at_system_audio_and_retries_slowly() {
        let m = failure_message(Source::Game, "E_NOTIMPL");
        assert_eq!(
            m,
            "Capturing DayZ's audio failed (E_NOTIMPL). Switch to System audio."
        );
        assert_eq!(retry_after(Source::Game), std::time::Duration::from_secs(5));
        assert_eq!(
            failure_message(Source::System, "gone"),
            "No audio device: gone"
        );
        assert!(retry_after(Source::System) < retry_after(Source::Game));
    }

    fn full_sink() -> (Sink, mpsc::Receiver<Message>) {
        let (tx, rx) = mpsc::sync_channel(1);
        let sink = Sink::new(0, tx);
        assert!(sink.send_samples(vec![0; 10]));
        (sink, rx)
    }

    #[test]
    fn send_within_gives_up_on_a_full_channel_after_its_budget() {
        let (sink, _rx) = full_sink();
        let stop = AtomicBool::new(false);
        let t = Instant::now();
        assert!(sink.send_within(Event::Discontinuity(1), &stop, Duration::from_millis(50)));
        let took = t.elapsed();
        assert!(
            took >= Duration::from_millis(50) && took < Duration::from_secs(2),
            "{took:?}"
        );
    }

    #[test]
    fn send_stops_waiting_when_stopped() {
        let (sink, _rx) = full_sink();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            flag.store(true, Ordering::Relaxed);
        });
        let t = Instant::now();
        sink.send(Event::SessionEnded, &stop);
        assert!(t.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn send_delivers_once_the_engine_catches_up() {
        let (sink, rx) = full_sink();
        let stop = AtomicBool::new(false);
        let drain = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            let first = rx.recv().unwrap();
            assert!(matches!(first.event, Event::Samples(_)));
            rx.recv_timeout(Duration::from_secs(5)).unwrap()
        });
        assert!(sink.send(Event::SessionEnded, &stop));
        assert!(matches!(drain.join().unwrap().event, Event::SessionEnded));
    }

    #[test]
    fn only_one_activation_is_in_flight() {
        static FLAG: InFlight = InFlight::new();
        let first = FLAG.begin().expect("first attempt");
        assert!(
            FLAG.begin().is_none(),
            "second attempt while the first hangs"
        );
        drop(first);
        assert!(
            FLAG.begin().is_some(),
            "free again once the helper finished"
        );
    }
}
