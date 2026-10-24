//! Persisted settings and the per-window UI state, plus the state-driven window and attention reactions.

use std::collections::HashMap;
use std::time::Instant;

use egui::{UserAttentionType, Vec2, ViewportCommand, WindowLevel};
use serde::{Deserialize, Serialize};

use crate::capture::Source;
use crate::engine::Shared;

/// Smallest window the layout is built for, in points.
pub const MIN_WINDOW: Vec2 = Vec2::new(900.0, 700.0);

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub source: Source,
    pub onboarded: bool,
    pub always_on_top: bool,
    pub flash_taskbar: bool,
    pub open_map_for_new: bool,
    /// No pulses, glows, blinking cursor or pop animations, and no fast repaint for them.
    pub reduce_motion: bool,
    /// The Morse cheat sheet under the bunkers.
    pub show_guide: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            source: Source::Game,
            onboarded: false,
            always_on_top: false,
            flash_taskbar: true,
            open_map_for_new: false,
            reduce_motion: false,
            show_guide: true,
        }
    }
}

/// What the window last saw in a digit slot, for the one-shot animations (times in egui time).
#[derive(Clone, Copy, Default)]
pub(super) struct SlotSeen {
    ch: Option<char>,
    is_final: bool,
    pub changed_at: Option<f64>,
    pub settled_at: Option<f64>,
}

impl SlotSeen {
    /// Records this frame's content; a changed digit restarts the pop, a tentative digit turning final the ring.
    pub fn observe(&mut self, ch: Option<char>, is_final: bool, now: f64) {
        let is_final = ch.is_some() && is_final;
        if ch.is_none() {
            *self = SlotSeen::default();
            return;
        }
        if self.ch != ch {
            self.changed_at = Some(now);
        }
        if is_final && !self.is_final {
            self.settled_at = Some(now);
        }
        self.ch = ch;
        self.is_final = is_final;
        if !is_final {
            self.settled_at = None;
        }
    }
}

pub struct UiState {
    pub settings: Settings,
    pub onboarding: Option<usize>,
    pub(super) slot_seen: [SlotSeen; 10],
    /// Grid and lock code last seen per card key, so only a new or changed card raises attention.
    card_seen: HashMap<String, (String, String)>,
    pub(super) copied: Option<(String, Instant)>,
    pub(super) selected: Option<String>,
    /// Map links a menu asked for during the frame, opened once the engine lock is released.
    pub(super) open_later: Vec<String>,
    /// The cheat sheet digit being played and when it started, in egui time.
    pub(super) guide_play: Option<(char, f64)>,
    on_top_sent: Option<bool>,
    min_size_sent: bool,
    window_fitted: bool,
    pub(super) modal_ids: (egui::Id, egui::Id),
}

impl UiState {
    pub fn new(settings: Settings) -> UiState {
        let onboarding = (!settings.onboarded).then_some(0);
        UiState {
            settings,
            onboarding,
            slot_seen: [SlotSeen::default(); 10],
            card_seen: HashMap::new(),
            copied: None,
            selected: None,
            open_later: Vec::new(),
            guide_play: None,
            on_top_sent: None,
            min_size_sent: false,
            window_fitted: false,
            modal_ids: (
                egui::Id::unique("onboarding-backdrop"),
                egui::Id::unique("onboarding"),
            ),
        }
    }

    pub(super) fn finish_onboarding(&mut self) {
        self.onboarding = None;
        self.settings.onboarded = true;
    }

    /// Sends the always-on-top command when the setting changed since the last frame.
    pub(super) fn sync_window_level(&mut self, ctx: &egui::Context) {
        let on_top = self.settings.always_on_top;
        if self.on_top_sent != Some(on_top) {
            self.on_top_sent = Some(on_top);
            ctx.send_viewport_cmd(ViewportCommand::WindowLevel(if on_top {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            }));
        }
    }

    /// The window opens at 1060x800 points whatever the monitor. On the first frame that knows the monitor size it is
    /// cut down to 90% of the monitor, and the minimum size drops so a short window still shows the result.
    pub(super) fn size_window(&mut self, ctx: &egui::Context) {
        const OPEN: Vec2 = Vec2::new(1060.0, 800.0);
        if !self.min_size_sent {
            self.min_size_sent = true;
            ctx.send_viewport_cmd(ViewportCommand::MinInnerSize(MIN_WINDOW));
        }
        if self.window_fitted {
            return;
        }
        let Some(monitor) = ctx.input(|i| i.viewport().monitor_size) else {
            return;
        };
        self.window_fitted = true;
        let current = ctx
            .input(|i| i.viewport().inner_rect)
            .map_or(OPEN, |r| r.size());
        let limit = (monitor * 0.9).max(MIN_WINDOW);
        if current.x > limit.x || current.y > limit.y {
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(current.min(limit)));
        }
    }

    /// Flashes the taskbar for a new or changed card and returns the map links to open once the lock is released.
    pub(super) fn react_to_new_bunkers(
        &mut self,
        ctx: &egui::Context,
        s: &mut Shared,
    ) -> Vec<String> {
        let mut to_open = Vec::new();
        self.card_seen
            .retain(|key, _| s.cards.iter().any(|c| &c.key == key));
        for key in std::mem::take(&mut s.changed) {
            let Some(card) = s.cards.iter().find(|c| c.key == key) else {
                continue;
            };
            let now = (card.grid.clone(), card.code.clone());
            let previous = self.card_seen.insert(key, now.clone());
            let is_new = previous.is_none();
            if !is_new && previous.as_ref() == Some(&now) {
                continue;
            }
            if self.settings.flash_taskbar {
                ctx.send_viewport_cmd(ViewportCommand::RequestUserAttention(if is_new {
                    UserAttentionType::Critical
                } else {
                    UserAttentionType::Informational
                }));
            }
            if let (true, true, Some(url)) = (
                self.settings.open_map_for_new,
                card.count == 1,
                card.url.as_deref(),
            ) && nasdara_decoder::bunkers::is_izurvive_url(url)
            {
                to_open.push(url.to_string());
            }
        }
        to_open
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observe_restarts_pop_on_change_and_ring_on_settle() {
        let mut s = SlotSeen::default();
        s.observe(Some('3'), false, 1.0);
        assert_eq!((s.changed_at, s.settled_at), (Some(1.0), None));
        s.observe(Some('3'), false, 1.5);
        assert_eq!(s.changed_at, Some(1.0));
        s.observe(Some('3'), true, 2.0);
        assert_eq!((s.changed_at, s.settled_at), (Some(1.0), Some(2.0)));
        s.observe(Some('3'), true, 2.5);
        assert_eq!(s.settled_at, Some(2.0));
        s.observe(Some('4'), true, 3.0);
        assert_eq!(s.changed_at, Some(3.0));
        s.observe(None, false, 4.0);
        assert_eq!((s.changed_at, s.settled_at), (None, None));
    }
}
