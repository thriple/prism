//! Cones, spheres and tori (Exact Curves part 5b), and what every surface
//! turned about an axis shares: a place on it is an angle round the axis
//! and a place along its profile (the meridian).
//!
//! These faces are made by Follow Me (steps 5c and 5d). Like cylinder
//! faces, a face never goes all the way round its axis: seams split it.

use prism_math::{DAffine3, DVec2, DVec3, Plane};
use serde::{Deserialize, Serialize};

use crate::{Curve, Cylinder, even_scale};

/// Unit vectors `u`, `v` square to `axis`, with `u` × `v` = `axis`.
pub fn axis_frame(origin: DVec3, axis: DVec3) -> (DVec3, DVec3) {
    let f = Plane::new(origin, axis).frame();
    if f.normal.dot(axis) >= 0.0 {
        (f.u, f.v)
    } else {
        (f.v, f.u)
    }
}

/// The direction straight out from the axis towards `p`, and how far `p`
/// is from the axis. On the axis, any direction square to it.
fn out_from(origin: DVec3, axis: DVec3, p: DVec3) -> (DVec3, f64) {
    let q = p - origin;
    let r = q - axis * q.dot(axis);
    let d = r.length();
    match r.try_normalize() {
        Some(e) => (e, d),
        None => (axis_frame(origin, axis).0, 0.0),
    }
}

/// The two parallel axes test `holds` uses for arcs.
fn parallel(a: DVec3, b: DVec3) -> bool {
    (a.dot(b).abs() - 1.0).abs() <= 1e-9
}

/// One nappe of a round cone: every point at height `h` ≥ 0 above `apex`
/// along the unit `axis` is `h · tan(half_angle)` from the axis.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cone {
    pub apex: DVec3,
    /// Unit length, from the apex into the cone.
    pub axis: DVec3,
    /// Between 0 and a quarter turn, not including either.
    pub half_angle: f64,
    /// The face points away from the axis (true) or towards it.
    pub outward: bool,
}

impl Cone {
    /// The point's place in the plane through the axis: (out, up).
    fn local(&self, p: DVec3) -> (DVec3, f64, f64) {
        let (e, r) = out_from(self.apex, self.axis, p);
        (e, r, (p - self.apex).dot(self.axis))
    }

    /// How far `p` is from the surface (positive outside: away from the
    /// axis).
    pub fn distance(&self, p: DVec3) -> f64 {
        let (_, r, h) = self.local(p);
        let (s, c) = self.half_angle.sin_cos();
        if r * s + h * c < 0.0 {
            // Behind the apex: the apex is nearest.
            return p.distance(self.apex);
        }
        r * c - h * s
    }

    /// The nearest point of the surface to `p`.
    pub fn project(&self, p: DVec3) -> DVec3 {
        let (e, r, h) = self.local(p);
        let (s, c) = self.half_angle.sin_cos();
        let t = r * s + h * c;
        if t <= 0.0 {
            return self.apex;
        }
        self.apex + (self.axis * c + e * s) * t
    }

    pub fn normal_at(&self, p: DVec3) -> DVec3 {
        let (e, _, _) = self.local(p);
        let (s, c) = self.half_angle.sin_cos();
        let out = (e * c - self.axis * s).normalize_or_zero();
        if self.outward { out } else { -out }
    }

    pub fn flipped(&self) -> Cone {
        Cone {
            outward: !self.outward,
            ..*self
        }
    }

    /// After `m` scaling by `s` (angles are kept).
    fn moved(&self, m: &DAffine3) -> Cone {
        Cone {
            apex: m.transform_point3(self.apex),
            axis: (m.matrix3 * self.axis).normalize(),
            half_angle: self.half_angle,
            outward: self.outward != (m.matrix3.determinant() < 0.0),
        }
    }

