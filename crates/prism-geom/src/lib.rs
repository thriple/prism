//! Curves and surfaces: lines, exact arcs (Exact Curves part 2), planes,
//! cylinders (part 4), and cones, spheres and tori (part 5b).
//! The enums leave room for NURBS (PLAN 1.1).
//!
//! A curve does not hold its ends: an edge's two vertices are its ends, and
//! every method takes their positions `a` and `b`.

use std::f64::consts::TAU;

use prism_math::{DAffine3, DVec3, Plane};
use serde::{Deserialize, Serialize};

mod revolved;
pub use revolved::{Cone, Profile, Revolution, Sphere, Torus, axis_frame};

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

    /// The point of the curve from `a` to `b` closest to `p`.
    pub fn closest_point(&self, a: DVec3, b: DVec3, p: DVec3) -> DVec3 {
        match *self {
            Curve::Line => {
                let d = b - a;
                let l2 = d.length_squared();
                if l2 == 0.0 {
                    return a;
                }
                a + d * ((p - a).dot(d) / l2).clamp(0.0, 1.0)
            }
            Curve::Arc { radius, .. } => {
                let Some((u, v, c)) = self.frame(a) else {
                    return a;
                };
                let w = p - c;
                let (x, y) = (w.dot(u), w.dot(v));
                if x == 0.0 && y == 0.0 {
                    return a;
                }
                let th = y.atan2(x).rem_euclid(TAU);
                if th <= self.sweep(a, b) {
                    c + (u * th.cos() + v * th.sin()) * radius
                } else if p.distance(a) <= p.distance(b) {
                    a
                } else {
                    b
                }
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

/// How far `p` is from the circle about `centre` with unit `axis` and
/// `radius`.
pub fn circle_distance(centre: DVec3, axis: DVec3, radius: f64, p: DVec3) -> f64 {
    let w = p - centre;
    let h = w.dot(axis);
    ((w - axis * h).length() - radius).hypot(h)
}

/// An arc as drawn: its circle, and the sweep counter-clockwise about
/// `normal` from its start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArcSpec {
    pub centre: DVec3,
    pub normal: DVec3,
    pub radius: f64,
    pub sweep: f64,
}

/// The arc from `a` through `m` to `b`. None if the three points are in a
/// line (or two are the same), within `eps`.
pub fn arc_through(a: DVec3, m: DVec3, b: DVec3, eps: f64) -> Option<ArcSpec> {
    let (u, w) = (m - a, b - a);
    let n = u.cross(w);
    let nl = n.length();
    if nl <= eps * u.length().max(w.length()) || u.length() <= eps || w.length() <= eps {
        return None;
    }
    // The circumcentre, in the plane of the three points.
    let centre =
        a + (n.cross(u) * w.length_squared() + w.cross(n) * u.length_squared()) / (2.0 * nl * nl);
    let normal = n / nl;
    let radius = centre.distance(a);
    let arc = Curve::Arc {
        centre,
        axis: normal,
        radius,
    };
    Some(ArcSpec {
        centre,
        normal,
        radius,
        sweep: arc.sweep(a, b),
    })
}

/// The arc from `a` to `b` in the plane with `normal`, bulging `bulge` from
/// the chord's middle: positive to the left of `a`→`b` about `normal`. None
/// if the chord or the bulge is shorter than `eps`.
pub fn arc_bulge(a: DVec3, b: DVec3, normal: DVec3, bulge: f64, eps: f64) -> Option<ArcSpec> {
    let d = b - a;
    let n = normal.normalize_or_zero();
    let left = n.cross(d).normalize_or_zero();
    if d.length() <= eps || bulge.abs() <= eps || left == DVec3::ZERO {
        return None;
    }
    arc_through(a, a.lerp(b, 0.5) + left * bulge, b, eps)
}

/// The bulge of the arc of `radius` on a chord of `chord` length: the
/// shorter arc. None if the radius is less than half the chord.
pub fn bulge_for_radius(chord: f64, radius: f64) -> Option<f64> {
    let h = chord / 2.0;
    (radius >= h).then(|| radius - (radius * radius - h * h).max(0.0).sqrt())
}

/// Where the circle about `centre` (unit `axis`, `radius`) meets the line
/// through `p` and `q`: none, one (touching, or the line passing through
/// its plane) or two points. `eps` is the model tolerance: it decides
/// touching and whether the line lies in the circle's plane.
pub fn circle_line(
    centre: DVec3,
    axis: DVec3,
    radius: f64,
    p: DVec3,
    q: DVec3,
    eps: f64,
) -> Vec<DVec3> {
    let len = p.distance(q);
    if len <= eps {
        return Vec::new();
    }
    let d = (q - p) / len;
    let (hp, hq) = ((p - centre).dot(axis), (q - centre).dot(axis));
    if hp.abs() <= eps && hq.abs() <= eps {
        // In the plane: around the foot of the centre on the line.
        let foot = p + d * (centre - p).dot(d);
        let h = foot.distance(centre);
        if (h - radius).abs() <= eps {
            return vec![foot];
        }
        if h > radius {
            return Vec::new();
        }
        let s = (radius * radius - h * h).sqrt();
        return vec![foot - d * s, foot + d * s];
    }
    // Through the plane: at most the one point where it passes.
    let den = d.dot(axis);
    if den.abs() * len <= eps {
        return Vec::new();
    }
    let x = p - d * (hp / den);
    if circle_distance(centre, axis, radius, x) <= eps {
        vec![x]
    } else {
        Vec::new()
    }
}

/// Where two circles meet: none, one (touching) or two points. Circles in
/// one plane are solved there; otherwise the first circle meets the
/// second's plane at up to two points, kept if they lie on the second
/// circle. The same circle twice gives none. `eps` as for [`circle_line`].
pub fn circle_circle(
    (c1, n1, r1): (DVec3, DVec3, f64),
    (c2, n2, r2): (DVec3, DVec3, f64),
    eps: f64,
) -> Vec<DVec3> {
    if n1.cross(n2).length() * r1.max(r2) <= eps {
        let h = (c2 - c1).dot(n1);
        if h.abs() > eps {
            return Vec::new();
        }
        let d = c2 - c1 - n1 * h;
        let dist = d.length();
        if dist <= eps || dist > r1 + r2 + eps || dist < (r1 - r2).abs() - eps {
            return Vec::new();
        }
        let e = d / dist;
        let along = (dist * dist + r1 * r1 - r2 * r2) / (2.0 * dist);
        if (dist - (r1 + r2)).abs() <= eps || (dist - (r1 - r2).abs()).abs() <= eps {
            return vec![c1 + e * r1 * along.signum()];
        }
        let f = n1.cross(e);
        let s = (r1 * r1 - along * along).max(0.0).sqrt();
        return vec![c1 + e * along - f * s, c1 + e * along + f * s];
    }
    // The first circle against the second's plane: the height above it at
    // angle t is c + a cos t + b sin t = c + rr cos(t - phi).
    let u = n1.any_orthonormal_vector();
    let v = n1.cross(u);
    let (a, b, c) = (r1 * u.dot(n2), r1 * v.dot(n2), (c1 - c2).dot(n2));
    let rr = a.hypot(b);
    if c.abs() > rr + eps {
        return Vec::new();
    }
    let phi = b.atan2(a);
    let at = |t: f64| c1 + (u * t.cos() + v * t.sin()) * r1;
    let pts = if (c.abs() - rr).abs() <= eps {
        vec![at(if c > 0.0 {
            phi + std::f64::consts::PI
        } else {
            phi
        })]
    } else {
        let w = (-c / rr).clamp(-1.0, 1.0).acos();
        vec![at(phi - w), at(phi + w)]
    };
    pts.into_iter()
        .filter(|x| circle_distance(c2, n2, r2, *x) <= eps)
        .collect()
}

/// A round surface about an axis (Exact Curves part 4): every point is
/// `radius` from the line through `origin` along the unit `axis`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cylinder {
    pub origin: DVec3,
    /// Unit length.
    pub axis: DVec3,
    pub radius: f64,
    /// The face points away from the axis (true) or towards it.
    pub outward: bool,
}

impl Cylinder {
    /// The nearest point of the axis to `p`.
    pub fn axis_point(&self, p: DVec3) -> DVec3 {
        self.origin + self.axis * (p - self.origin).dot(self.axis)
    }

    /// How far `p` is from the surface (positive outside).
    pub fn distance(&self, p: DVec3) -> f64 {
        (p - self.axis_point(p)).length() - self.radius
    }

    /// The face's normal at `p` (a point on or near the surface).
    pub fn normal_at(&self, p: DVec3) -> DVec3 {
        let out = (p - self.axis_point(p)).normalize_or_zero();
        if self.outward { out } else { -out }
    }

    /// The same surface, facing the other way.
    pub fn flipped(&self) -> Cylinder {
        Cylinder {
            outward: !self.outward,
            ..*self
        }
    }

    /// The surface after `m`, if it is still a cylinder (moves, turns,
    /// mirrors and even scales; an uneven scale gives None). A mirror turns
    /// the face's loops round, so the face then points the other way.
    pub fn transformed(&self, m: &DAffine3) -> Option<Cylinder> {
        let s = even_scale(m)?;
        Some(Cylinder {
            origin: m.transform_point3(self.origin),
            axis: (m.matrix3 * self.axis).normalize(),
            radius: self.radius * s,
            outward: self.outward != (m.matrix3.determinant() < 0.0),
        })
    }

    /// Where the ray from `o` along `d` meets the surface: up to two
    /// distances along it (in units of `d`), nearest first.
    pub fn ray_hits(&self, o: DVec3, d: DVec3) -> Vec<f64> {
        let flat = |v: DVec3| v - self.axis * v.dot(self.axis);
        let (w, e) = (flat(o - self.origin), flat(d));
        let a = e.length_squared();
        if a <= 1e-300 {
            return Vec::new();
        }
        let b = 2.0 * w.dot(e);
        let c = w.length_squared() - self.radius * self.radius;
        let disc = b * b - 4.0 * a * c;
        if disc < 0.0 {
            return Vec::new();
        }
        let r = disc.sqrt();
        // The stable form of the two roots.
        let q = -0.5 * (b + b.signum() * r);
        let (t1, t2) = (q / a, if q != 0.0 { c / q } else { -b / (2.0 * a) });
        let (lo, hi) = (t1.min(t2), t1.max(t2));
        if lo == hi { vec![lo] } else { vec![lo, hi] }
    }

    /// The circle of this cylinder through `p`: its centre and axis.
    pub fn circle_at(&self, p: DVec3) -> (DVec3, DVec3) {
        (self.axis_point(p), self.axis)
    }

    /// Whether `curve` from `a` to `b` lies on the surface, within `eps`:
    /// a line along the axis, or an arc of one of its circles.
    pub fn holds(&self, curve: &Curve, a: DVec3, b: DVec3, eps: f64) -> bool {
        let on = |p: DVec3| self.distance(p).abs() <= eps;
        if !on(a) || !on(b) {
            return false;
        }
        match *curve {
            Curve::Line => {
                let d = b - a;
                (d - self.axis * d.dot(self.axis)).length() <= eps
            }
            Curve::Arc {
                centre,
                axis,
                radius,
            } => {
                (radius - self.radius).abs() <= eps
                    && (axis.dot(self.axis).abs() - 1.0).abs() <= 1e-9
                    && centre.distance(self.axis_point(centre)) <= eps
            }
        }
    }
}

/// The surface a face lies on.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Surface {
    Plane(Plane),
    Cylinder(Cylinder),
    Cone(Cone),
    Sphere(Sphere),
    Torus(Torus),
}

