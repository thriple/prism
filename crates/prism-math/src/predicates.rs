//! Exact geometric predicates (Shewchuk's adaptive arithmetic, via `robust`).
//! Orientation and side tests never give a wrong sign because of rounding.

use glam::{DVec2, DVec3};

fn c(p: DVec2) -> robust::Coord<f64> {
    robust::Coord { x: p.x, y: p.y }
}

/// Positive if `a`, `b`, `c` turn left (counter-clockwise), negative if
/// right, zero if exactly in line.
pub fn orient2d(a: DVec2, b: DVec2, p: DVec2) -> f64 {
    robust::orient2d(c(a), c(b), c(p))
}

/// Which side of the plane through `a`, `b`, `c` the point `d` is on (exact).
pub fn orient3d(a: DVec3, b: DVec3, cc: DVec3, d: DVec3) -> f64 {
    let k = |p: DVec3| robust::Coord3D {
        x: p.x,
        y: p.y,
        z: p.z,
    };
    robust::orient3d(k(a), k(b), k(cc), k(d))
}

/// Where a point is relative to a polygon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inside {
    Inside,
    Outside,
    /// Exactly on the boundary.
    Boundary,
}

/// Whether `p` is inside a simple polygon (either winding), exactly.
pub fn point_in_polygon(p: DVec2, poly: &[DVec2]) -> Inside {
    let n = poly.len();
    if n < 3 {
        return Inside::Outside;
    }
    let mut winding = 0i32;
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let o = orient2d(a, b, p);
        if o == 0.0
            && p.x >= a.x.min(b.x)
            && p.x <= a.x.max(b.x)
            && p.y >= a.y.min(b.y)
            && p.y <= a.y.max(b.y)
        {
            return Inside::Boundary;
        }
        if a.y <= p.y {
            if b.y > p.y && o > 0.0 {
                winding += 1;
            }
        } else if b.y <= p.y && o < 0.0 {
            winding -= 1;
        }
    }
    if winding != 0 {
        Inside::Inside
    } else {
        Inside::Outside
    }
}

/// Twice the signed area of a polygon (positive if counter-clockwise).
pub fn signed_area2(poly: &[DVec2]) -> f64 {
    let n = poly.len();
    (0..n).map(|i| poly[i].perp_dot(poly[(i + 1) % n])).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Vec<DVec2> {
        vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(4.0, 0.0),
            DVec2::new(4.0, 4.0),
            DVec2::new(0.0, 4.0),
        ]
    }

    #[test]
    fn orientation_is_exact_for_nearly_collinear_points() {
        // A classic case where naive arithmetic gets the sign wrong.
        let a = DVec2::new(0.5, 0.5);
        let b = DVec2::new(12.0, 12.0);
        let p = DVec2::new(24.0, 24.0);
        assert_eq!(orient2d(a, b, p), 0.0);
        let q = DVec2::new(24.0, 24.000000000000004);
        assert!(orient2d(a, b, q) > 0.0);
    }

    #[test]
    fn inside_outside_boundary() {
        let sq = square();
        assert_eq!(point_in_polygon(DVec2::new(2.0, 2.0), &sq), Inside::Inside);
        assert_eq!(point_in_polygon(DVec2::new(5.0, 2.0), &sq), Inside::Outside);
        assert_eq!(
            point_in_polygon(DVec2::new(4.0, 2.0), &sq),
            Inside::Boundary
        );
        let mut cw = sq.clone();
        cw.reverse();
        assert_eq!(point_in_polygon(DVec2::new(2.0, 2.0), &cw), Inside::Inside);
    }

    #[test]
    fn area_sign() {
        assert_eq!(signed_area2(&square()), 32.0);
        let mut cw = square();
        cw.reverse();
        assert_eq!(signed_area2(&cw), -32.0);
    }

    #[test]
    fn orient3d_sign() {
        let (a, b, c) = (DVec3::ZERO, DVec3::X, DVec3::Y);
        assert!(orient3d(a, b, c, DVec3::Z) != 0.0);
        assert_eq!(orient3d(a, b, c, DVec3::new(3.0, 5.0, 0.0)), 0.0);
    }
}