    /// Whether `curve` from `a` to `b` lies on it: a line through the
    /// apex, or an arc of one of its circles.
    pub fn holds(&self, curve: &Curve, a: DVec3, b: DVec3, eps: f64) -> bool {
        let on = |p: DVec3| self.distance(p).abs() <= eps;
        if !on(a) || !on(b) {
            return false;
        }
        match *curve {
            Curve::Spline(ref sp) => sp.inner_points().into_iter().all(on),
            Curve::Line => {
                let d = (b - a).normalize_or_zero();
                let w = self.apex - a;
                (w - d * w.dot(d)).length() <= eps
            }
            Curve::Arc { centre, axis, .. } => {
                parallel(axis, self.axis)
                    && centre.distance(self.apex + self.axis * (centre - self.apex).dot(self.axis))
                        <= eps
            }
        }
    }
}

/// A sphere. It keeps the axis it was turned about, which its seams and
/// circles follow.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sphere {
    pub centre: DVec3,
    /// Unit length.
    pub axis: DVec3,
    pub radius: f64,
    /// The face points away from the centre (true) or towards it.
    pub outward: bool,
}

impl Sphere {
    pub fn distance(&self, p: DVec3) -> f64 {
        p.distance(self.centre) - self.radius
    }

    pub fn project(&self, p: DVec3) -> DVec3 {
        let d = (p - self.centre)
            .try_normalize()
            .unwrap_or_else(|| axis_frame(self.centre, self.axis).0);
        self.centre + d * self.radius
    }

    pub fn normal_at(&self, p: DVec3) -> DVec3 {
        let out = (p - self.centre).normalize_or_zero();
        if self.outward { out } else { -out }
    }

    pub fn flipped(&self) -> Sphere {
        Sphere {
            outward: !self.outward,
            ..*self
        }
    }

    fn moved(&self, m: &DAffine3, s: f64) -> Sphere {
        Sphere {
            centre: m.transform_point3(self.centre),
            axis: (m.matrix3 * self.axis).normalize(),
            radius: self.radius * s,
            outward: self.outward != (m.matrix3.determinant() < 0.0),
        }
    }

    /// Whether `curve` lies on it: an arc of any circle on the sphere.
    pub fn holds(&self, curve: &Curve, a: DVec3, b: DVec3, eps: f64) -> bool {
        let on = |p: DVec3| self.distance(p).abs() <= eps;
        if !on(a) || !on(b) {
            return false;
        }
        match *curve {
            Curve::Spline(ref sp) => sp.inner_points().into_iter().all(on),
            Curve::Line => a.distance(b) <= eps,
            Curve::Arc {
                centre,
                axis,
                radius,
            } => {
                let w = centre - self.centre;
                (w - axis * w.dot(axis)).length() <= eps
                    && (w.length().hypot(radius) - self.radius).abs() <= eps
            }
        }
    }
}

/// A torus: the circle of radius `minor` about a point `major` from the
/// axis, turned about the axis.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Torus {
    pub centre: DVec3,
    /// Unit length.
    pub axis: DVec3,
    pub major: f64,
    pub minor: f64,
    /// The face points away from the tube's middle (true) or into it.
    pub outward: bool,
}

impl Torus {
    /// The nearest point of the tube's middle circle to `p`.
    fn core(&self, p: DVec3) -> DVec3 {
        let (e, _) = out_from(self.centre, self.axis, p);
        self.centre + e * self.major
    }

    pub fn distance(&self, p: DVec3) -> f64 {
        p.distance(self.core(p)) - self.minor
    }

    pub fn project(&self, p: DVec3) -> DVec3 {
        let k = self.core(p);
        let d = (p - k).try_normalize().unwrap_or(self.axis);
        k + d * self.minor
    }

    pub fn normal_at(&self, p: DVec3) -> DVec3 {
        let out = (p - self.core(p)).normalize_or_zero();
        if self.outward { out } else { -out }
    }

    pub fn flipped(&self) -> Torus {
        Torus {
            outward: !self.outward,
            ..*self
        }
    }

    fn moved(&self, m: &DAffine3, s: f64) -> Torus {
        Torus {
            centre: m.transform_point3(self.centre),
            axis: (m.matrix3 * self.axis).normalize(),
            major: self.major * s,
            minor: self.minor * s,
            outward: self.outward != (m.matrix3.determinant() < 0.0),
        }
    }

