//! The cases in arris (MIT OR Apache-2.0).

use std::f64::consts::{PI, TAU};

use arris::Control;
use arris::check::{Level, check};
use arris::geom::profile::{Profile, ProfileLoop, ProfileSegment};
use arris::math::nalgebra::{Unit, UnitQuaternion, Vector3};
use arris::math::{Axis, Frame, Isometry, Point2, Point3, Vec3};
use arris::ops::measure::mass_properties;
use arris::ops::{
    common, cut, fillet, fuse, primitive_box, primitive_cylinder, revolve, transform,
};
use arris::topo::{Body, Model};

use crate::Outcome;

const C: &Control<'static> = &Control::NONE;

type R<T> = Result<T, String>;

fn e<E: std::fmt::Debug>(x: E) -> String {
    format!("{x:?}")
}

fn cube(m: &mut Model, lo: [f64; 3], hi: [f64; 3]) -> R<Body> {
    Ok(primitive_box(m, Point3::from(lo), Point3::from(hi), C)
        .map_err(e)?
        .0)
}

/// A cylinder of radius `r` from `from` along `dir` for `len`.
fn cyl(m: &mut Model, from: [f64; 3], dir: [f64; 3], r: f64, len: f64) -> R<Body> {
    let axis = Axis::new(Point3::from(from), Vec3::from(dir)).map_err(e)?;
    Ok(primitive_cylinder(m, axis, r, len, C).map_err(e)?.0)
}

/// A ball by turning a half disc about z.
fn ball(m: &mut Model, centre: [f64; 3], r: f64) -> R<Body> {
    let p = Point2::new;
    let profile = Profile {
        plane: Frame::new(Point3::from(centre), -Vec3::y(), Vec3::x()).map_err(e)?,
        outer: ProfileLoop::Path {
            start: p(0.0, -r),
            segments: vec![
                ProfileSegment::ArcTo {
                    to: p(0.0, r),
                    via: p(r, 0.0),
                },
                ProfileSegment::LineTo(p(0.0, -r)),
            ],
        },
        holes: Vec::new(),
    };
    Ok(
        revolve(m, &profile, Axis::z_at(Point3::from(centre)), TAU, C)
            .map_err(e)?
            .0,
    )
}

fn torus(m: &mut Model, big: f64, small: f64) -> R<Body> {
    let profile = Profile {
        plane: Frame::new(Point3::origin(), -Vec3::y(), Vec3::x()).map_err(e)?,
        outer: ProfileLoop::Circle {
            center: Point2::new(big, 0.0),
            radius: small,
        },
        holes: Vec::new(),
    };
    Ok(revolve(m, &profile, Axis::z_at(Point3::origin()), TAU, C)
        .map_err(e)?
        .0)
}

/// A valid solid's exact volume.
fn solid(m: &Model, b: Body) -> R<Outcome> {
    let report = check(m, b, Level::Full);
    if !report.is_empty() {
        return Ok(Outcome::Refused(format!("invalid result: {report}")));
    }
    Ok(Outcome::Solid(mass_properties(m, b, C).map_err(e)?.volume))
}

fn case(k: usize) -> R<Outcome> {
    let mut m = Model::default();
    let m = &mut m;
    let b = match k {
        0 => {
            let a = cube(m, [-1.0; 3], [1.0; 3])?;
            let t = cube(m, [0.0; 3], [2.0; 3])?;
            cut(m, a, t, C).map_err(e)?.0
        }
        1 => {
            let a = cube(m, [-1.0; 3], [1.0; 3])?;
            let t = cyl(m, [0.0, 0.0, -2.0], [0.0, 0.0, 1.0], 0.5, 4.0)?;
            cut(m, a, t, C).map_err(e)?.0
        }
        2 => {
            let a = cube(m, [-1.0; 3], [1.0; 3])?;
            let (s, c) = (PI / 6.0).sin_cos();
            let t = cyl(m, [-4.0 * s, 0.0, -4.0 * c], [s, 0.0, c], 0.4, 8.0)?;
            cut(m, a, t, C).map_err(e)?.0
        }
        3 | 5 => {
            let a = cyl(m, [-3.0, 0.0, 0.0], [1.0, 0.0, 0.0], 1.0, 6.0)?;
            let b = cyl(m, [0.0, -3.0, 0.0], [0.0, 1.0, 0.0], 1.0, 6.0)?;
            if k == 3 {
                common(m, a, b, C).map_err(e)?.0
            } else {
                fuse(m, a, b, C).map_err(e)?.0
            }
        }
        4 => {
            let a = cyl(m, [-3.0, 0.0, 0.0], [1.0, 0.0, 0.0], 1.0, 6.0)?;
            let b = cyl(m, [0.0, -3.0, 0.0], [0.0, 1.0, 0.0], 0.6, 6.0)?;
            common(m, a, b, C).map_err(e)?.0
        }
        6 => {
            let a = ball(m, [0.0; 3], 1.0)?;
            let t = cube(m, [-2.0, -2.0, 0.0], [2.0, 2.0, 2.0])?;
            common(m, a, t, C).map_err(e)?.0
        }
        7 => {
            let a = ball(m, [0.0; 3], 1.0)?;
            let b = ball(m, [1.0, 0.0, 0.0], 1.0)?;
            fuse(m, a, b, C).map_err(e)?.0
        }
        8 => {
            let a = ball(m, [0.0; 3], 1.0)?;
            let t = cyl(m, [0.3, 0.0, -2.0], [0.0, 0.0, 1.0], 0.4, 4.0)?;
            cut(m, a, t, C).map_err(e)?.0
        }
        9 => {
            let a = torus(m, 2.0, 0.5)?;
            let t = cube(m, [-3.0, 0.0, -1.0], [3.0, 3.0, 1.0])?;
            cut(m, a, t, C).map_err(e)?.0
        }
        10 | 11 => {
            let a = cube(m, [-1.0; 3], [1.0; 3])?;
            let edges = m.edges(a).map_err(e)?;
            let pick: Vec<_> = if k == 10 {
                edges.into_iter().take(1).collect()
            } else {
                edges
            };
            fillet(m, a, &pick, 0.3, C).map_err(e)?.0
        }
        _ => return Err("no such case".into()),
    };
    solid(m, b)
}

