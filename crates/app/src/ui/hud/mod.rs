//! Division-2-style HUD primitives, painter only, no behaviour.

mod frame;
mod panel;

pub(crate) use frame::{chamfer_points, paint_focus, paint_frame};
pub(crate) use frame::{paint_brackets, paint_leader};
pub(crate) use panel::{Panel, panel};