    /// Whether `curve` lies on it: an arc of a circle about the axis, or of
    /// the tube's own circle (a meridian).
    pub fn holds(&self, curve: &Curve, a: DVec3, b: DVec3, eps: f64) -> bool {
        let on = |p: DVec3| self.distance(p).abs() <= eps;
        if !on(a) || !on(b) {
            return false;
        }
        match *curve {
            Curve::Spline(ref sp) => sp.inner_points().into_iter().all(on),
            Curve::Line => a.distance(b) <= eps,
            Curve::Arc {
                centre,
                axis,
                radius,
            } => {
                let w = centre - self.centre;
                if parallel(axis, self.axis) {
                    return (w - self.axis * w.dot(self.axis)).length() <= eps;
                }
                let (e, r) = out_from(self.centre, self.axis, centre);
                (radius - self.minor).abs() <= eps
                    && w.dot(self.axis).abs() <= eps
                    && (r - self.major).abs() <= eps
                    && axis.dot(self.axis).abs() <= 1e-9
                    && axis.dot(e).abs() <= 1e-9
            }
        }
    }
}

/// A surface turned about an axis, as an angle round the axis (`θ`) and a
/// place along the profile (`s`). The profile is a line (cylinders and
/// cones: `s` is the height along the axis) or a circle (spheres and tori:
/// `s` is the angle round the profile's circle).
///
/// Walking a face's loop with the face's outside towards you, its points
/// go counter-clockwise in (θ, s) when the face points outward.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Revolution {
    pub origin: DVec3,
    pub axis: DVec3,
    pub u: DVec3,
    pub v: DVec3,
    pub profile: Profile,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Profile {
    /// Distance from the axis `r0 + slope · s` at height `s`.
    Line { r0: f64, slope: f64 },
    /// The circle of `radius` about the point `r0` from the axis at height
    /// `h0`: distance `r0 + radius cos s`, height `h0 + radius sin s`.
    Circle { r0: f64, h0: f64, radius: f64 },
}

impl Revolution {
    fn new(origin: DVec3, axis: DVec3, profile: Profile) -> Self {
        let (u, v) = axis_frame(origin, axis);
        Revolution {
            origin,
            axis,
            u,
            v,
            profile,
        }
    }

    /// Distance from the axis and height at `s`, and how fast each changes.
    pub fn at(&self, s: f64) -> (f64, f64, f64, f64) {
        match self.profile {
            Profile::Line { r0, slope } => (r0 + slope * s, s, slope, 1.0),
            Profile::Circle { r0, h0, radius } => {
                let (sn, cs) = s.sin_cos();
                (
                    r0 + radius * cs,
                    h0 + radius * sn,
                    -radius * sn,
                    radius * cs,
                )
            }
        }
    }

    /// Where (θ, s) sits in a texture, in metres (Basic Materials part 2):
    /// a cylinder unrolls exactly as (θ·r, s); a cone exactly into a sector
    /// of a disc (no stretch); a sphere is (θ·R, s·R) and a torus
    /// (θ·R, s·r), which stretch towards the poles and the inner rim, as in
    /// every modeller.
    pub fn texture_at(&self, theta: f64, s: f64) -> DVec2 {
        match self.profile {
            Profile::Line { r0, slope: 0.0 } => DVec2::new(theta * r0, s),
            Profile::Line { r0, slope } => {
                // Unrolled, the cone is a sector: a point is its slant
                // distance from the apex, at θ·sin(half angle) round it.
                let k = slope.abs() / slope.hypot(1.0);
                let slant = (r0 + slope * s) / k;
                let a = theta * k;
                DVec2::new(slant * a.sin(), -slant * a.cos())
            }
            Profile::Circle { r0, radius, .. } => {
                let round = if r0 > 0.0 { r0 } else { radius };
                DVec2::new(theta * round, s * radius)
            }
        }
    }

    /// (θ, s) of a point on or near the surface; θ in (-π, π].
    pub fn param(&self, p: DVec3) -> (f64, f64) {
        let q = p - self.origin;
        let (x, y, h) = (q.dot(self.u), q.dot(self.v), q.dot(self.axis));
        let theta = y.atan2(x);
        let s = match self.profile {
            Profile::Line { .. } => h,
            Profile::Circle { r0, h0, .. } => (h - h0).atan2(x.hypot(y) - r0),
        };
        (theta, s)
    }