impl Surface {
    /// The plane, for a flat face; None for a curved one.
    pub fn plane(&self) -> Option<&Plane> {
        match self {
            Surface::Plane(p) => Some(p),
            _ => None,
        }
    }

    pub fn is_curved(&self) -> bool {
        !matches!(self, Surface::Plane(_))
    }

    /// Which way the face points at `p` (a point on it).
    pub fn normal_at(&self, p: DVec3) -> DVec3 {
        match self {
            Surface::Plane(pl) => pl.normal,
            Surface::Cylinder(c) => c.normal_at(p),
            Surface::Cone(c) => c.normal_at(p),
            Surface::Sphere(x) => x.normal_at(p),
            Surface::Torus(t) => t.normal_at(p),
        }
    }

    /// Whether a curved face points out of its surface (away from the axis,
    /// the centre or the tube's middle). True for a plane.
    pub fn outward(&self) -> bool {
        match self {
            Surface::Plane(_) => true,
            Surface::Cylinder(c) => c.outward,
            Surface::Cone(c) => c.outward,
            Surface::Sphere(x) => x.outward,
            Surface::Torus(t) => t.outward,
        }
    }

    /// How far `p` is from the surface, signed by the face's side.
    pub fn distance(&self, p: DVec3) -> f64 {
        let (d, out) = match self {
            Surface::Plane(pl) => return pl.distance(p),
            Surface::Cylinder(c) => (c.distance(p), c.outward),
            Surface::Cone(c) => (c.distance(p), c.outward),
            Surface::Sphere(x) => (x.distance(p), x.outward),
            Surface::Torus(t) => (t.distance(p), t.outward),
        };
        if out { d } else { -d }
    }

