//! The NURBS kernel (PLAN 2.3: own the data model, borrow the hard maths).
//!
//! Prism's curves and surfaces are its own (`prism-geom`). What is hard to
//! own goes through the [`Kernel`] trait, and the one implementation is
//! [`Arris`] (the kernel picked by the NURBS Spike, DECISIONS #9). arris is
//! pre-1.0, so its version is pinned and every call to it is in this crate
//! (here, and the bridge for solids in [`solid`]): an upgrade or a change
//! of kernel touches nothing else.
//!
//! Pure and deterministic, as the core crates are: arris reads no clock
//! and no random source.

use prism_geom::{Nurbs, NurbsSurface};
use prism_math::DVec3;

pub mod solid;

/// Said when the kernel cannot make the surface.
pub const NO_SURFACE: &str = "Refused: no surface could be made from these curves.";
/// Said when a curve to turn lies on the axis.
pub const ON_AXIS: &str = "Refused: the curve lies on the axis it would turn about.";

/// The hard NURBS operations.
pub trait Kernel {
    /// The surface through `sections` (already alike: `Nurbs::alike`), in
    /// order. Its u runs along the sections and its v across them, from 0
    /// at the first to 1 at the last, at `params` (one for each section,
    /// rising). Smooth across when there are three or more.
    fn skin(&self, sections: &[Nurbs], params: &[f64]) -> Result<NurbsSurface, &'static str>;

    /// `curve` turned about the line through `origin` along `axis` by
    /// `angle` radians (a full turn at most). Its u is the angle, its v the
    /// curve's own parameter.
    fn revolve(
        &self,
        curve: &Nurbs,
        origin: DVec3,
        axis: DVec3,
        angle: f64,
    ) -> Result<NurbsSurface, &'static str>;
}

/// The kernel Prism uses.
pub fn kernel() -> &'static dyn Kernel {
    &Arris
}

/// arris, behind the trait.
pub struct Arris;

pub(crate) fn point(p: DVec3) -> arris_math::Point3 {
    arris_math::Point3::new(p.x, p.y, p.z)
}

pub(crate) fn back(p: &arris_math::Point3) -> DVec3 {
    DVec3::new(p.x, p.y, p.z)
}

fn curve(c: &Nurbs) -> Option<arris_geom::NurbsCurve> {
    arris_geom::NurbsCurve::new(
        c.degree(),
        c.knots().to_vec(),
        c.points().iter().map(|p| point(*p)).collect(),
        c.weights().to_vec(),
    )
    .ok()
}

fn surface(s: &arris_geom::NurbsSurface, periodic: [bool; 2]) -> Option<NurbsSurface> {
    let [ku, kv] = s.knots();
    NurbsSurface::new(
        s.degree(),
        [ku.to_vec(), kv.to_vec()],
        s.counts(),
        s.control_points().iter().map(back).collect(),
        s.weights().to_vec(),
        periodic,
    )
}

impl Kernel for Arris {
    fn skin(&self, sections: &[Nurbs], params: &[f64]) -> Result<NurbsSurface, &'static str> {
        let first = sections.first().ok_or(NO_SURFACE)?;
        let curves: Vec<arris_geom::NurbsCurve> = sections
            .iter()
            .map(curve)
            .collect::<Option<_>>()
            .ok_or(NO_SURFACE)?;
        let degree = 3.min(sections.len().saturating_sub(1)).max(1);
        let s =
            arris_geom::interpolate_skin(&curves, params, degree, false).map_err(|_| NO_SURFACE)?;
        surface(&s, [first.is_periodic(), false]).ok_or(NO_SURFACE)
    }

    fn revolve(
        &self,
        c: &Nurbs,
        origin: DVec3,
        axis: DVec3,
        angle: f64,
    ) -> Result<NurbsSurface, &'static str> {
        let n = axis.try_normalize().ok_or(ON_AXIS)?;
        let range = arris_math::Interval::new(0.0, angle.min(std::f64::consts::TAU))
            .map_err(|_| NO_SURFACE)?;
        let s = arris_geom::NurbsSurface::revolution(
            &curve(c).ok_or(NO_SURFACE)?,
            point(origin),
            arris_math::UnitVec3::new_normalize(arris_math::Vec3::new(n.x, n.y, n.z)),
            range,
        )
        .map_err(|_| ON_AXIS)?;
        surface(&s, [false, c.is_periodic()]).ok_or(NO_SURFACE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wave(z: f64, bump: f64) -> Nurbs {
        Nurbs::open(
            &[
                DVec3::new(0.0, 0.0, z),
                DVec3::new(1.0, bump, z),
                DVec3::new(3.0, -bump, z),
                DVec3::new(4.0, 0.0, z),
            ],
            3,
        )
        .unwrap()
    }

    #[test]
    fn a_skin_passes_through_its_sections() {
        let sections = [wave(0.0, 2.0), wave(1.0, 0.5), wave(3.0, 1.0)];
        let s = kernel().skin(&sections, &[0.0, 0.4, 1.0]).unwrap();
        assert_eq!(s.degree(), [3, 2]);
        for (c, v) in sections.iter().zip([0.0, 0.4, 1.0]) {
            for i in 0..=10 {
                let u = i as f64 / 10.0;
                assert!(s.eval(u, v).distance(c.eval(u)) < 1e-12);
            }
        }
    }

    #[test]
    fn closed_sections_make_a_tube_with_no_seam() {
        let ring = |z: f64, r: f64| {
            let pts: Vec<DVec3> = (0..6)
                .map(|i| {
                    let a = i as f64 / 6.0 * std::f64::consts::TAU;
                    DVec3::new(r * a.cos(), r * a.sin(), z)
                })
                .collect();
            Nurbs::closed(&pts, 3).unwrap()
        };
        let sections = [ring(0.0, 1.0), ring(1.0, 2.0)];
        let s = kernel().skin(&sections, &[0.0, 1.0]).unwrap();
        assert_eq!(s.periodic(), [true, false]);
        assert_eq!(s.cage(), [6, 2]);
        for i in 0..=12 {
            let u = i as f64 / 12.0;
            assert!(s.eval(u, 1.0).distance(sections[1].eval(u)) < 1e-12);
            let mid = s.eval(u, 0.5);
            assert!(mid.distance((sections[0].eval(u) + sections[1].eval(u)) / 2.0) < 1e-12);
        }
    }

    #[test]
    fn a_line_turned_about_an_axis_is_a_cone() {
        let line = Nurbs::line(DVec3::new(1.0, 0.0, 0.0), DVec3::new(2.0, 0.0, 3.0)).unwrap();
        let s = kernel()
            .revolve(&line, DVec3::ZERO, DVec3::Z, std::f64::consts::TAU)
            .unwrap();
        let [(u0, u1), (v0, v1)] = s.domain();
        for i in 0..=8 {
            for j in 0..=4 {
                let p = s.eval(
                    u0 + (u1 - u0) * i as f64 / 8.0,
                    v0 + (v1 - v0) * j as f64 / 4.0,
                );
                let r = p.x.hypot(p.y);
                assert!((r - (1.0 + p.z / 3.0)).abs() < 1e-12, "{p}");
            }
        }
        assert!(s.eval(u0, v0).distance(s.eval(u1, v0)) < 1e-12);
        let on_axis = Nurbs::line(DVec3::ZERO, DVec3::Z).unwrap();
        assert_eq!(
            kernel().revolve(&on_axis, DVec3::ZERO, DVec3::Z, 1.0).err(),
            Some(ON_AXIS)
        );
    }
}