    /// How far `p` is from the axis.
    pub fn axis_distance(&self, p: DVec3) -> f64 {
        let q = p - self.origin;
        (q - self.axis * q.dot(self.axis)).length()
    }

    /// The point at (θ, s).
    pub fn point(&self, theta: f64, s: f64) -> DVec3 {
        let (r, h, _, _) = self.at(s);
        let (sn, cs) = theta.sin_cos();
        self.origin + self.axis * h + (self.u * cs + self.v * sn) * r
    }

    /// The outward normal at (θ, s) (away from the axis, or from the
    /// profile circle's centre).
    pub fn normal(&self, theta: f64, s: f64) -> DVec3 {
        let (_, _, dr, dh) = self.at(s);
        let (sn, cs) = theta.sin_cos();
        let e = self.u * cs + self.v * sn;
        (e * dh - self.axis * dr).normalize_or_zero()
    }

    /// `∫ g dθ` for the integrand `g` of a face's volume share (position ·
    /// the unnormalised normal), up to a part that depends on `s` alone,
    /// which vanishes round a closed loop. See `prism_ops::health`.
    pub fn volume_potential(&self, theta: f64, s: f64) -> f64 {
        let (r, h, dr, dh) = self.at(s);
        let (cu, cv, ca) = (
            self.origin.dot(self.u),
            self.origin.dot(self.v),
            self.origin.dot(self.axis),
        );
        let (sn, cs) = theta.sin_cos();
        r * (dh * (cu * sn - cv * cs + r * theta) - dr * (ca + h) * theta)
    }
}

impl Cylinder {
    pub fn project(&self, p: DVec3) -> DVec3 {
        let (e, _) = out_from(self.origin, self.axis, p);
        let at = self.axis_point(p);
        at + e * self.radius
    }

    pub fn revolution(&self) -> Revolution {
        Revolution::new(
            self.origin,
            self.axis,
            Profile::Line {
                r0: self.radius,
                slope: 0.0,
            },
        )
    }

    fn moved(&self, m: &DAffine3, s: f64) -> Cylinder {
        Cylinder {
            origin: m.transform_point3(self.origin),
            axis: (m.matrix3 * self.axis).normalize(),
            radius: self.radius * s,
            outward: self.outward != (m.matrix3.determinant() < 0.0),
        }
    }
}

impl Cone {
    pub fn revolution(&self) -> Revolution {
        Revolution::new(
            self.apex,
            self.axis,
            Profile::Line {
                r0: 0.0,
                slope: self.half_angle.tan(),
            },
        )
    }
}

impl Sphere {
    pub fn revolution(&self) -> Revolution {
        Revolution::new(
            self.centre,
            self.axis,
            Profile::Circle {
                r0: 0.0,
                h0: 0.0,
                radius: self.radius,
            },
        )
    }
}

impl Torus {
    pub fn revolution(&self) -> Revolution {
        Revolution::new(
            self.centre,
            self.axis,
            Profile::Circle {
                r0: self.major,
                h0: 0.0,
                radius: self.minor,
            },
        )
    }
}

/// The surface after `m` with every length scaled by `s` (an even scale,
/// or the nearest even one for an uneven `m`).
pub(crate) fn moved(surface: &crate::Surface, m: &DAffine3, s: f64) -> crate::Surface {
    use crate::Surface as S;
    match surface {
        S::Plane(p) => {
            let n = m.matrix3.inverse().transpose() * p.normal;
            S::Plane(Plane::new(m.transform_point3(p.origin), n.normalize()))
        }
        S::Cylinder(c) => S::Cylinder(c.moved(m, s)),
        S::Cone(c) => S::Cone(c.moved(m)),
        S::Sphere(x) => S::Sphere(x.moved(m, s)),
        S::Torus(t) => S::Torus(t.moved(m, s)),
    }
}