    /// The nearest point of the surface to `p`.
    pub fn project(&self, p: DVec3) -> DVec3 {
        match self {
            Surface::Plane(pl) => p - pl.normal * pl.distance(p),
            Surface::Cylinder(c) => c.project(p),
            Surface::Cone(c) => c.project(p),
            Surface::Sphere(x) => x.project(p),
            Surface::Torus(t) => t.project(p),
        }
    }

    /// The same surface, facing the other way.
    pub fn flipped(&self) -> Surface {
        match self {
            Surface::Plane(p) => Surface::Plane(p.flipped()),
            Surface::Cylinder(c) => Surface::Cylinder(c.flipped()),
            Surface::Cone(c) => Surface::Cone(c.flipped()),
            Surface::Sphere(x) => Surface::Sphere(x.flipped()),
            Surface::Torus(t) => Surface::Torus(t.flipped()),
        }
    }

    /// The surface after `m`, if it keeps its kind exactly (moves, turns,
    /// mirrors and even scales). A mirror turns a face's loops round, so a
    /// curved face then points the other way.
    pub fn transformed(&self, m: &DAffine3) -> Option<Surface> {
        let (s, even) = revolved::scale_of(m);
        even.then(|| revolved::moved(self, m, s))
    }

    /// As [`Surface::transformed`], but an uneven scale gives the nearest
    /// even one (the model check then finds points off the surface, until
    /// the segment fallback of DECISIONS #32).
    pub fn transformed_loosely(&self, m: &DAffine3) -> Surface {
        let (s, _) = revolved::scale_of(m);
        revolved::moved(self, m, s)
    }

