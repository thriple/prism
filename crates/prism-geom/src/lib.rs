//! Curves and surfaces: lines, exact arcs (Exact Curves part 2) and planes.
//! The enums leave room for NURBS (PLAN 1.1).
//!
//! A curve does not hold its ends: an edge's two vertices are its ends, and
//! every method takes their positions `a` and `b`.

use std::f64::consts::TAU;

use prism_math::{DAffine3, DVec3, Plane};
use serde::{Deserialize, Serialize};

/// Segments per full turn when an arc is drawn or used as a polygon. Fine
/// enough for now; adaptive tessellation by screen error is part 4.
pub const SEGMENTS_PER_TURN: usize = 128;

/// The curve an edge follows between its two vertices.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Curve {
    /// A straight line.
    #[default]
    Line,
    /// An arc of the circle about `centre` with unit `axis` and `radius`,
    /// running counter-clockwise about the axis from `a` to `b`.
    Arc {
        centre: DVec3,
        axis: DVec3,
        radius: f64,
    },
}

impl Curve {
    pub fn is_arc(&self) -> bool {
        matches!(self, Curve::Arc { .. })
    }

    /// The same curve walked from `b` to `a`.
    pub fn reversed(&self) -> Curve {
        match *self {
            Curve::Line => Curve::Line,
            Curve::Arc {
                centre,
                axis,
                radius,
            } => Curve::Arc {
                centre,
                axis: -axis,
                radius,
            },
        }
    }

    /// The angle swept from `a` to `b` (0 for a line). In (0, 2π]: ends at
    /// the same angle make a full turn.
    pub fn sweep(&self, a: DVec3, b: DVec3) -> f64 {
        let Some((u, v, c)) = self.frame(a) else {
            return 0.0;
        };
        let d = b - c;
        let ang = d.dot(v).atan2(d.dot(u));
        if ang <= 1e-15 { ang + TAU } else { ang }
    }

    /// The in-plane frame of an arc at `a`: unit `u` towards `a`, `v` along
    /// the arc there, and the centre.
    fn frame(&self, a: DVec3) -> Option<(DVec3, DVec3, DVec3)> {
        let Curve::Arc { centre, axis, .. } = *self else {
            return None;
        };
        let r = a - centre;
        let u = (r - axis * r.dot(axis)).normalize_or_zero();
        Some((u, axis.cross(u), centre))
    }

    /// A point at fraction `t` (0 to 1) of the way from `a` to `b`.
    pub fn point(&self, a: DVec3, b: DVec3, t: f64) -> DVec3 {
        match *self {
            Curve::Line => a.lerp(b, t),
            Curve::Arc { radius, .. } => {
                let Some((u, v, c)) = self.frame(a) else {
                    return a;
                };
                let th = self.sweep(a, b) * t;
                c + (u * th.cos() + v * th.sin()) * radius
            }
        }
    }

    /// The unit direction of travel at `a`, walking towards `b`.
    pub fn start_tangent(&self, a: DVec3, b: DVec3) -> DVec3 {
        match self.frame(a) {
            Some((_, v, _)) => v,
            None => (b - a).normalize_or_zero(),
        }
    }

    /// The unit direction of travel arriving at `b` from `a`.
    pub fn end_tangent(&self, a: DVec3, b: DVec3) -> DVec3 {
        match self.frame(b) {
            Some((_, v, _)) => v,
            None => (b - a).normalize_or_zero(),
        }
    }

    /// The length from `a` to `b`.
    pub fn length(&self, a: DVec3, b: DVec3) -> f64 {
        match *self {
            Curve::Line => a.distance(b),
            Curve::Arc { radius, .. } => radius * self.sweep(a, b),
        }
    }

    /// The points strictly between `a` and `b`, at even steps (none for a
    /// line).
    pub fn inner_points(&self, a: DVec3, b: DVec3) -> Vec<DVec3> {
        if !self.is_arc() {
            return Vec::new();
        }
        let n = segments(self.sweep(a, b));
        (1..n)
            .map(|i| self.point(a, b, i as f64 / n as f64))
            .collect()
    }

    /// The area between the chord `a`–`b` and the curve, signed about
    /// `normal`: positive when the curve bulges to the right of the chord
    /// walked from `a` to `b` (out of a counter-clockwise loop).
    pub fn bulge_area(&self, a: DVec3, b: DVec3, normal: DVec3) -> f64 {
        let Curve::Arc { axis, radius, .. } = *self else {
            return 0.0;
        };
        let th = self.sweep(a, b);
        radius * radius * 0.5 * (th - th.sin()) * axis.dot(normal).signum()
    }

    /// The curve after `m`, if it is still this kind of curve. An arc stays
    /// an arc under moves, turns, mirrors and even scales; an uneven scale
    /// would make an ellipse, so it gives None.
    pub fn transformed(&self, m: &DAffine3) -> Option<Curve> {
        match *self {
            Curve::Line => Some(Curve::Line),
            Curve::Arc {
                centre,
                axis,
                radius,
            } => {
                let s = even_scale(m)?;
                let flip = m.matrix3.determinant().signum();
                Some(Curve::Arc {
                    centre: m.transform_point3(centre),
                    axis: (m.matrix3 * axis).normalize() * flip,
                    radius: radius * s,
                })
            }
        }
    }

