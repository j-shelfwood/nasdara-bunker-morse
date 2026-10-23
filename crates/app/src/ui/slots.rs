//! The ten digit slots of a bunker message: six grid digits and four lock code digits.

use egui::{Align2, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use nasdara_decoder::grid::SHIPPED_TIMING;
use nasdara_decoder::morse::pattern_of;

use super::UiState;
use super::widgets::{paint_caption, paint_symbols, symbols_width};
use crate::engine::Shared;
use crate::theme::{self, AMBER, DIM, FAINT, RED, STROKE, SURFACE_HI, TEAL, TEXT, alpha};

const DIGITS: usize = 10;
/// Length above which a tone counts as a dash (dots are ~0.09 s, dashes ~0.48 s).
pub const DASH_FROM_S: f64 = 0.28;
/// Extra wait after the next digit's expected start before the previous digit counts as complete.
const SETTLE_S: f64 = 0.9;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Empty,
    /// The digit being sent right now.
    Active,
    /// Read from the audio so far; a later pass may change it.
    Provisional,
    Final,
}

#[derive(Clone)]
struct Slot {
    ch: Option<char>,
    support: f32,
    phase: Phase,
}

pub struct View {
    slots: Vec<Slot>,
    /// Symbols heard of the digit in progress.
    live_symbols: String,
    pub headline: String,
    pub hint: String,
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
        // The last digit of a partial trace is fitted to beeps that may still be arriving (three dashes of a 0 look
        // like a 2 with its dots lost), so it counts only once the next digit's first beep is due and has not come.
        let settled = preview.map_or(0, |p| {
            let last_settled = p
                .digits
                .last()
                .is_some_and(|d| now > d.window_end + SHIPPED_TIMING.digit_gap + SETTLE_S);
            p.digits.len().min(DIGITS) - usize::from(!last_settled && !p.digits.is_empty())
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

pub fn show(ui: &mut Ui, st: &mut UiState, v: &View) {
    let time = ui.input(|i| i.time) as f32;
    let avail = ui.available_width();
    let gap = 8.0_f32;
    let group_gap = 16.0_f32;
    let block_gap = 44.0_f32;
    let fixed = gap * 7.0 + group_gap + block_gap;
    let w = ((avail - fixed) / 10.0).clamp(40.0, 66.0);
    let h = (w * 1.28).round();
    let total = w * 10.0 + fixed;
    let label_h = 20.0;

    let (area, _) = ui.allocate_exact_size(Vec2::new(avail, h + label_h), Sense::hover());
    let painter = ui.painter().with_clip_rect(area.expand(20.0));
    let left = area.center().x - total / 2.0;
    let top = area.top() + label_h;

    // x of each slot
    let mut xs = Vec::with_capacity(DIGITS);
    let mut x = left;
    for i in 0..DIGITS {
        xs.push(x);
        x += w;
        x += match i {
            2 | 4 => gap,
            3 => group_gap,
            5 => block_gap,
            _ => gap,
        };
    }
    let span = |a: usize, b: usize| (xs[a], xs[b] + w);
    for (label, (a, b)) in [("Map grid", span(0, 5)), ("Lock code", span(6, 9))] {
        let y = area.top() + 8.0;
        paint_caption(
            &painter,
            Pos2::new((a + b) / 2.0, y),
            Align2::CENTER_CENTER,
            label,
            DIM,
        );
        let half = label.len() as f32 * 4.2 + 10.0;
        let mid = (a + b) / 2.0;
        painter.line_segment(
            [Pos2::new(a, y), Pos2::new(mid - half, y)],
            Stroke::new(1.0, STROKE),
        );
        painter.line_segment(
            [Pos2::new(mid + half, y), Pos2::new(b, y)],
            Stroke::new(1.0, STROKE),
        );
    }

    for (i, slot) in v.slots.iter().enumerate() {
        let rect = Rect::from_min_size(Pos2::new(xs[i], top), Vec2::new(w, h));
        let pop = st.pop(i, slot.ch, slot.phase == Phase::Final, ui.input(|i| i.time));
        if pop.pop > 0.0 || pop.ring > 0.0 {
            ui.ctx().request_repaint();
        }
        paint_slot(&painter, rect, slot, &v.live_symbols, time, pop);
    }
}

fn paint_slot(painter: &egui::Painter, rect: Rect, slot: &Slot, live: &str, time: f32, anim: Pop) {
    let pop = anim.pop;
    let scale = 1.0 + 0.16 * pop;
    let phase = slot.phase;
    let (border, fill) = match phase {
        Phase::Empty => (STROKE, alpha(SURFACE_HI, 0.55)),
        Phase::Active => {
            let breathe = 0.5 + 0.5 * (time * 4.0).sin();
            (alpha(AMBER, 0.45 + 0.5 * breathe), SURFACE_HI)
        }
        Phase::Provisional => (alpha(AMBER, 0.28), SURFACE_HI),
        Phase::Final => (alpha(TEAL, 0.38), SURFACE_HI),
    };
    if phase == Phase::Active {
        let glow = 0.5 + 0.5 * (time * 4.0).sin();
        painter.rect_filled(rect.expand(5.0), 15.0, alpha(AMBER, 0.05 + 0.07 * glow));
    }
    if pop > 0.0 {
        painter.rect_filled(
            rect.expand(3.0 * pop),
            13.0,
            alpha(if phase == Phase::Final { TEAL } else { AMBER }, 0.16 * pop),
        );
    }
    painter.rect_filled(rect, 11.0, fill);
    painter.rect_stroke(rect, 11.0, Stroke::new(1.2, border), StrokeKind::Inside);
    if anim.ring > 0.0 {
        let grow = 16.0 * (1.0 - anim.ring);
        painter.rect_stroke(
            rect.expand(2.0 + grow),
            11.0 + grow,
            Stroke::new(2.5 * anim.ring + 0.5, alpha(TEAL, 0.9 * anim.ring)),
            StrokeKind::Outside,
        );
    }

    let digit_y = rect.top() + rect.height() * 0.43;
    match (slot.ch, phase) {
        (Some(ch), _) => {
            let unreadable = ch == '?';
            let color = match (phase, unreadable) {
                (_, true) => RED,
                (Phase::Final, _) => TEXT,
                _ => alpha(AMBER, 0.6),
            };
            painter.text(
                Pos2::new(rect.center().x, digit_y),
                Align2::CENTER_CENTER,
                ch.to_string(),
                theme::digits(rect.width() * 0.78 * scale),
                color,
            );
            if !unreadable {
                paint_symbols(
                    painter,
                    Pos2::new(rect.center().x, rect.bottom() - 19.0),
                    pattern_of(ch),
                    0.85,
                    alpha(DIM, 0.8),
                );
            }
            if phase == Phase::Final {
                // confidence: how much of the expected beep pattern was heard
                let bar = Rect::from_min_size(
                    Pos2::new(rect.left() + 8.0, rect.bottom() - 8.0),
                    Vec2::new(rect.width() - 16.0, 3.0),
                );
                painter.rect_filled(bar, 1.5, alpha(FAINT, 0.35));
                let share = slot.support.clamp(0.0, 1.0);
                let c = if share >= 0.7 {
                    TEAL
                } else if share >= 0.45 {
                    AMBER
                } else {
                    RED
                };
                painter.rect_filled(
                    Rect::from_min_size(bar.min, Vec2::new(bar.width() * share, 3.0)),
                    1.5,
                    c,
                );
            }
        }
        (None, Phase::Active) => {
            if live.is_empty() {
                let on = (time * 2.0).fract() < 0.5;
                if on {
                    painter.rect_filled(
                        Rect::from_center_size(
                            Pos2::new(rect.center().x, digit_y),
                            Vec2::new(2.0, rect.height() * 0.32),
                        ),
                        1.0,
                        AMBER,
                    );
                }
            } else {
                let scale = 1.5_f32.min((rect.width() - 16.0) / symbols_width(live));
                paint_symbols(
                    painter,
                    Pos2::new(rect.center().x, digit_y),
                    live,
                    scale,
                    AMBER,
                );
            }
        }
        (None, _) => {
            painter.text(
                Pos2::new(rect.center().x, digit_y),
                Align2::CENTER_CENTER,
                "·",
                theme::digits(rect.width() * 0.6),
                alpha(FAINT, 0.7),
            );
        }
    }
}

/// What the window last saw in a digit slot, for the one-shot animations.
#[derive(Clone, Copy, Default)]
pub struct SlotSeen {
    ch: Option<char>,
    is_final: bool,
    changed_at: f64,
    settled_at: f64,
}

/// Animation levels of one slot, each 1.0 at the moment it fires and easing to 0.
#[derive(Clone, Copy, Default)]
struct Pop {
    /// The digit changed.
    pop: f32,
    /// The digit went from tentative to final: an expanding ring.
    ring: f32,
}

const RING_S: f64 = 0.7;

impl UiState {
    fn pop(&mut self, i: usize, ch: Option<char>, is_final: bool, now: f64) -> Pop {
        let e = &mut self.slot_seen[i];
        if e.ch != ch {
            e.changed_at = now;
        }
        if ch.is_some() && is_final && !e.is_final {
            e.settled_at = now;
        }
        e.ch = ch;
        e.is_final = ch.is_some() && is_final;
        if ch.is_none() {
            return Pop::default();
        }
        let ease = |t: f64| {
            if t >= 1.0 {
                0.0
            } else {
                (1.0 - t as f32).powi(2)
            }
        };
        Pop {
            pop: ease((now - e.changed_at) / 0.33),
            ring: if e.is_final {
                ease((now - e.settled_at) / RING_S)
            } else {
                0.0
            },
        }
    }
}
