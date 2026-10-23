use egui::{Pos2, Rect};
use nasdara_decoder::bunkers::MAP_SIZE;

/// Maps game metres (x east, z north) onto the screen rect of the plan.
pub struct Projection {
    inner: Rect,
}

impl Projection {
    pub fn new(inner: Rect) -> Self {
        Self { inner }
    }

    pub fn to_screen(&self, x: f64, z: f64) -> Pos2 {
        Pos2::new(
            self.inner.left() + (x / MAP_SIZE) as f32 * self.inner.width(),
            self.inner.top() + ((MAP_SIZE - z) / MAP_SIZE) as f32 * self.inner.height(),
        )
    }
}