    /// The surface after a rigid motion given as `f` for points and `nf`
    /// for directions (sizes are kept).
    pub fn mapped(&self, f: impl Fn(DVec3) -> DVec3, nf: impl Fn(DVec3) -> DVec3) -> Surface {
        match *self {
            Surface::Plane(p) => Surface::Plane(Plane::new(f(p.origin), nf(p.normal))),
            Surface::Cylinder(c) => Surface::Cylinder(Cylinder {
                origin: f(c.origin),
                axis: nf(c.axis),
                ..c
            }),
            Surface::Cone(c) => Surface::Cone(Cone {
                apex: f(c.apex),
                axis: nf(c.axis),
                ..c
            }),
            Surface::Sphere(x) => Surface::Sphere(Sphere {
                centre: f(x.centre),
                axis: nf(x.axis),
                ..x
            }),
            Surface::Torus(t) => Surface::Torus(Torus {
                centre: f(t.centre),
                axis: nf(t.axis),
                ..t
            }),
        }
    }

    /// Whether `curve` from `a` to `b` lies on a curved surface, within
    /// `eps`. For a plane: whether an arc lies in its plane.
    pub fn holds(&self, curve: &Curve, a: DVec3, b: DVec3, eps: f64) -> bool {
        match self {
            Surface::Plane(pl) => match curve {
                Curve::Line => true,
                Curve::Arc { axis, .. } => (axis.dot(pl.normal).abs() - 1.0).abs() <= 1e-9,
            },
            Surface::Cylinder(c) => c.holds(curve, a, b, eps),
            Surface::Cone(c) => c.holds(curve, a, b, eps),
            Surface::Sphere(x) => x.holds(curve, a, b, eps),
            Surface::Torus(t) => t.holds(curve, a, b, eps),
        }
    }

    /// The surface as a turned profile; None for a plane.
    pub fn revolution(&self) -> Option<Revolution> {
        match self {
            Surface::Plane(_) => None,
            Surface::Cylinder(c) => Some(c.revolution()),
            Surface::Cone(c) => Some(c.revolution()),
            Surface::Sphere(x) => Some(x.revolution()),
            Surface::Torus(t) => Some(t.revolution()),
        }
    }

