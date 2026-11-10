//! Cones, spheres and tori (Exact Curves 5b): surfaces turned about an
//! axis. Each is a profile in the half plane (distance from the axis ρ,
//! height along it h) turned a full circle. A point on one is named by its
//! angle θ about the axis (counter-clockwise, from a fixed direction `r0`)
//! and its place `s` along the profile, measured as a length. Unrolled this
//! way, a face's loops wind counter-clockwise where it faces out, as a
//! cylinder's do.

use std::f64::consts::{PI, TAU};

use prism_math::{DAffine3, DVec2, DVec3, Plane};
use serde::{Deserialize, Serialize};

use crate::{Curve, even_scale};

/// A cone about the line through `origin` along the unit `axis`: `radius`
/// at the origin, growing by `slope` per unit along the axis (negative:
/// narrowing). The tip is where the radius reaches zero; the cone is the
/// part on the side of the tip where the radius grows.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cone {
    pub origin: DVec3,
    pub axis: DVec3,
    pub radius: f64,
    pub slope: f64,
    /// The face points away from the axis (true) or towards it.
    pub outward: bool,
}

/// A sphere. `axis` only names its poles, for unrolling.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sphere {
    pub centre: DVec3,
    pub axis: DVec3,
    pub radius: f64,
    /// The face points away from the centre (true) or towards it.
    pub outward: bool,
}

/// A ring: the circle of radius `minor` whose centre is `major` from the
/// axis, turned about it. `major` is more than `minor`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Torus {
    pub centre: DVec3,
    pub axis: DVec3,
    pub major: f64,
    pub minor: f64,
    /// The face points away from the tube's middle (true) or towards it.
    pub outward: bool,
}

/// The profile turned: a line `ρ = radius + slope·h`, or the circle about
/// (`rho`, 0) of `radius`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Profile {
    Line { radius: f64, slope: f64 },
    Circle { rho: f64, radius: f64 },
}

/// Any of the three, as a profile about an axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Turned {
    pub origin: DVec3,
    pub axis: DVec3,
    pub profile: Profile,
    pub outward: bool,
}

impl Cone {
    pub fn turned(&self) -> Turned {
        Turned {
            origin: self.origin,
            axis: self.axis,
            profile: Profile::Line {
                radius: self.radius,
                slope: self.slope,
            },
            outward: self.outward,
        }
    }

    /// The tip.
    pub fn apex(&self) -> Option<DVec3> {
        (self.slope != 0.0).then(|| self.origin - self.axis * (self.radius / self.slope))
    }
}

impl Sphere {
    pub fn turned(&self) -> Turned {
        Turned {
            origin: self.centre,
            axis: self.axis,
            profile: Profile::Circle {
                rho: 0.0,
                radius: self.radius,
            },
            outward: self.outward,
        }
    }
}

impl Torus {
    pub fn turned(&self) -> Turned {
        Turned {
            origin: self.centre,
            axis: self.axis,
            profile: Profile::Circle {
                rho: self.major,
                radius: self.minor,
            },
            outward: self.outward,
        }
    }
}

/// Gauss-Legendre nodes and weights on [0, 1].
const GAUSS: [(f64, f64); 5] = [
    (0.046_910_077_030_668_00, 0.118_463_442_528_094_54),
    (0.230_765_344_947_158_45, 0.239_314_335_249_683_23),
    (0.5, 0.284_444_444_444_444_46),
    (0.769_234_655_052_841_6, 0.239_314_335_249_683_23),
    (0.953_089_922_969_332, 0.118_463_442_528_094_54),
];

impl Turned {
    /// The fixed direction θ is measured from, and the one a quarter turn
    /// on.
    pub fn frame(&self) -> (DVec3, DVec3) {
        let r0 = Plane::new(self.origin, self.axis).frame().u;
        (r0, self.axis.cross(r0))
    }

