//! Division-2-style HUD primitives, painter only, no behaviour.

mod frame;
mod panel;

pub(crate) use frame::{chamfer_points, paint_focus, paint_frame};
// Reserved for the map lane's callouts.
#[allow(unused_imports)]
pub(crate) use frame::{paint_brackets, paint_leader};
pub(crate) use panel::{Panel, panel};
