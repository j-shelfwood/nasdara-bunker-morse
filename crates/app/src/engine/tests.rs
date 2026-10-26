use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use nasdara_decoder::bursts::Burst;
use nasdara_decoder::synth;

use super::state::MAX_NOTES;
use super::worker::{AliveGuard, Engine};
use super::{Beep, Command, Mode, Shared};
use crate::capture::{CHANNEL_CAPACITY, Event, Message, Source, Status};
use crate::log::DebugLog;

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
}

#[test]
fn events_of_the_current_capture_are_applied() {
    let mut e = engine(3);
    e.event(msg(3, Event::Status(Status::Listening)));
    assert_eq!(e.lock().status, Status::Listening);
    e.event(msg(3, Event::Samples(vec![0; 4410])));
    assert!(!e.lock().frames.is_empty());
}

#[test]
fn test_mode_switches_on_and_back_to_the_chosen_source() {
    let mut e = engine(0);
    let (tx, _rx) = mpsc::sync_channel(CHANNEL_CAPACITY);
    e.command(Command::StartDemo, &tx);
    assert_eq!(e.lock().mode, Mode::Demo);
    assert_eq!(e.lock().status, Status::Listening);
    e.command(Command::SetSource(Source::System), &tx);
    assert_eq!(e.lock().mode, Mode::Live);
    assert_eq!(e.lock().source, Source::System);
}

#[test]
fn a_restart_retires_the_queued_events_of_the_old_capture() {
    let mut e = engine(0);
    let (tx, _rx) = mpsc::sync_channel(CHANNEL_CAPACITY);
    e.restart(&tx);
    let old = e.generation;
    e.command(Command::SetSource(Source::Game), &tx);
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
        guess: None,
        beeps: Vec::new(),
        first_gap: 0.5,
    }
}

#[test]
fn digest_leaves_the_debug_log_to_the_caller() {
    let path = std::env::temp_dir().join(format!("bunker-morse-lock-{}.log", std::process::id()));
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

#[test]
fn discontinuities_are_counted_and_noted_once_per_session() {
    let mut e = engine(1);
    e.event(msg(1, Event::Discontinuity(2)));
    e.event(msg(1, Event::Discontinuity(1)));
    assert_eq!(e.gaps, 3);
    assert_eq!(e.lock().notes.len(), 1);
    e.event(msg(1, Event::SessionEnded));
    e.event(msg(1, Event::Discontinuity(1)));
    assert_eq!(e.lock().notes.len(), 2);
}

#[test]
fn a_dying_engine_thread_clears_engine_alive() {
    let shared = Arc::new(Mutex::new(Shared::new(Source::Game)));
    assert!(shared.lock().unwrap().engine_alive);
    let guard = AliveGuard {
        shared: shared.clone(),
        ctx: egui::Context::default(),
    };
    let died = std::thread::spawn(move || {
        let _guard = guard;
        panic!("engine test panic");
    })
    .join();
    assert!(died.is_err());
    assert!(
        !shared
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .engine_alive
    );
}
