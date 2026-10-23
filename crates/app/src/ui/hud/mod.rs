//! Division-2-style HUD primitives, painter only, no behaviour.

// Foundations land before their first caller.
#![allow(dead_code, unused_imports)]

mod frame;

pub(crate) use frame::{chamfer_points, paint_brackets, paint_frame, paint_leader};
