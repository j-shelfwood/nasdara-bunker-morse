use std::time::Instant;

use nasdara_decoder::bunkers::{SnapKind, cell_center, izurvive_url};
use nasdara_decoder::bursts::{BunkerRecord, Burst};

use super::HISTORY_S;
use super::state::{
    Beep, Card, Finished, MIN_BEEP_S, MIN_GAP_S, Mode, Note, OFF_DB, ON_DB, QUIET_S, Shared,
    trim_notes,
};
use super::worker::Engine;
use crate::capture::Source;
use crate::log::{DebugLog, LogLine};

impl Engine {
    pub(super) fn samples(&mut self, pcm: &[i16]) {
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
    pub(super) fn track_beeps(&mut self, s: &mut Shared, t: f64, db: f32) {
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
    pub(super) fn close_beep(&mut self, s: &mut Shared, t: f64) {
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

    pub(super) fn bursts(&mut self, bursts: Vec<Burst>) {
        let shared = self.shared.clone();
        let mut s = shared.lock().unwrap_or_else(|p| p.into_inner());
        let mut lines = Vec::new();
        let fresh = self.digest(&mut s, bursts, &mut lines);
        drop(s);
        DebugLog::write(lines);
        self.after(fresh);
    }

    /// Updates the shared state; the debug log lines go to `lines` for the caller to write once the lock is released.
    pub(super) fn digest(
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
            if s.frozen || b.ignored.is_some() {
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
                    support: painted_support(r),
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

    pub(super) fn after(&self, fresh: Vec<String>) {
        if !fresh.is_empty() {
            self.ctx.request_repaint();
        }
    }
}

fn accepted(r: &BunkerRecord, system: bool) -> bool {
    // system-wide audio holds music and voice: only a message whose grid is a real bunker counts
    let known = r.snap.kind() != SnapKind::None;
    known || (!system && r.grid.chars().all(|c| c.is_ascii_digit()))
}

/// Support as the digit bar paints it: a grid that is exactly one of the 23 bunkers is certain whatever its tone
/// evidence, so its six digits are not shown as weak.
pub(super) fn painted_support(r: &BunkerRecord) -> Vec<f32> {
    let mut support = r.support.clone();
    if r.snap.kind() == SnapKind::Exact {
        support.iter_mut().take(6).for_each(|s| *s = 1.0);
    }
    support
}
