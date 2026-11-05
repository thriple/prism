//! The tolerance: how close two things must be to count as the same.
//!
//! It is relative: a small fraction of the model's size (PLAN 1.9), so a
//! millimetre part and a kilometre site behave the same. Every operation
//! asks this value; no other distance epsilons are allowed in operations.

use glam::DVec3;
use serde::{Deserialize, Serialize};

/// The fraction of the model's extent that counts as "the same point".
pub const RELATIVE: f64 = 1e-6;
/// The tolerance never goes below or above these (model units).
pub const MIN: f64 = 1e-12;
pub const MAX: f64 = 1e-2;
/// Two directions within this of each other (as 1 - |cos|) count as parallel.
/// It is an angle, so it does not scale with the model.
pub const ANGLE: f64 = 1e-9;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tol {
    /// Distance in model units.
    pub dist: f64,
}

impl Tol {
    /// The tolerance for a model whose bounding box is `extent` across.
    pub fn for_extent(extent: f64) -> Self {
        let e = if extent.is_finite() && extent > 0.0 {
            extent
        } else {
            1.0
        };
        Self {
            dist: (e * RELATIVE).clamp(MIN, MAX),
        }
    }

    /// The tolerance for a set of points (the extent of their bounding box).
    pub fn for_points<'a>(points: impl IntoIterator<Item = &'a DVec3>) -> Self {
        let mut lo = DVec3::splat(f64::INFINITY);
        let mut hi = DVec3::splat(f64::NEG_INFINITY);
        let mut any = false;
        for p in points {
            lo = lo.min(*p);
            hi = hi.max(*p);
            any = true;
        }
        if !any {
            return Self::for_extent(1.0);
        }
        Self::for_extent((hi - lo).max_element())
    }

    pub fn same_point(&self, a: DVec3, b: DVec3) -> bool {
        a.distance(b) < self.dist
    }

    pub fn is_zero(&self, x: f64) -> bool {
        x.abs() < self.dist
    }

    /// Whether two unit directions are parallel (same or opposite).
    pub fn parallel(&self, a: DVec3, b: DVec3) -> bool {
        1.0 - a.dot(b).abs() < ANGLE
    }

    /// Whether two unit directions are perpendicular.
    pub fn perpendicular(&self, a: DVec3, b: DVec3) -> bool {
        a.dot(b).abs() < ANGLE.sqrt()
    }

    /// An area below this counts as zero.
    pub fn area(&self) -> f64 {
        self.dist * self.dist
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_to_the_model_and_clamped() {
        assert!((Tol::for_extent(4.0).dist - 4e-6).abs() < 1e-18);
        assert!((Tol::for_extent(4000.0).dist - 4e-3).abs() < 1e-15);
        assert_eq!(Tol::for_extent(1e12).dist, MAX);
        assert_eq!(Tol::for_extent(1e-12).dist, MIN);
        assert_eq!(
            Tol::for_extent(0.0).dist,
            1e-6,
            "an empty model acts as 1 unit"
        );
    }

    #[test]
    fn from_points() {
        let pts = [DVec3::ZERO, DVec3::new(6.0, 2.0, 0.0)];
        assert!((Tol::for_points(&pts).dist - 6e-6).abs() < 1e-18);
    }
}
