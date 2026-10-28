//! What the bar shows, derived from the shared engine state: ten digit slots, the symbols of the digit in progress,
//! and the headline with its hint.

use crate::engine::Shared;
use nasdara_decoder::bunkers::Bunker;

pub(super) const DIGITS: usize = 10;
/// Length above which a tone counts as a dash (dots are ~0.09 s, dashes ~0.48 s).
pub const DASH_FROM_S: f64 = 0.28;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Phase {
    Empty,
    /// The digit being sent right now.
    Active,
    /// Read from the audio so far; a later pass may change it.
    Provisional,
    Final,
}

#[derive(Clone)]
pub(super) struct Slot {
    pub ch: Option<char>,
    pub support: f32,
    pub phase: Phase,
    /// The digit the one remaining candidate bunker has in this still-empty grid slot.
    pub ghost: Option<char>,
}

pub struct View {
    pub(super) slots: Vec<Slot>,
    /// Symbols heard of the digit in progress.
    pub(super) live_symbols: String,
    pub headline: String,
    pub hint: String,
    /// Bunkers that still fit the grid digits heard so far; empty outside a transmission and when none fits.
    pub candidates: Vec<&'static Bunker>,
}

impl View {
    /// "3 bunkers possible" or "Only Bunker 7 fits"; `None` while every bunker is still possible or none fits.
    pub fn candidate_caption(&self, short: bool) -> Option<String> {
        match (self.candidates.as_slice(), short) {
            ([], _) => None,
            ([b], false) => Some(format!("Only Bunker {} fits", b.index)),
            ([b], true) => Some(format!("Bunker {}", b.index)),
            (c, _) if c.len() >= nasdara_decoder::bunkers::bunkers().len() => None,
            (c, false) => Some(format!("{} bunkers possible", c.len())),
            (c, true) => Some(format!("{} possible", c.len())),
        }
    }
}

/// Symbols heard so far of the digit being sent; empty when no message is arriving.
pub fn live_symbols(s: &Shared) -> String {
    view(s).live_symbols
}

pub fn view(s: &Shared) -> View {
    let now = s.now();
    let mut slots = vec![
        Slot {
            ch: None,
            support: 0.0,
            phase: Phase::Empty,
            ghost: None
        };
        DIGITS
    ];
    let mut live_symbols = String::new();
    if s.receiving() {
        let preview = s.preview.as_ref();
        // The decoder marks the trailing digit unsettled while its beeps may still be arriving (three dashes of a 0
        // look like a 2 with its dots lost); only settled digits are shown.
        let settled = preview.map_or(0, |p| {
            p.digits
                .iter()
                .take(DIGITS)
                .take_while(|d| d.settled)
                .count()
        });
        if let Some(p) = preview {
            for (i, d) in p.digits.iter().take(settled).enumerate() {
                slots[i] = Slot {
                    ch: Some(d.ch),
                    support: d.support,
                    phase: Phase::Provisional,
                    ghost: None,
                };
            }
        }
        let done = settled;
        let after = preview
            .and_then(|p| p.digits.get(done.wrapping_sub(1)))
            .map_or(f64::NEG_INFINITY, |d| d.window_end - 0.2);
        live_symbols = s
            .beeps
            .iter()
            .rev()
            .take(5)
            .filter(|b| b.start > after)
            .map(|b| {
                if b.length(now) > DASH_FROM_S {
                    '-'
                } else {
                    '.'
                }
            })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if done < DIGITS {
            slots[done].phase = Phase::Active;
        }
        let candidates = preview.map_or_else(Vec::new, |p| p.candidates.clone());
        if let [b] = candidates[..] {
            for (i, g) in b.grid.chars().enumerate().skip(done) {
                slots[i].ghost = Some(g);
            }
        }
        let n = done.min(DIGITS);
        return View {
            candidates,
            slots,
            live_symbols,
            headline: format!("Receiving a transmission, digit {} of {DIGITS}", (n + 1).min(DIGITS)),
            hint: "Dim digits are tentative and can still change. The message repeats, so a missed digit is filled in on the next pass.".into(),
        };
    }

    if let Some(f) = &s.finished {
        for (i, ch) in f.digits.iter().take(DIGITS).enumerate() {
            slots[i] = Slot {
                ch: Some(*ch),
                support: f.support.get(i).copied().unwrap_or(0.0),
                phase: Phase::Final,
                ghost: None,
            };
        }
        let ago = f.heard.elapsed().as_secs();
        let which = f
            .bunker
            .map_or("a grid outside the known bunkers".to_string(), |b| {
                format!("bunker {b}")
            });
        return View {
            candidates: Vec::new(),
            slots,
            live_symbols,
            headline: format!("Last transmission: {which}, {}", ago_text(ago)),
            hint:
                "The next one starts within a few minutes. The cards below keep every bunker heard."
                    .into(),
        };
    }

    let (headline, hint) = match s.status {
        crate::capture::Status::Listening => (
            "Listening for a bunker broadcast".to_string(),
            "Hold a powered transceiver tuned to the bunker frequency. The broadcast runs from 17:45 to 06:00 game time.".to_string(),
        ),
        crate::capture::Status::WaitingForGame => ("Waiting for DayZ to start".to_string(), "The app attaches by itself as soon as the game is running.".to_string()),
        crate::capture::Status::Failed(ref e) => ("Cannot listen".to_string(), e.clone()),
    };
    View {
        candidates: Vec::new(),
        slots,
        live_symbols,
        headline,
        hint,
    }
}