    /// For an arc, its plane (through the centre, along the axis).
    pub fn plane(&self) -> Option<Plane> {
        match *self {
            Curve::Line => None,
            Curve::Arc { centre, axis, .. } => Some(Plane::new(centre, axis)),
        }
    }
}

/// How many even steps an arc of `sweep` radians is drawn with.
pub fn segments(sweep: f64) -> usize {
    ((sweep / TAU * SEGMENTS_PER_TURN as f64).ceil() as usize).clamp(2, SEGMENTS_PER_TURN)
}

/// The scale of `m` if it scales evenly in every direction (a rigid motion
/// times one factor), else None.
pub fn even_scale(m: &DAffine3) -> Option<f64> {
    let c = [m.matrix3.x_axis, m.matrix3.y_axis, m.matrix3.z_axis];
    let s = c[0].length();
    let close = |x: f64, y: f64| (x - y).abs() <= 1e-9 * s.max(1e-300);
    let even = close(c[1].length(), s)
        && close(c[2].length(), s)
        && close(c[0].dot(c[1]) / s, 0.0)
        && close(c[1].dot(c[2]) / s, 0.0)
        && close(c[0].dot(c[2]) / s, 0.0);
    (even && s > 0.0).then_some(s)
}

/// The surface a face lies on.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Surface {
    Plane(Plane),
}

impl Surface {
    pub fn plane(&self) -> &Plane {
        match self {
            Surface::Plane(p) => p,
        }
    }

    /// Which way the face points.
    pub fn normal(&self) -> DVec3 {
        self.plane().normal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_math::DQuat;
    use std::f64::consts::PI;

    fn close(a: DVec3, b: DVec3) -> bool {
        a.distance(b) < 1e-12
    }

    /// The upper half of the unit circle about the origin, from +x to -x.
    fn upper() -> (Curve, DVec3, DVec3) {
        (
            Curve::Arc {
                centre: DVec3::ZERO,
                axis: DVec3::Z,
                radius: 1.0,
            },
            DVec3::X,
            -DVec3::X,
        )
    }

    #[test]
    fn line_points() {
        let p = Curve::Line.point(DVec3::ZERO, DVec3::new(4.0, 0.0, 0.0), 0.25);
        assert_eq!(p, DVec3::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn arc_points_tangents_and_length() {
        let (c, a, b) = upper();
        assert!((c.sweep(a, b) - PI).abs() < 1e-12);
        assert!(close(c.point(a, b, 0.5), DVec3::Y));
        assert!(close(c.start_tangent(a, b), DVec3::Y));
        assert!(close(c.end_tangent(a, b), -DVec3::Y));
        assert!((c.length(a, b) - PI).abs() < 1e-12);
        // Walked the other way it is the lower half.
        assert!(close(c.reversed().point(a, b, 0.5), -DVec3::Y));
        // Ends at one place make a full turn.
        assert!((c.sweep(a, a) - TAU).abs() < 1e-12);
    }

    #[test]
    fn inner_points_lie_on_the_circle() {
        let (c, a, b) = upper();
        let pts = c.inner_points(a, b);
        assert_eq!(pts.len(), SEGMENTS_PER_TURN / 2 - 1);
        assert!(
            pts.iter()
                .all(|p| (p.length() - 1.0).abs() < 1e-12 && p.y > 0.0)
        );
        assert!(Curve::Line.inner_points(a, b).is_empty());
    }

    #[test]
    fn two_half_bulges_make_a_circle() {
        let (c, a, b) = upper();
        // Walked counter-clockwise about +z, both halves bulge out.
        let area = c.bulge_area(a, b, DVec3::Z) + c.bulge_area(b, a, DVec3::Z);
        assert!((area - PI).abs() < 1e-12);
        assert!((c.bulge_area(a, b, -DVec3::Z) + PI / 2.0).abs() < 1e-12);
    }

    #[test]
    fn arcs_stay_arcs_under_even_transforms() {
        let (c, a, b) = upper();
        let m = DAffine3::from_scale_rotation_translation(
            DVec3::splat(2.0),
            DQuat::from_rotation_x(0.7),
            DVec3::new(1.0, 2.0, 3.0),
        );
        let t = c.transformed(&m).expect("still an arc");
        let (ta, tb) = (m.transform_point3(a), m.transform_point3(b));
        assert!(close(
            t.point(ta, tb, 0.5),
            m.transform_point3(c.point(a, b, 0.5))
        ));
        // A mirror keeps the arc on the same side.
        let mirror = DAffine3::from_scale(DVec3::new(-1.0, 1.0, 1.0));
        let t = c.transformed(&mirror).expect("still an arc");
        let (ma, mb) = (mirror.transform_point3(a), mirror.transform_point3(b));
        assert!(close(t.point(ma, mb, 0.5), DVec3::Y));
        // An uneven scale would make an ellipse.
        assert!(
            c.transformed(&DAffine3::from_scale(DVec3::new(2.0, 1.0, 1.0)))
                .is_none()
        );
    }
}
