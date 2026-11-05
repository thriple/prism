//! Planes, best-fit normals, and a 2D frame on a plane.

use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

use crate::Tol;

/// Newell's normal of a polygon: its length is twice the area, and it points
/// the way the polygon winds counter-clockwise. Robust for any planar or
/// nearly planar polygon.
pub fn newell(points: &[DVec3]) -> DVec3 {
    let n = points.len();
    let mut v = DVec3::ZERO;
    for i in 0..n {
        let a = points[i];
        let b = points[(i + 1) % n];
        v.x += (a.y - b.y) * (a.z + b.z);
        v.y += (a.z - b.z) * (a.x + b.x);
        v.z += (a.x - b.x) * (a.y + b.y);
    }
    v
}

/// An oriented plane.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Plane {
    pub origin: DVec3,
    /// Unit length.
    pub normal: DVec3,
}

impl Plane {
    pub fn new(origin: DVec3, normal: DVec3) -> Self {
        Self {
            origin,
            normal: normal.normalize(),
        }
    }

    /// The best-fit plane of a polygon, through its centroid, oriented by its
    /// winding. None if the polygon has no area.
    pub fn fit(points: &[DVec3], tol: &Tol) -> Option<Plane> {
        let n = newell(points);
        if n.length() * 0.5 < tol.area() {
            return None;
        }
        let c = points.iter().copied().sum::<DVec3>() / points.len() as f64;
        Some(Plane::new(c, n))
    }

    /// Signed distance from the plane.
    pub fn distance(&self, p: DVec3) -> f64 {
        (p - self.origin).dot(self.normal)
    }

    pub fn contains(&self, p: DVec3, tol: &Tol) -> bool {
        tol.is_zero(self.distance(p))
    }

    pub fn project(&self, p: DVec3) -> DVec3 {
        p - self.normal * self.distance(p)
    }

    /// The same plane facing the other way.
    pub fn flipped(&self) -> Plane {
        Plane {
            origin: self.origin,
            normal: -self.normal,
        }
    }

    /// Whether two planes are the same surface (either facing).
    pub fn same_surface(&self, other: &Plane, tol: &Tol) -> bool {
        tol.parallel(self.normal, other.normal) && self.contains(other.origin, tol)
    }

    /// A deterministic key for the unoriented plane: the normal flipped so its
    /// largest component is positive, and the offset along it.
    pub fn canonical(&self) -> (DVec3, f64) {
        let n = self.normal;
        let a = n.abs();
        let s = if a.x >= a.y && a.x >= a.z {
            n.x.signum()
        } else if a.y >= a.z {
            n.y.signum()
        } else {
            n.z.signum()
        };
        let n = n * s;
        (n, n.dot(self.origin))
    }

    /// A 2D frame on the plane: `to_2d` keeps counter-clockwise (about the
    /// canonical normal) counter-clockwise.
    pub fn frame(&self) -> Frame {
        let (n, d) = self.canonical();
        let a = n.abs();
        let helper = if a.x <= a.y && a.x <= a.z {
            DVec3::X
        } else if a.y <= a.z {
            DVec3::Y
        } else {
            DVec3::Z
        };
        let u = helper.cross(n).normalize();
        let v = n.cross(u);
        Frame {
            origin: n * d,
            u,
            v,
            normal: n,
        }
    }
}

/// Orthonormal 2D coordinates on a plane.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub origin: DVec3,
    pub u: DVec3,
    pub v: DVec3,
    /// The canonical normal: `u` × `v`.
    pub normal: DVec3,
}

impl Frame {
    pub fn to_2d(&self, p: DVec3) -> DVec2 {
        let d = p - self.origin;
        DVec2::new(d.dot(self.u), d.dot(self.v))
    }

    pub fn to_3d(&self, p: DVec2) -> DVec3 {
        self.origin + self.u * p.x + self.v * p.y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newell_area_and_direction() {
        let sq = [
            DVec3::ZERO,
            DVec3::X * 4.0,
            DVec3::new(4.0, 4.0, 0.0),
            DVec3::Y * 4.0,
        ];
        let n = newell(&sq);
        assert_eq!(n, DVec3::new(0.0, 0.0, 32.0));
    }

    #[test]
    fn frames_round_trip_and_keep_handedness() {
        for normal in [DVec3::Z, -DVec3::Z, DVec3::X, DVec3::new(1.0, 2.0, 3.0)] {
            let p = Plane::new(DVec3::new(1.0, 2.0, 3.0), normal);
            let f = p.frame();
            let q = p.project(DVec3::new(5.0, -2.0, 7.0));
            assert!(f.to_3d(f.to_2d(q)).distance(q) < 1e-12);
            assert!((f.u.cross(f.v) - f.normal).length() < 1e-12);
            assert!(f.normal.dot(normal).abs() > 0.999);
        }
    }

    #[test]
    fn canonical_ignores_facing() {
        let a = Plane::new(DVec3::new(0.0, 0.0, 2.0), DVec3::Z);
        assert_eq!(a.canonical(), a.flipped().canonical());
        let tol = Tol::for_extent(4.0);
        assert!(a.same_surface(&a.flipped(), &tol));
    }
}
