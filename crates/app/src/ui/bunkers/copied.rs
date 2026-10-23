//! The short "Copied" confirmation that replaces "Copy code" after a click.

use std::time::Instant;

use egui::{Context, Id};

use super::super::UiState;
use crate::ui::anim::egui_time_of;

/// How long "Copied" replaces "Copy code".
pub(crate) const COPIED_S: f64 = 1.6;

/// Remember a copy to the clipboard under `key`. The engine clock is converted to egui time on the next read.
pub(crate) fn mark_copied(st: &mut UiState, key: &str) {
    st.copied = Some((key.to_string(), Instant::now()));
}

/// Whether `key` was copied within `COPIED_S`; schedules the repaint that flips the label back.
pub(crate) fn copied_recently(ctx: &Context, st: &UiState, key: &str) -> bool {
    let Some((k, at)) = st.copied.as_ref().filter(|(k, _)| k == key) else {
        return false;
    };
    let since = ctx.input(|i| i.time) - egui_time_of(ctx, Id::unique(("copied", k)), *at);
    let left = COPIED_S - since;
    if left > 0.0 {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(left));
    }
    left > 0.0
}
