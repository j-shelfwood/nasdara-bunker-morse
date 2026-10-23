use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use super::{Event, SAMPLE_RATE, Status};

/// Feeds a clip into the engine in 10 ms steps, `speed` times faster than real time.
pub fn play(pcm: Vec<i16>, speed: f64, tx: Sender<Event>, stop: Arc<AtomicBool>) {
    let _ = tx.send(Event::Status(Status::Listening));
    let step = (SAMPLE_RATE as f64 / 100.0 * speed) as usize;
    let started = Instant::now();
    let mut at = 0;
    while at < pcm.len() && !stop.load(Ordering::Relaxed) {
        let due = (started.elapsed().as_secs_f64() * 100.0) as usize * step;
        while at < due.min(pcm.len()) {
            let end = (at + step).min(pcm.len());
            if tx.send(Event::Samples(pcm[at..end].to_vec())).is_err() {
                return;
            }
            at = end;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    if !stop.load(Ordering::Relaxed) {
        let _ = tx.send(Event::DemoFinished);
    }
}
