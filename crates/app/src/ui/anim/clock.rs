//! Engine timestamps are `Instant`s; the motion layer runs on egui time. These helpers convert once, the first
//! time a value is seen, and keep the result in egui's memory so the paint path never calls `Instant::elapsed`.

use std::time::Instant;

use egui::{Context, Id};

/// Egui time at which the engine moment `at` happened. Stable across frames for the same `at`.
pub(crate) fn egui_time_of(ctx: &Context, id: Id, at: Instant) -> f64 {
    let known = ctx.data_mut(|d| d.get_temp::<(Instant, f64)>(id));
    if let Some((seen, t)) = known
        && seen == at
    {
        return t;
    }
    let t = ctx.input(|i| i.time) - at.elapsed().as_secs_f64();
    ctx.data_mut(|d| d.insert_temp(id, (at, t)));
    t
}

/// Egui time at which `value` last changed from what an earlier frame saw. `None` for the very first sight
/// (a card that just appeared is announced by its own appearance, not by a change).
pub(crate) fn changed_at<T: PartialEq + Clone + Send + Sync + 'static>(
    ctx: &Context,
    id: Id,
    value: &T,
) -> Option<f64> {
    let now = ctx.input(|i| i.time);
    let prev = ctx.data_mut(|d| d.get_temp::<(T, Option<f64>)>(id));
    let at = match prev {
        Some((old, at)) if &old == value => at,
        Some(_) => Some(now),
        None => None,
    };
    ctx.data_mut(|d| d.insert_temp(id, (value.clone(), at)));
    at
}

/// Egui time at which `id` was first drawn.
pub(crate) fn first_seen(ctx: &Context, id: Id) -> f64 {
    let now = ctx.input(|i| i.time);
    ctx.data_mut(|d| *d.get_temp_mut_or_insert_with::<f64>(id, || now))
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::RawInput;

    fn at(ctx: &Context, time: f64, f: impl FnOnce(&Context)) {
        ctx.begin_pass(RawInput {
            time: Some(time),
            ..Default::default()
        });
        f(ctx);
        let _ = ctx.end_pass();
    }

    #[test]
    fn a_change_is_stamped_once_and_the_first_sight_is_not_a_change() {
        let ctx = Context::default();
        let id = Id::unique("count");
        let mut seen = vec![];
        for (time, v) in [(1.0, 2), (2.0, 2), (3.0, 3), (4.0, 3)] {
            at(&ctx, time, |c| seen.push(changed_at(c, id, &v)));
        }
        assert_eq!(seen, [None, None, Some(3.0), Some(3.0)]);
    }

    #[test]
    fn an_engine_instant_converts_once_and_stays_put() {
        let ctx = Context::default();
        let id = Id::unique("heard");
        let heard = Instant::now() - std::time::Duration::from_secs(5);
        let (mut a, mut b) = (0.0, 0.0);
        at(&ctx, 100.0, |c| a = egui_time_of(c, id, heard));
        at(&ctx, 107.0, |c| b = egui_time_of(c, id, heard));
        assert!((a - 95.0).abs() < 0.1, "{a}");
        assert_eq!(a, b);
    }

    #[test]
    fn first_seen_keeps_the_first_time() {
        let ctx = Context::default();
        let id = Id::unique("card");
        let (mut a, mut b) = (0.0, 0.0);
        at(&ctx, 5.0, |c| a = first_seen(c, id));
        at(&ctx, 9.0, |c| b = first_seen(c, id));
        assert_eq!((a, b), (5.0, 5.0));
    }
}
