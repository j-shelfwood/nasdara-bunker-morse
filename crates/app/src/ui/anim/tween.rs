use egui::{Context, Id};

/// Animation entry points. With `reduce` set (from `Settings.reduce_motion`) nothing moves and nothing asks for a
/// repaint. Time is `InputState::time`, so headless renders are deterministic.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Motion {
    pub reduce: bool,
}

#[derive(Clone, Copy)]
struct Tween {
    from: f32,
    to: f32,
    start: f64,
    /// Value shown last frame, the origin when the target moves mid-flight.
    shown: f32,
}

impl Motion {
    pub fn new(reduce: bool) -> Self {
        Self { reduce }
    }

    /// Eased value moving toward `target` over `secs`. A new target restarts from the value currently shown.
    pub fn toward(
        &self,
        ctx: &Context,
        id: Id,
        target: f32,
        secs: f32,
        ease: fn(f32) -> f32,
    ) -> f32 {
        let now = ctx.input(|i| i.time);
        if self.reduce || secs <= 0.0 {
            ctx.data_mut(|d| {
                d.insert_temp(
                    id,
                    Tween {
                        from: target,
                        to: target,
                        start: now,
                        shown: target,
                    },
                )
            });
            return target;
        }
        let mut t = ctx.data_mut(|d| d.get_temp::<Tween>(id)).unwrap_or(Tween {
            from: target,
            to: target,
            start: now,
            shown: target,
        });
        if t.to != target {
            t = Tween {
                from: t.shown,
                to: target,
                start: now,
                shown: t.shown,
            };
        }
        let p = (((now - t.start) as f32) / secs).clamp(0.0, 1.0);
        let value = if p >= 1.0 {
            target
        } else {
            t.from + (t.to - t.from) * ease(p)
        };
        t.shown = value;
        ctx.data_mut(|d| d.insert_temp(id, t));
        if p < 1.0 {
            ctx.request_repaint();
        }
        value
    }

    /// One-shot 1 -> 0 from `started_at` (egui time). 0 when idle or reduced.
    pub fn pulse(
        &self,
        ctx: &Context,
        _id: Id,
        started_at: Option<f64>,
        secs: f32,
        ease: fn(f32) -> f32,
    ) -> f32 {
        let Some(start) = started_at else { return 0.0 };
        if self.reduce || secs <= 0.0 {
            return 0.0;
        }
        let p = ((ctx.input(|i| i.time) - start) as f32 / secs).clamp(0.0, 1.0);
        if p < 1.0 {
            ctx.request_repaint();
        }
        1.0 - ease(p)
    }

    /// Sine in 0..1 at `hz`. Steady 0.5 when reduced.
    pub fn breathe(&self, ctx: &Context, hz: f32) -> f32 {
        if self.reduce {
            return 0.5;
        }
        ctx.request_repaint();
        let t = ctx.input(|i| i.time);
        0.5 + 0.5 * (t * f64::from(hz) * std::f64::consts::TAU).sin() as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, RawInput};

    fn frame(ctx: &Context, time: f64, f: impl FnOnce(&Context)) -> egui::FullOutput {
        ctx.begin_pass(RawInput {
            time: Some(time),
            events: Vec::<Event>::new(),
            ..Default::default()
        });
        f(ctx);
        ctx.end_pass()
    }

    fn id() -> Id {
        Id::unique("tween")
    }

    fn sample(ctx: &Context, m: Motion, time: f64, target: f32) -> f32 {
        let mut v = 0.0;
        frame(ctx, time, |c| {
            v = m.toward(c, id(), target, 1.0, egui::emath::easing::cubic_out)
        });
        v
    }

    #[test]
    fn toward_is_monotone_with_endpoints() {
        let ctx = Context::default();
        let m = Motion::new(false);
        assert_eq!(sample(&ctx, m, 0.0, 0.0), 0.0);
        let mut last = sample(&ctx, m, 0.1, 10.0);
        assert!(last < 10.0);
        for k in 2..=12 {
            let v = sample(&ctx, m, 0.1 * k as f64, 10.0);
            assert!(v >= last, "{v} < {last}");
            last = v;
        }
        assert_eq!(last, 10.0);
    }

    #[test]
    fn toward_snaps_under_reduce_without_repaint() {
        let delay = |m: Motion| {
            let ctx = Context::default();
            for k in 0..4 {
                frame(&ctx, 0.01 * k as f64, |c| {
                    m.toward(c, id(), 0.0, 1.0, egui::emath::easing::cubic_out);
                });
            }
            let mut v = 0.0;
            let out = frame(&ctx, 0.1, |c| {
                v = m.toward(c, id(), 10.0, 1.0, egui::emath::easing::cubic_out)
            });
            let d = out.viewport_output.values().map(|o| o.repaint_delay).min();
            (v, d.unwrap())
        };
        let (v, quiet) = delay(Motion::new(true));
        assert_eq!(v, 10.0);
        let (v2, busy) = delay(Motion::new(false));
        assert!(v2 < 10.0);
        assert!(busy < quiet, "busy {busy:?} quiet {quiet:?}");
        assert_eq!(quiet, std::time::Duration::MAX);
    }

    #[test]
    fn pulse_decays_to_zero_and_is_off_under_reduce() {
        let ctx = Context::default();
        let m = Motion::new(false);
        let at = |t: f64| {
            let mut v = 0.0;
            frame(&ctx, t, |c| {
                v = m.pulse(c, id(), Some(1.0), 0.5, egui::emath::easing::quadratic_out)
            });
            v
        };
        assert_eq!(at(1.0), 1.0);
        assert!(at(1.25) < 1.0 && at(1.25) > 0.0);
        assert_eq!(at(1.5), 0.0);
        assert_eq!(at(9.0), 0.0);
        let r = Motion::new(true);
        frame(&ctx, 1.0, |c| {
            assert_eq!(
                r.pulse(c, id(), Some(1.0), 0.5, egui::emath::easing::quadratic_out),
                0.0
            )
        });
        frame(&ctx, 1.0, |c| {
            assert_eq!(
                m.pulse(c, id(), None, 0.5, egui::emath::easing::quadratic_out),
                0.0
            )
        });
    }

    #[test]
    fn retarget_continues_from_current_value() {
        let ctx = Context::default();
        let m = Motion::new(false);
        sample(&ctx, m, 0.0, 0.0);
        sample(&ctx, m, 0.01, 10.0);
        let mid = sample(&ctx, m, 0.5, 10.0);
        assert!(mid > 0.0 && mid < 10.0);
        let after = sample(&ctx, m, 0.5, 0.0);
        assert!((after - mid).abs() < 1e-4, "jumped {mid} -> {after}");
        assert!(sample(&ctx, m, 0.6, 0.0) < mid);
    }

    #[test]
    fn breathe_is_steady_under_reduce_and_bounded() {
        let ctx = Context::default();
        frame(&ctx, 0.3, |c| {
            assert_eq!(Motion::new(true).breathe(c, 1.0), 0.5);
            let v = Motion::new(false).breathe(c, 1.0);
            assert!((0.0..=1.0).contains(&v));
        });
    }
}
