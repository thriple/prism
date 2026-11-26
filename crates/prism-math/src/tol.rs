//! The tolerance: how close two things must be to count as the same.
//!
//! One fixed distance per document (DECISIONS #42, owner 2026-11-26),
//! the same in every group and stored in the file; not worked out from
//! the model's size. Default [`STANDARD`] (0.01 mm); the document setting
//! offers [`FINE`] and [`COARSE`] for extreme scales. Every operation asks
//! the model for it ([`Tol`] through `Model::tol()`); no other distance
//! epsilons are allowed in operations.

use glam::DVec3;
use serde::{Deserialize, Serialize};

/// The default tolerance (Standard): 0.01 mm.
pub const STANDARD: f64 = 1e-5;
/// Fine: 0.001 mm (small parts, jewellery).
pub const FINE: f64 = 1e-6;
/// Coarse: 0.1 mm (sites, terrain).
pub const COARSE: f64 = 1e-4;
/// A fraction of a size that counts as "the same point" there: used only
/// to pick a document's precision for an older file from its geometry
/// ([`Tol::for_extent`]), never for an edit.
pub const RELATIVE: f64 = 1e-6;
/// The tolerance never goes below or above these (model units).
pub const MIN: f64 = 1e-12;
pub const MAX: f64 = 1e-2;
/// A face thinner than this many tolerances on average, and far longer
/// than wide, is a sliver (Model Health): almost always a slip, such as a
/// straight edge drawn beside a curve between the same two points.
pub const SLIVER: f64 = 100.0;
/// A sliver is at least this many times longer than it is wide.
pub const SLIVER_RATIO: f64 = 100.0;
/// Two directions within this of each other (as 1 - |cos|) count as parallel
/// when no length is at hand ([`Tol::parallel`]): about 0.0026°. Where a
/// length is known, [`Tol::parallel_over`] is the rule (DECISIONS #42).
pub const ANGLE: f64 = 1e-9;
/// However short the length, two directions further apart than this (as
/// the sine of the angle, about 0.057°) are never parallel.
pub const PARALLEL_MAX: f64 = 1e-3;
/// However long the length, directions this close (as a sine) always are:
/// below the precision of a computed unit vector.
pub const PARALLEL_MIN: f64 = 1e-12;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tol {
    /// Distance in model units.
    pub dist: f64,
}

impl Default for Tol {
    /// The default document tolerance, [`STANDARD`].
    fn default() -> Self {
        Self { dist: STANDARD }
    }
}

impl Tol {
    /// A document tolerance of `dist`, kept within [`MIN`] and [`MAX`].
    pub fn new(dist: f64) -> Self {
        let d = if dist.is_finite() && dist > 0.0 {
            dist
        } else {
            STANDARD
        };
        Self {
            dist: d.clamp(MIN, MAX),
        }
    }

    /// The relative tolerance for something `extent` across: only for
    /// choosing a document's precision from geometry (an older file, an
    /// import), not for an edit.
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

    /// Whether two unit directions are parallel (same or opposite) over
    /// `len`: something `len` long along one ends less than the tolerance
    /// from where it would along the other. A fixed angle would let the far
    /// end of a long face drift well past the tolerance, and hold a short
    /// one tighter than the tolerance needs (DECISIONS #42).
    pub fn parallel_over(&self, a: DVec3, b: DVec3, len: f64) -> bool {
        let len = if len.is_finite() {
            len.max(self.dist)
        } else {
            self.dist
        };
        let allowed = (self.dist / len).clamp(PARALLEL_MIN, PARALLEL_MAX);
        a.cross(b).length() < allowed
    }

    /// Whether two unit directions are perpendicular over `len`: something
    /// `len` long along `b` rises less than the tolerance along `a`.
    pub fn perpendicular_over(&self, a: DVec3, b: DVec3, len: f64) -> bool {
        let len = if len.is_finite() {
            len.max(self.dist)
        } else {
            self.dist
        };
        a.dot(b).abs() < (self.dist / len).clamp(PARALLEL_MIN, PARALLEL_MAX)
    }

    /// Whether two unit directions are perpendicular.
    pub fn perpendicular(&self, a: DVec3, b: DVec3) -> bool {
        a.dot(b).abs() < ANGLE.sqrt()
    }

    /// A sliver is thinner than this on average (see [`SLIVER`]).
    pub fn sliver(&self) -> f64 {
        self.dist * SLIVER
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
    fn parallel_allows_the_tolerance_over_the_length() {
        let t = Tol::new(STANDARD);
        let tilt = |s: f64| DVec3::new(s, 0.0, (1.0 - s * s).sqrt());
        // 1.1e-6 over 10 m drifts past 0.01 mm; 0.9e-6 stays within it.
        assert!(!t.parallel_over(DVec3::Z, tilt(1.1e-6), 10.0));
        assert!(t.parallel_over(DVec3::Z, tilt(0.9e-6), 10.0));
        // Over 5 cm the same document allows 2e-4.
        assert!(t.parallel_over(DVec3::Z, tilt(1.9e-4), 0.05));
        assert!(!t.parallel_over(DVec3::Z, tilt(2.1e-4), 0.05));
        // Opposite directions are parallel too.
        assert!(t.parallel_over(DVec3::Z, -tilt(1e-7), 10.0));
        // Never further apart than PARALLEL_MAX, however short.
        assert!(!t.parallel_over(DVec3::Z, tilt(2e-3), 1e-9));
        assert!(t.parallel_over(DVec3::Z, tilt(0.5e-3), 0.0));
        // Square: a 10 m wall leaning 2e-6 rises 0.02 mm, past the tolerance.
        assert!(!t.perpendicular_over(DVec3::Z, DVec3::new(1.0, 0.0, 2e-6).normalize(), 10.0));
        assert!(t.perpendicular_over(DVec3::Z, DVec3::new(1.0, 0.0, 2e-6).normalize(), 1.0));
    }

    #[test]
    fn from_points() {
        let pts = [DVec3::ZERO, DVec3::new(6.0, 2.0, 0.0)];
        assert!((Tol::for_points(&pts).dist - 6e-6).abs() < 1e-18);
    }
}
