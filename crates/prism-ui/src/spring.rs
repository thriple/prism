//! Damped springs, as the kit's motion language uses them.
//!
//! The caller passes the time step, so this never reads a clock and the same
//! steps always give the same result. The step is solved exactly (not
//! integrated), so any frame time is stable and two half steps equal one
//! whole step.

/// A spring's stiffness and damping, with unit mass (the kit's numbers).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    pub stiffness: f64,
    pub damping: f64,
}

/// Containers resizing.
pub const SPRING: Spring = Spring::new(420.0, 32.0);
/// Menus, tooltips, pills.
pub const SPRING_SNAPPY: Spring = Spring::new(520.0, 34.0);
/// Toasts and snackbars.
pub const SPRING_SOFT: Spring = Spring::new(400.0, 30.0);
/// A button sinking under the pointer: quick and firm.
pub const PRESS: Spring = Spring::new(700.0, 40.0);
/// A button coming back up: bouncy, overshoots past rest.
pub const RELEASE: Spring = Spring::new(380.0, 13.0);
/// A button label riding the press.
pub const LABEL: Spring = Spring::new(500.0, 18.0);
/// A slider handle following the pointer.
pub const HANDLE: Spring = Spring::new(500.0, 38.0);
/// A slider handle lifting while held.
pub const LIFT: Spring = Spring::new(400.0, 20.0);
/// A toggle knob travelling.
pub const KNOB: Spring = Spring::new(520.0, 30.0);
/// Small accent dots popping in.
pub const DOT: Spring = Spring::new(500.0, 20.0);
/// A dropdown trigger sinking as its list opens, and springing back.
pub const OPEN: Spring = Spring::new(420.0, 34.0);
pub const CLOSE: Spring = Spring::new(420.0, 15.0);
/// Frost clearing or fogging over (slow and smooth, no overshoot).
pub const FROST: Spring = Spring::new(60.0, 16.0);

impl Spring {
    pub const fn new(stiffness: f64, damping: f64) -> Self {
        Self { stiffness, damping }
    }

    /// The damping ratio: below 1 it overshoots, 1 or more it does not.
    pub fn damping_ratio(&self) -> f64 {
        self.damping / (2.0 * self.stiffness.sqrt())
    }
}

/// A value moving on a spring towards a target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringValue {
    pub value: f64,
    pub velocity: f64,
}

/// Closer than this, and slower than this, counts as arrived.
const SETTLE_DISTANCE: f64 = 1e-3;
const SETTLE_SPEED: f64 = 1e-2;

impl SpringValue {
    /// At rest at `value`.
    pub fn at(value: f64) -> Self {
        Self {
            value,
            velocity: 0.0,
        }
    }

    /// Whether it has arrived at `target` and stopped.
    pub fn is_settled(&self, target: f64) -> bool {
        (self.value - target).abs() < SETTLE_DISTANCE && self.velocity.abs() < SETTLE_SPEED
    }

    /// Jump straight to `target` (used when motion is reduced).
    pub fn snap(&mut self, target: f64) {
        *self = Self::at(target);
    }

    /// Advance by `dt` seconds towards `target`.
    pub fn step(&mut self, target: f64, spring: Spring, dt: f64) {
        if dt <= 0.0 {
            return;
        }
        let x0 = self.value - target;
        let v0 = self.velocity;
        let w0 = spring.stiffness.sqrt();
        let zeta = spring.damping_ratio();

        let (x, v) = if (zeta - 1.0).abs() < 1e-9 {
            // Critically damped.
            let b = v0 + w0 * x0;
            let e = (-w0 * dt).exp();
            ((x0 + b * dt) * e, (v0 - w0 * b * dt) * e)
        } else if zeta < 1.0 {
            // Underdamped: oscillates while it settles.
            let wd = w0 * (1.0 - zeta * zeta).sqrt();
            let e = (-zeta * w0 * dt).exp();
            let (s, c) = (wd * dt).sin_cos();
            let x = e * (x0 * c + (v0 + zeta * w0 * x0) / wd * s);
            let v = e * (v0 * c - (zeta * w0 * v0 + w0 * w0 * x0) / wd * s);
            (x, v)
        } else {
            // Overdamped: creeps in without overshoot.
            let root = (zeta * zeta - 1.0).sqrt();
            let r1 = -w0 * (zeta - root);
            let r2 = -w0 * (zeta + root);
            let c2 = (v0 - r1 * x0) / (r2 - r1);
            let c1 = x0 - c2;
            let (e1, e2) = ((r1 * dt).exp(), (r2 * dt).exp());
            (c1 * e1 + c2 * e2, r1 * c1 * e1 + r2 * c2 * e2)
        };

        self.value = target + x;
        self.velocity = v;
        if self.is_settled(target) {
            self.snap(target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Spring; 14] = [
        SPRING,
        SPRING_SNAPPY,
        SPRING_SOFT,
        PRESS,
        RELEASE,
        LABEL,
        HANDLE,
        LIFT,
        KNOB,
        DOT,
        OPEN,
        CLOSE,
        FROST,
        Spring::new(100.0, 20.0), // critically damped
    ];

    /// Run at 60 fps until settled; return the time and the furthest point reached.
    fn run(spring: Spring, from: f64, to: f64) -> (f64, f64) {
        let mut s = SpringValue::at(from);
        let mut t = 0.0;
        let mut peak = from;
        while !s.is_settled(to) {
            s.step(to, spring, 1.0 / 60.0);
            t += 1.0 / 60.0;
            if (s.value - from).abs() > (peak - from).abs() {
                peak = s.value;
            }
            assert!(t < 10.0, "{spring:?} never settled");
        }
        (t, peak)
    }

    #[test]
    fn every_spring_settles_on_its_target() {
        for spring in ALL {
            let (t, _) = run(spring, 0.0, 1.0);
            assert!(t < 3.0, "{spring:?} took {t} s");
        }
    }

    #[test]
    fn release_overshoots_past_rest_and_press_barely_does() {
        // A button coming back up from pressed (-0.7) to rest (1).
        let (_, peak) = run(RELEASE, -0.7, 1.0);
        assert!(peak > 1.2, "release peak {peak}");
        let (_, peak) = run(PRESS, 1.0, -0.7);
        assert!(peak > -0.8, "press peak {peak}");
        let (_, peak) = run(FROST, 0.0, 1.0);
        assert!(peak <= 1.0 + 1e-9, "frost must not overshoot: {peak}");
    }

    #[test]
    fn two_half_steps_equal_one_step() {
        for spring in ALL {
            let mut a = SpringValue {
                value: 0.3,
                velocity: -2.0,
            };
            let mut b = a;
            a.step(1.0, spring, 0.05);
            b.step(1.0, spring, 0.025);
            b.step(1.0, spring, 0.025);
            assert!((a.value - b.value).abs() < 1e-9, "{spring:?}");
            assert!((a.velocity - b.velocity).abs() < 1e-9, "{spring:?}");
        }
    }

    #[test]
    fn a_long_step_is_stable() {
        for spring in ALL {
            let mut s = SpringValue::at(0.0);
            s.step(1.0, spring, 5.0);
            assert!((s.value - 1.0).abs() < 1e-3, "{spring:?}: {}", s.value);
        }
    }

    #[test]
    fn no_time_means_no_change_and_snap_is_instant() {
        let mut s = SpringValue::at(0.0);
        s.step(1.0, SPRING, 0.0);
        assert_eq!(s, SpringValue::at(0.0));
        s.snap(1.0);
        assert!(s.is_settled(1.0));
    }
}
