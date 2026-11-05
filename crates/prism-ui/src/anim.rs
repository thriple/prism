//! Spring-animated values kept between frames for each widget.
//!
//! The time comes from egui's input (`RawInput::time`, set by the app), never
//! from a clock read here. While anything is moving, a repaint is requested;
//! once everything has settled, nothing asks for more frames.

use egui::{Context, Id};

use crate::spring::{Spring, SpringValue};
use crate::theme;

/// Motion settings for the whole UI.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MotionSettings {
    /// Springs jump straight to their end.
    pub reduce_motion: bool,
    /// The aurora behind glass slowly moves (off by default; never with reduce motion).
    pub animate_aurora: bool,
}

/// Set the motion settings. Call every frame, or whenever they change.
pub fn set_motion(ctx: &Context, settings: MotionSettings) {
    ctx.data_mut(|d| d.insert_temp(Id::NULL.with("prism-ui-motion"), settings));
}

/// The current motion settings.
pub fn motion(ctx: &Context) -> MotionSettings {
    ctx.data(|d| d.get_temp(Id::NULL.with("prism-ui-motion")))
        .unwrap_or_default()
}

#[derive(Clone, Copy)]
struct State {
    spring: SpringValue,
    last_time: f64,
}

/// The value of the spring `id` this frame, moving towards `target`.
/// The first time an id is seen it starts at its target, with no motion.
pub fn spring(ctx: &Context, id: Id, target: f32, spring: Spring) -> f32 {
    spring_with_speed(ctx, id, target, spring).0
}

/// Like [`spring`], but the first time `id` is seen it starts at `start`
/// (a menu opening from nothing).
pub fn spring_from(ctx: &Context, id: Id, start: f32, target: f32, spring: Spring) -> f32 {
    step(ctx, id, f64::from(start), target, spring).0
}

/// Like [`spring`], and also returns the current speed (units per second).
pub fn spring_with_speed(ctx: &Context, id: Id, target: f32, spring: Spring) -> (f32, f32) {
    step(ctx, id, f64::from(target), target, spring)
}

fn step(ctx: &Context, id: Id, start: f64, target: f32, spring: Spring) -> (f32, f32) {
    let now = ctx.input(|i| i.time);
    let reduce = motion(ctx).reduce_motion;
    let target = f64::from(target);
    let state = ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_insert_with(id, || State {
            spring: SpringValue::at(start),
            last_time: now,
        });
        let dt = (now - s.last_time).clamp(0.0, theme::motion::MAX_STEP);
        s.last_time = now;
        if reduce {
            s.spring.snap(target);
        } else {
            s.spring.step(target, spring, dt);
        }
        *s
    });
    if !state.spring.is_settled(target) {
        ctx.request_repaint();
    }
    (state.spring.value as f32, state.spring.velocity as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &Context, time: f64, f: impl FnMut(&Context)) {
        let input = egui::RawInput {
            time: Some(time),
            ..Default::default()
        };
        ctx.run_pass(input, f).textures_delta.clear();
    }

    #[test]
    fn starts_at_target_then_moves_and_settles() {
        let ctx = Context::default();
        let id = Id::unique("t");
        let mut v = 0.0;
        frame(&ctx, 0.0, |c| v = spring(c, id, 0.0, crate::spring::SPRING));
        assert_eq!(v, 0.0);
        frame(&ctx, 0.016, |c| {
            v = spring(c, id, 1.0, crate::spring::SPRING)
        });
        assert!(v > 0.0 && v < 1.0, "moving: {v}");
        let mut t = 0.016;
        for _ in 0..200 {
            t += 0.016;
            frame(&ctx, t, |c| v = spring(c, id, 1.0, crate::spring::SPRING));
        }
        assert_eq!(v, 1.0);
    }

    #[test]
    fn reduce_motion_jumps() {
        let ctx = Context::default();
        let id = Id::unique("t");
        let mut v = 0.0;
        frame(&ctx, 0.0, |c| v = spring(c, id, 0.0, crate::spring::SPRING));
        frame(&ctx, 0.016, |c| {
            set_motion(
                c,
                MotionSettings {
                    reduce_motion: true,
                    animate_aurora: false,
                },
            );
            v = spring(c, id, 1.0, crate::spring::SPRING);
        });
        assert_eq!(v, 1.0);
    }
}