fn ago_text(secs: u64) -> String {
    match secs {
        0..=4 => "just now".into(),
        5..=59 => format!("{secs} s ago"),
        60..=3599 => format!("{} min ago", secs / 60),
        _ => format!("{} h ago", secs / 3600),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::{Source, Status};
    use crate::engine::Beep;
    use nasdara_decoder::bunkers::{bunkers, candidates};
    use nasdara_decoder::pipeline::{Preview, PreviewDigit};

    fn receiving(heard: &str) -> Shared {
        let mut s = Shared::new(Source::Game);
        s.status = Status::Listening;
        s.burst_beeps = 6;
        s.beeps.push_back(Beep {
            start: 0.0,
            end: None,
        });
        let digits: Vec<PreviewDigit> = heard
            .chars()
            .enumerate()
            .map(|(i, ch)| PreviewDigit {
                ch,
                support: 0.9,
                margin: 0.5,
                start: i as f64,
                window_end: i as f64 + 0.9,
                settled: true,
            })
            .collect();
        let chars: Vec<char> = heard.chars().collect();
        s.preview = Some(Preview {
            start: 0.0,
            candidates: candidates(&chars),
            digits,
        });
        s
    }

    #[test]
    fn the_candidate_count_reaches_the_view_and_a_single_bunker_ghosts_its_remaining_digits() {
        let grid = bunkers()[19].grid.clone();
        let one = view(&receiving(&grid[..1]));
        assert!(one.candidates.len() > 1);
        assert_eq!(
            one.candidate_caption(false),
            Some(format!("{} bunkers possible", one.candidates.len()))
        );
        assert!(one.slots.iter().all(|s| s.ghost.is_none()));

        let mut n = 2;
        let narrowed = loop {
            let v = view(&receiving(&grid[..n]));
            if v.candidates.len() == 1 {
                break v;
            }
            n += 1;
        };
        assert_eq!(
            narrowed.candidate_caption(false),
            Some("Only Bunker 20 fits".into())
        );
        let ghosts: String = narrowed.slots.iter().filter_map(|s| s.ghost).collect();
        assert_eq!(ghosts, grid[n..]);
        assert!(narrowed.slots[..n].iter().all(|s| s.ghost.is_none()));
    }

    #[test]
    fn no_transmission_means_no_candidates() {
        let v = view(&Shared::new(Source::Game));
        assert!(v.candidates.is_empty() && v.candidate_caption(false).is_none());
    }
}