    /// `p` as (ρ, h), with the unit direction out from the axis (None on
    /// the axis).
    fn local(&self, p: DVec3) -> (f64, f64, Option<DVec3>) {
        let q = p - self.origin;
        let h = q.dot(self.axis);
        let r = q - self.axis * h;
        let rho = r.length();
        (rho, h, r.try_normalize())
    }

    /// The place along the profile nearest (ρ, h).
    fn s_of(&self, rho: f64, h: f64) -> f64 {
        match self.profile {
            Profile::Line { radius, slope } => {
                ((rho - radius) * slope + h) / (1.0 + slope * slope).sqrt()
            }
            Profile::Circle { rho: c, radius } => radius * h.atan2(rho - c),
        }
    }

    /// (ρ, h) at `s`.
    pub fn at(&self, s: f64) -> DVec2 {
        match self.profile {
            Profile::Line { radius, slope } => {
                let h = s / (1.0 + slope * slope).sqrt();
                DVec2::new(radius + slope * h, h)
            }
            Profile::Circle { rho, radius } => {
                let a = s / radius;
                DVec2::new(rho + radius * a.cos(), radius * a.sin())
            }
        }
    }

    /// The profile's outward normal at `s`, as (ρ, h).
    fn normal2(&self, s: f64) -> DVec2 {
        match self.profile {
            Profile::Line { slope, .. } => DVec2::new(1.0, -slope) / (1.0 + slope * slope).sqrt(),
            Profile::Circle { radius, .. } => {
                let a = s / radius;
                DVec2::new(a.cos(), a.sin())
            }
        }
    }

    /// The length one turn of `s` covers, if it comes round (a circle).
    pub fn s_period(&self) -> Option<f64> {
        match self.profile {
            Profile::Line { .. } => None,
            Profile::Circle { radius, .. } => Some(TAU * radius),
        }
    }

    /// Where the surface meets its axis: the place `s` at the low end and
    /// at the high end, if it does there (a sphere's poles, a cone's tip).
    pub fn poles(&self) -> [Option<f64>; 2] {
        match self.profile {
            Profile::Line { radius, slope } => {
                if slope == 0.0 {
                    return [None, None];
                }
                let s = -radius / slope * (1.0 + slope * slope).sqrt();
                if slope > 0.0 {
                    [Some(s), None]
                } else {
                    [None, Some(s)]
                }
            }
            Profile::Circle { rho, radius } => {
                if rho == 0.0 {
                    [Some(-radius * PI / 2.0), Some(radius * PI / 2.0)]
                } else {
                    [None, None]
                }
            }
        }
    }

    /// A length that says how big it is.
    pub fn size(&self) -> f64 {
        match self.profile {
            Profile::Line { radius, .. } => radius.abs(),
            Profile::Circle { rho, radius } => rho + radius,
        }
    }

    /// How far `p` is from the surface, out along the profile's normal
    /// (not signed by the face's side).
    pub fn distance(&self, p: DVec3) -> f64 {
        let (rho, h, _) = self.local(p);
        match self.profile {
            Profile::Line { radius, slope } => {
                (rho - radius - slope * h) / (1.0 + slope * slope).sqrt()
            }
            Profile::Circle { rho: c, radius } => (rho - c).hypot(h) - radius,
        }
    }

    /// `p` (near the surface) as (θ in (-π, π], s).
    pub fn param(&self, p: DVec3) -> DVec2 {
        let (r0, w) = self.frame();
        let (rho, h, _) = self.local(p);
        let q = p - self.origin;
        DVec2::new(q.dot(w).atan2(q.dot(r0)), self.s_of(rho, h))
    }

    /// The point at (θ, s).
    pub fn point(&self, t: DVec2) -> DVec3 {
        let (r0, w) = self.frame();
        let out = r0 * t.x.cos() + w * t.x.sin();
        let q = self.at(t.y);
        self.origin + out * q.x + self.axis * q.y
    }