/// A smooth loft (NURBS faces) through three squares of different sizes,
/// each turned a little more, from z = −1 to 1, moved by `at`.
fn twisted(m: &mut Model, at: [f64; 3], turn: f64) -> R<Body> {
    let section = |z: f64, half: f64, a: f64| -> R<Profile> {
        let (s, c) = a.sin_cos();
        let p = |x: f64, y: f64| Point2::new(c * x - s * y, s * x + c * y);
        Ok(Profile {
            plane: Frame::new(Point3::new(at[0], at[1], at[2] + z), Vec3::z(), Vec3::x())
                .map_err(e)?,
            outer: ProfileLoop::Path {
                start: p(-half, -half),
                segments: vec![
                    ProfileSegment::LineTo(p(half, -half)),
                    ProfileSegment::LineTo(p(half, half)),
                    ProfileSegment::LineTo(p(-half, half)),
                    ProfileSegment::LineTo(p(-half, -half)),
                ],
            },
            holes: Vec::new(),
        })
    };
    let sections = [
        section(-1.0, 1.0, 0.0)?,
        section(0.0, 0.6, turn)?,
        section(1.0, 0.9, 2.0 * turn)?,
    ];
    Ok(
        arris::ops::loft(m, &sections, arris::ops::LoftKind::Smooth, false, C)
            .map_err(e)?
            .0,
    )
}

fn vol(m: &Model, b: Body) -> R<f64> {
    match solid(m, b)? {
        Outcome::Solid(v) => Ok(v),
        Outcome::Refused(r) => Err(r),
    }
}

/// Freeform (NURBS) faces in booleans, checked for consistency: a loft A
/// and a box B give V(A) = V(A∩B) + V(A−B); two lofts give
/// V(A∪B) = V(A) + V(B) − V(A∩B). Returns the worst relative error.
pub fn nurbs_consistency() -> R<(f64, f64)> {
    let mut m = Model::default();
    let m = &mut m;
    let a = twisted(m, [0.0; 3], 0.5)?;
    let b = cube(m, [0.1, -2.0, -0.4], [2.0, 2.0, 0.7])?;
    let va = vol(m, a)?;
    let and = common(m, a, b, C).map_err(e)?.0;
    let minus = cut(m, a, b, C).map_err(e)?.0;
    let box_err = ((vol(m, and)? + vol(m, minus)?) - va).abs() / va;
    let c = twisted(m, [0.7, 0.3, 0.2], -0.4)?;
    let vc = vol(m, c)?;
    let both = common(m, a, c, C).map_err(e)?.0;
    let either = fuse(m, a, c, C).map_err(e)?.0;
    let lofts_err = (vol(m, either)? - (va + vc - vol(m, both)?)).abs() / (va + vc);
    Ok((box_err, lofts_err))
}

pub fn run(k: usize) -> Outcome {
    case(k).unwrap_or_else(Outcome::Refused)
}

/// A 2 m box minus a cylinder tilted `tilt` from z, turned `turn` about z,
/// moved `off` in x and y.
pub fn random_cut(tilt: f64, turn: f64, off: [f64; 2], radius: f64) -> Outcome {
    let go = || -> R<Outcome> {
        let mut m = Model::default();
        let m = &mut m;
        let a = cube(m, [-1.0; 3], [1.0; 3])?;
        let t = cyl(m, [0.0, 0.0, -4.0], [0.0, 0.0, 1.0], radius, 8.0)?;
        let q = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), turn)
            * UnitQuaternion::from_axis_angle(&Unit::new_normalize(Vector3::x()), tilt);
        let t = transform(m, t, &Isometry::new(q, Vec3::new(off[0], off[1], 0.0)), C)
            .map_err(e)?
            .0;
        let b = cut(m, a, t, C).map_err(e)?.0;
        solid(m, b)
    };
    go().unwrap_or_else(Outcome::Refused)
}