/// `even_scale`, or the nearest even scale (the cube root of the volume
/// change) when `m` is uneven.
pub(crate) fn scale_of(m: &DAffine3) -> (f64, bool) {
    match even_scale(m) {
        Some(s) => (s, true),
        None => (m.matrix3.determinant().abs().cbrt(), false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Surface;
    use prism_math::DQuat;

    fn surfaces() -> [Surface; 3] {
        [
            Surface::Cone(Cone {
                apex: DVec3::new(1.0, 2.0, 3.0),
                axis: DVec3::new(0.0, 0.6, 0.8),
                half_angle: 0.4,
                outward: true,
            }),
            Surface::Sphere(Sphere {
                centre: DVec3::new(-1.0, 0.5, 2.0),
                axis: DVec3::Z,
                radius: 1.5,
                outward: false,
            }),
            Surface::Torus(Torus {
                centre: DVec3::ZERO,
                axis: DVec3::X,
                major: 3.0,
                minor: 0.75,
                outward: true,
            }),
        ]
    }

    #[test]
    fn places_on_a_turned_surface_go_there_and_back() {
        for s in surfaces() {
            let rev = s.revolution().expect("turned");
            for (t, a) in [(0.3, 0.7), (-2.0, 1.2), (3.0, 0.4)] {
                let p = rev.point(t, a);
                assert!(s.distance(p).abs() < 1e-12, "{s:?} at {t} {a}");
                let (t2, a2) = rev.param(p);
                assert!(rev.point(t2, a2).distance(p) < 1e-12);
                // The outward normal, turned to the face's side.
                let n = rev.normal(t, a);
                let face = s.normal_at(p);
                assert!(face.distance(if s.outward() { n } else { -n }) < 1e-9);
            }
        }
    }

    #[test]
    fn projecting_lands_on_the_surface_and_moves_keep_it_exact() {
        let m = DAffine3::from_scale_rotation_translation(
            DVec3::splat(2.0),
            DQuat::from_axis_angle(DVec3::new(1.0, 1.0, 0.0).normalize(), 0.7),
            DVec3::new(3.0, -1.0, 2.0),
        );
        for s in surfaces() {
            let p = s.project(DVec3::new(0.3, 4.0, -1.0));
            assert!(s.distance(p).abs() < 1e-12, "{s:?}");
            let moved = s.transformed(&m).expect("an even scale");
            assert!(moved.distance(m.transform_point3(p)).abs() < 1e-12);
            let uneven = DAffine3::from_scale(DVec3::new(1.0, 2.0, 1.0));
            assert!(s.transformed(&uneven).is_none());
        }
    }

    #[test]
    fn curves_on_them_are_held() {
        let eps = 1e-9;
        let [cone, sphere, torus] = surfaces();
        // A line through the cone's apex, and one that misses it.
        let rev = cone.revolution().unwrap();
        let (a, b) = (rev.point(0.5, 1.0), rev.point(0.5, 2.0));
        assert!(cone.holds(&Curve::Line, a, b, eps));
        assert!(!cone.holds(&Curve::Line, a, rev.point(1.5, 2.0), eps));
        // A circle of latitude on the sphere.
        let rev = sphere.revolution().unwrap();
        let (a, b) = (rev.point(0.0, 0.5), rev.point(2.0, 0.5));
        let Surface::Sphere(sp) = sphere else {
            unreachable!()
        };
        let c = sp.centre + sp.axis * sp.radius * 0.5f64.sin();
        let lat = Curve::Arc {
            centre: c,
            axis: sp.axis,
            radius: sp.radius * 0.5f64.cos(),
        };
        assert!(sphere.holds(&lat, a, b, eps));
        assert!(!sphere.holds(&Curve::Line, a, b, eps));
        // The torus's tube circle at one angle.
        let rev = torus.revolution().unwrap();
        let Surface::Torus(t) = torus else {
            unreachable!()
        };
        let (a, b) = (rev.point(1.0, 0.2), rev.point(1.0, 2.0));
        let e = rev.u * 1.0f64.cos() + rev.v * 1.0f64.sin();
        let tube = Curve::Arc {
            centre: t.centre + e * t.major,
            axis: t.axis.cross(e),
            radius: t.minor,
        };
        assert!(torus.holds(&tube, a, b, eps));
    }
}