    /// The face's normal at (θ, s).
    pub fn normal(&self, t: DVec2) -> DVec3 {
        let (r0, w) = self.frame();
        let out = r0 * t.x.cos() + w * t.x.sin();
        let n = self.normal2(t.y);
        let v = out * n.x + self.axis * n.y;
        if self.outward { v } else { -v }
    }

    /// The face's normal at `p`, a point on or near it.
    pub fn normal_at(&self, p: DVec3) -> DVec3 {
        let (rho, h, out) = self.local(p);
        let out = out.unwrap_or_else(|| self.frame().0);
        let n = self.normal2(self.s_of(rho, h));
        let v = out * n.x + self.axis * n.y;
        if self.outward { v } else { -v }
    }

    /// The nearest point of the surface to `p`.
    pub fn closest(&self, p: DVec3) -> DVec3 {
        let (rho, h, out) = self.local(p);
        let out = out.unwrap_or_else(|| self.frame().0);
        let q = match self.profile {
            Profile::Line { .. } => self.at(self.s_of(rho, h)),
            Profile::Circle { rho: c, radius } => {
                let d = DVec2::new(rho - c, h);
                let d = d.try_normalize().unwrap_or(DVec2::X);
                DVec2::new(c, 0.0) + d * radius
            }
        };
        self.origin + out * q.x + self.axis * q.y
    }

    /// Whether `curve` from `a` to `b` lies on the surface, within `eps`:
    /// it does when enough of its points do (a line meets one of these in
    /// at most four points unless it lies on it, a circle in at most
    /// eight).
    pub fn holds(&self, curve: &Curve, a: DVec3, b: DVec3, eps: f64) -> bool {
        let on = |p: DVec3| self.distance(p).abs() <= eps;
        if !on(a) || !on(b) {
            return false;
        }
        match *curve {
            Curve::Line => (1..4).all(|i| on(a.lerp(b, i as f64 / 4.0))),
            Curve::Arc {
                centre,
                axis,
                radius,
            } => {
                let u = Plane::new(centre, axis).frame().u;
                let v = axis.cross(u);
                (0..9).all(|i| {
                    let t = TAU * i as f64 / 9.0;
                    on(centre + (u * t.cos() + v * t.sin()) * radius)
                })
            }
        }
    }

    /// Where the ray from `o` along `d` crosses the surface between `t0`
    /// and `t1` (in units of `d`), nearest first.
    pub fn ray_hits(&self, o: DVec3, d: DVec3, t0: f64, t1: f64) -> Vec<f64> {
        const STEPS: usize = 256;
        let f = |t: f64| self.distance(o + d * t);
        let mut out = Vec::new();
        let (mut a, mut fa) = (t0, f(t0));
        for i in 1..=STEPS {
            let b = t0 + (t1 - t0) * i as f64 / STEPS as f64;
            let fb = f(b);
            if fa == 0.0 {
                out.push(a);
            } else if fa * fb < 0.0 {
                let (mut lo, mut hi, mut flo) = (a, b, fa);
                for _ in 0..80 {
                    let m = 0.5 * (lo + hi);
                    let fm = f(m);
                    if (fm < 0.0) == (flo < 0.0) {
                        lo = m;
                        flo = fm;
                    } else {
                        hi = m;
                    }
                }
                out.push(0.5 * (lo + hi));
            }
            (a, fa) = (b, fb);
        }
        out
    }

