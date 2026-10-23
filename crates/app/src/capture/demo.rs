use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::{Event, SAMPLE_RATE, Sink, Status};

/// Silence between two plays of the clip, in clip seconds. Longer than the decoder's quiet threshold, so every play
/// closes its last burst the way the repeating broadcast does.
const GAP_S: usize = 12;

/// Feeds a clip into the engine in 10 ms steps, `speed` times faster than real time, and repeats it until stopped.
pub fn play(pcm: Vec<i16>, speed: f64, sink: Sink, stop: Arc<AtomicBool>) {
    sink.send(Event::Status(Status::Listening), &stop);
    let mut cycle = pcm;
    cycle.resize(cycle.len() + GAP_S * SAMPLE_RATE, 0);
    let step = (SAMPLE_RATE as f64 / 100.0 * speed) as usize;
    let started = Instant::now();
    let (mut at, mut sent) = (0, 0usize);
    while !stop.load(Ordering::Relaxed) {
        let due = (started.elapsed().as_secs_f64() * 100.0) as usize;
        while sent < due {
            let end = (at + step).min(cycle.len());
            if stop.load(Ordering::Relaxed)
                || !sink.send(Event::Samples(cycle[at..end].to_vec()), &stop)
            {
                return;
            }
            at = if end == cycle.len() { 0 } else { end };
            sent += 1;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::CHANNEL_CAPACITY;

    #[test]
    fn the_clip_repeats_after_a_silent_gap_until_stopped() {
        let (tx, rx) = std::sync::mpsc::sync_channel(CHANNEL_CAPACITY);
        let stop = Arc::new(AtomicBool::new(false));
        let clip = vec![1000i16; SAMPLE_RATE];
        let cycle = clip.len() + GAP_S * SAMPLE_RATE;
        let flag = stop.clone();
        let player = std::thread::spawn(move || {
            play(clip, 200.0, Sink::new(0, tx), flag);
        });
        let mut heard = Vec::new();
        while heard.len() < 2 * cycle + SAMPLE_RATE {
            if let Event::Samples(pcm) = rx.recv().expect("player ended early").event {
                heard.extend(pcm);
            }
        }
        stop.store(true, Ordering::Relaxed);
        drop(rx);
        player.join().expect("player thread");
        assert!(heard[..SAMPLE_RATE].iter().all(|&s| s == 1000));
        assert!(heard[SAMPLE_RATE..cycle].iter().all(|&s| s == 0));
        assert!(heard[cycle..cycle + SAMPLE_RATE].iter().all(|&s| s == 1000));
    }
}
