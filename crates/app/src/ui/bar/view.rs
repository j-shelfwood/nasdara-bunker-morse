//! What the bar shows, derived from the shared engine state: ten digit slots, the symbols of the digit in progress,
//! and the headline with its hint.

use crate::engine::Shared;

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
}

pub struct View {
    pub(super) slots: Vec<Slot>,
    /// Symbols heard of the digit in progress.
    pub(super) live_symbols: String,
    pub headline: String,
    pub hint: String,
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
            phase: Phase::Empty
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
        let n = done.min(DIGITS);
        return View {
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
            };
        }
        let ago = f.heard.elapsed().as_secs();
        let which = f
            .bunker
            .map_or("a grid outside the known bunkers".to_string(), |b| {
                format!("bunker {b}")
            });
        return View {
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