    /// For a loop unrolled as (θ, s) points, its share of the face's area
    /// and of the volume its closed shell encloses (a third of the integral
    /// of position · normal). Both are signed: positive for a loop that
    /// winds counter-clockwise about the face's own normal. By Green's
    /// theorem on the unrolled plane, exact along lines of constant θ or s.
    pub fn loop_integrals(&self, pts: &[DVec2]) -> (f64, f64) {
        let (r0, w) = self.frame();
        let o = self.origin;
        // Primitives in s of ρ, of (position · normal about the origin)·ρ,
        // and of each part of the normal times ρ.
        let prims = |s: f64| -> [f64; 4] {
            match self.profile {
                Profile::Line { radius, slope } => {
                    let c = (1.0 + slope * slope).sqrt();
                    let g = radius * s + slope * s * s / (2.0 * c);
                    [g, radius / c * g, g / c, -slope * g / c]
                }
                Profile::Circle { rho, radius: a } => {
                    let p = s / a;
                    let (sn, cs) = p.sin_cos();
                    let half = p / 2.0 + (2.0 * p).sin() / 4.0;
                    [
                        a * (rho * p + a * sn),
                        a * ((rho * rho + a * a) * sn + a * rho * half + a * rho * p),
                        a * (rho * sn + a * half),
                        a * (-rho * cs + a * sn * sn / 2.0),
                    ]
                }
            }
        };
        let (mut area, mut q, mut nr, mut nh) = (0.0, 0.0, DVec3::ZERO, 0.0);
        let n = pts.len();
        for i in 0..n {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            if b.x == a.x {
                continue;
            }
            // In short pieces, so the rule is exact to rounding round a
            // whole turn too (the run across a pole is one segment).
            let pieces = ((b.x - a.x).abs() / (TAU / 256.0)).ceil().max(1.0);
            let dt = (b.x - a.x) / pieces;
            for (k, (x, wt)) in (0..pieces as usize).flat_map(|k| GAUSS.map(|g| (k, g))) {
                let p = a.lerp(b, (k as f64 + x) / pieces);
                let [g, qq, nrr, nhh] = prims(p.y);
                let out = r0 * p.x.cos() + w * p.x.sin();
                area -= g * dt * wt;
                q -= qq * dt * wt;
                nr -= out * (nrr * dt * wt);
                nh -= nhh * dt * wt;
            }
        }
        let volume = (q + o.dot(nr + self.axis * nh)) / 3.0;
        if self.outward {
            (area, volume)
        } else {
            // Facing in, the normal is the other way; so is the winding.
            (-area, volume)
        }
    }