    /// Every number finite and every size sensible.
    pub fn is_valid(&self) -> bool {
        let unit = |a: DVec3| a.is_finite() && a != DVec3::ZERO;
        let len = |x: f64| x.is_finite() && x > 0.0;
        match *self {
            Surface::Plane(pl) => pl.origin.is_finite() && pl.normal.is_finite(),
            Surface::Cylinder(c) => c.origin.is_finite() && unit(c.axis) && len(c.radius),
            Surface::Cone(c) => {
                c.apex.is_finite()
                    && unit(c.axis)
                    && c.half_angle.is_finite()
                    && c.half_angle > 0.0
                    && c.half_angle < std::f64::consts::FRAC_PI_2
            }
            Surface::Sphere(x) => x.centre.is_finite() && unit(x.axis) && len(x.radius),
            Surface::Torus(t) => {
                t.centre.is_finite() && unit(t.axis) && len(t.major) && len(t.minor)
            }
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn rays_meet_a_cylinder_exactly() {
        let c = Cylinder {
            origin: DVec3::new(1.0, 0.0, 0.0),
            axis: DVec3::Z,
            radius: 2.0,
            outward: true,
        };
        let t = c.ray_hits(DVec3::new(-5.0, 0.0, 3.0), DVec3::X);
        assert_eq!(t, vec![4.0, 8.0]);
        assert!(c.ray_hits(DVec3::new(-5.0, 3.0, 0.0), DVec3::X).is_empty());
        assert!(
            c.ray_hits(DVec3::ZERO, DVec3::Z).is_empty(),
            "along the axis"
        );
        let d = DVec3::new(1.0, 0.3, 0.2);
        for t in c.ray_hits(DVec3::new(-4.0, -1.0, 0.0), d) {
            assert!(c.distance(DVec3::new(-4.0, -1.0, 0.0) + d * t).abs() < 1e-12);
        }
    }

    #[test]
    fn arcs_through_three_points_and_by_bulge() {
        use std::f64::consts::PI;
        let (a, m, b) = (DVec3::X, DVec3::Y, -DVec3::X);
        let s = arc_through(a, m, b, 1e-9).expect("an arc");
        assert!(s.centre.length() < 1e-12 && (s.radius - 1.0).abs() < 1e-12);
        assert!(s.normal.distance(DVec3::Z) < 1e-12);
        assert!((s.sweep - PI).abs() < 1e-12);
        // The other way round goes about -Z, still through m.
        let s = arc_through(b, m, a, 1e-9).expect("an arc");
        assert!(s.normal.distance(-DVec3::Z) < 1e-12 && (s.sweep - PI).abs() < 1e-12);
        // Through a point on the far side: the long way, 3/2 of a half turn.
        let s = arc_through(a, -DVec3::Y, DVec3::Y, 1e-9).expect("an arc");
        assert!(s.normal.distance(-DVec3::Z) < 1e-12);
        assert!((s.sweep - 1.5 * PI).abs() < 1e-12, "{}", s.sweep);
        assert!(arc_through(a, DVec3::ZERO, b, 1e-9).is_none(), "in a line");
        // A bulge of half the chord is a half circle.
        let s = arc_bulge(a, b, DVec3::Z, -1.0, 1e-9).expect("an arc");
        assert!(s.centre.length() < 1e-12 && (s.sweep - PI).abs() < 1e-12);
        let s = arc_bulge(DVec3::ZERO, 2.0 * DVec3::X, DVec3::Z, 0.5, 1e-9).expect("an arc");
        assert!((s.radius - 1.25).abs() < 1e-12, "{}", s.radius);
        assert_eq!(bulge_for_radius(2.0, 1.25), Some(0.5));
        assert_eq!(bulge_for_radius(2.0, 0.5), None);
    }

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
    fn closest_points_on_an_arc_and_a_line() {
        let (c, a, b) = upper();
        assert!(close(
            c.closest_point(a, b, DVec3::new(0.0, 3.0, 1.0)),
            DVec3::Y
        ));
        // Below the upper half: the nearer end.
        assert!(close(c.closest_point(a, b, DVec3::new(0.9, -2.0, 0.0)), a));
        let p = Curve::Line.closest_point(DVec3::ZERO, DVec3::X, DVec3::new(0.5, 2.0, 0.0));
        assert!(close(p, DVec3::new(0.5, 0.0, 0.0)));
    }

    #[test]
    fn circles_meet_lines() {
        let eps = 1e-9;
        let (o, z) = (DVec3::ZERO, DVec3::Z);
        let x = DVec3::new(-2.0, 0.5, 0.0);
        let pts = circle_line(o, z, 1.0, x, x + DVec3::X, eps);
        assert_eq!(pts.len(), 2);
        assert!(pts.iter().all(|p| (p.length() - 1.0).abs() < 1e-12));
        // Touching at the top; missing above it.
        let top = circle_line(
            o,
            z,
            1.0,
            DVec3::new(-2.0, 1.0, 0.0),
            DVec3::new(2.0, 1.0, 0.0),
            eps,
        );
        assert_eq!(top.len(), 1);
        assert!(close(top[0], DVec3::Y));
        assert!(
            circle_line(
                o,
                z,
                1.0,
                DVec3::new(0.0, 2.0, 0.0),
                DVec3::new(1.0, 2.0, 0.0),
                eps
            )
            .is_empty()
        );
        // Through the plane, on the circle or not.
        let up = circle_line(
            o,
            z,
            1.0,
            DVec3::new(1.0, 0.0, -1.0),
            DVec3::new(1.0, 0.0, 1.0),
            eps,
        );
        assert_eq!(up.len(), 1);
        assert!(close(up[0], DVec3::X));
        assert!(
            circle_line(
                o,
                z,
                1.0,
                DVec3::new(0.0, 0.0, -1.0),
                DVec3::new(0.0, 0.0, 1.0),
                eps
            )
            .is_empty()
        );
    }

    #[test]
    fn circles_meet_circles() {
        let eps = 1e-9;
        let z = DVec3::Z;
        let one = |c: DVec3, r: f64| (c, z, r);
        let pts = circle_circle(one(DVec3::ZERO, 1.0), one(DVec3::X, 1.0), eps);
        assert_eq!(pts.len(), 2);
        let h = 0.75f64.sqrt();
        assert!(pts.iter().any(|p| close(*p, DVec3::new(0.5, h, 0.0))));
        assert!(pts.iter().any(|p| close(*p, DVec3::new(0.5, -h, 0.0))));
        // Touching from outside and inside.
        let out = circle_circle(
            one(DVec3::ZERO, 1.0),
            one(DVec3::new(2.0, 0.0, 0.0), 1.0),
            eps,
        );
        assert_eq!(out.len(), 1);
        assert!(close(out[0], DVec3::X));
        let inside = circle_circle(one(DVec3::ZERO, 2.0), one(DVec3::X, 1.0), eps);
        assert_eq!(inside.len(), 1);
        assert!(close(inside[0], DVec3::new(2.0, 0.0, 0.0)));
        let inside = circle_circle(one(DVec3::X, 1.0), one(DVec3::ZERO, 2.0), eps);
        assert!(close(inside[0], DVec3::new(2.0, 0.0, 0.0)));
        // Apart, nested, the same circle.
        assert!(
            circle_circle(
                one(DVec3::ZERO, 1.0),
                one(DVec3::new(3.0, 0.0, 0.0), 1.0),
                eps
            )
            .is_empty()
        );
        assert!(circle_circle(one(DVec3::ZERO, 3.0), one(DVec3::X, 1.0), eps).is_empty());
        assert!(circle_circle(one(DVec3::ZERO, 1.0), one(DVec3::ZERO, 1.0), eps).is_empty());
        // A standing circle through a lying one.
        let pts = circle_circle(one(DVec3::ZERO, 1.0), (DVec3::ZERO, DVec3::Y, 1.0), eps);
        assert_eq!(pts.len(), 2);
        assert!(
            pts.iter()
                .all(|p| p.z.abs() < 1e-12 && (p.length() - 1.0).abs() < 1e-12)
        );
        // Standing, through only one of those points.
        let pts = circle_circle(
            one(DVec3::ZERO, 1.0),
            (DVec3::new(1.0, 0.0, 1.0), DVec3::Y, 1.0),
            eps,
        );
        assert_eq!(pts.len(), 1);
        assert!(close(pts[0], DVec3::X));
        // Standing in the plane x = 1, which only touches the lying one.
        let pts = circle_circle(
            one(DVec3::ZERO, 1.0),
            (DVec3::new(1.0, 0.0, 1.0), DVec3::X, 1.0),
            eps,
        );
        assert_eq!(pts.len(), 1);
        assert!(close(pts[0], DVec3::X));
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