    /// The surface after `m`, if it keeps its shape (moves, turns, mirrors
    /// and even scales).
    pub fn transformed(&self, m: &DAffine3) -> Option<(DVec3, DVec3, f64, bool)> {
        let s = even_scale(m)?;
        Some((
            m.transform_point3(self.origin),
            (m.matrix3 * self.axis).normalize(),
            s,
            self.outward != (m.matrix3.determinant() < 0.0),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The unrolled rectangle θ in [0, 2π], s in [s0, s1], counter-clockwise.
    fn band(s0: f64, s1: f64) -> Vec<DVec2> {
        vec![
            DVec2::new(0.0, s0),
            DVec2::new(TAU, s0),
            DVec2::new(TAU, s1),
            DVec2::new(0.0, s1),
        ]
    }

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9 * b.abs().max(1.0), "{a} vs {b}");
    }

    #[test]
    fn a_whole_sphere_has_the_exact_area_and_volume() {
        let r = 1.7;
        let c = DVec3::new(3.0, -2.0, 5.0);
        let s = Sphere {
            centre: c,
            axis: DVec3::new(1.0, 2.0, 2.0) / 3.0,
            radius: r,
            outward: true,
        }
        .turned();
        let [lo, hi] = s.poles();
        let (a, v) = s.loop_integrals(&band(lo.unwrap(), hi.unwrap()));
        close(a, 4.0 * PI * r * r);
        close(v, 4.0 / 3.0 * PI * r * r * r);
        // Facing in, the loop runs the other way: the same area, and the
        // volume of the space round it is negative.
        let inward = Turned {
            outward: false,
            ..s
        };
        let mut l = band(lo.unwrap(), hi.unwrap());
        l.reverse();
        let (a, v) = inward.loop_integrals(&l);
        close(a, 4.0 * PI * r * r);
        close(v, -4.0 / 3.0 * PI * r * r * r);
    }

    #[test]
    fn a_cone_and_a_torus_have_the_exact_area_and_volume() {
        // A cone from radius 2 at the origin to its tip 3 up.
        let cone = Cone {
            origin: DVec3::new(1.0, 1.0, 1.0),
            axis: DVec3::Z,
            radius: 2.0,
            slope: -2.0 / 3.0,
            outward: true,
        }
        .turned();
        let tip = cone.poles()[1].expect("a tip");
        let (a, v) = cone.loop_integrals(&band(0.0, tip));
        // Side area π r l; the side's share of the volume (with the base
        // disc at z = 1 and the origin off the axis).
        close(a, PI * 2.0 * 13f64.sqrt());
        let base_share = -PI * 4.0 * 1.0 / 3.0;
        close(v + base_share, PI * 4.0 * 3.0 / 3.0);
        let t = Torus {
            centre: DVec3::new(-1.0, 4.0, 2.0),
            axis: DVec3::X,
            major: 3.0,
            minor: 1.0,
            outward: true,
        }
        .turned();
        let (a, v) = t.loop_integrals(&band(-PI, PI));
        close(a, 4.0 * PI * PI * 3.0);
        close(v, 2.0 * PI * PI * 3.0);
    }

    #[test]
    fn points_go_to_and_from_their_place() {
        for t in [
            Sphere {
                centre: DVec3::X,
                axis: DVec3::Y,
                radius: 2.0,
                outward: true,
            }
            .turned(),
            Cone {
                origin: DVec3::ZERO,
                axis: DVec3::Z,
                radius: 1.0,
                slope: 0.5,
                outward: false,
            }
            .turned(),
            Torus {
                centre: DVec3::ZERO,
                axis: DVec3::Z,
                major: 2.0,
                minor: 0.5,
                outward: true,
            }
            .turned(),
        ] {
            let q = DVec2::new(0.7, 0.3);
            let p = t.point(q);
            assert!(t.distance(p).abs() < 1e-12);
            assert!(t.param(p).distance(q) < 1e-12);
            let n = t.normal(q);
            assert!(n.distance(t.normal_at(p)) < 1e-12);
            // Out along the normal: the distance grows (or shrinks, facing
            // in), and the closest point comes back.
            let off = p + n * 0.01;
            let d = if t.outward { 0.01 } else { -0.01 };
            assert!((t.distance(off) - d).abs() < 1e-9);
            assert!(t.closest(off).distance(p) < 1e-9);
            // A ray straight in meets it where it started.
            let hits = t.ray_hits(p + n * 0.5, -n, 0.0, 0.6);
            assert!(hits.iter().any(|h| (h - 0.5).abs() < 1e-9), "{hits:?}");
        }
    }

    #[test]
    fn circles_and_lines_on_the_surfaces() {
        let s = Sphere {
            centre: DVec3::ZERO,
            axis: DVec3::Z,
            radius: 1.0,
            outward: true,
        }
        .turned();
        let tilted = DVec3::new(1.0, 0.0, 1.0).normalize();
        let arc = Curve::Arc {
            centre: tilted * 0.6,
            axis: tilted,
            radius: 0.8,
        };
        let p = tilted * 0.6 + DVec3::Y * 0.8;
        assert!(s.holds(&arc, p, p, 1e-9), "any circle of a sphere");
        assert!(!s.holds(&Curve::Line, DVec3::X, DVec3::Y, 1e-9));
        let c = Cone {
            origin: DVec3::ZERO,
            axis: DVec3::Z,
            radius: 1.0,
            slope: -1.0,
            outward: true,
        }
        .turned();
        assert!(c.holds(&Curve::Line, DVec3::X, DVec3::Z, 1e-9), "to the tip");
        assert!(!c.holds(&Curve::Line, DVec3::X, DVec3::Y, 1e-9), "a chord");
    }
}
