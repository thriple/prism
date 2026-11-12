//! The cases in monstertruck (Apache-2.0).

use std::f64::consts::PI;

use monstertruck::meshing::prelude::*;
use monstertruck::modeling::*;

use crate::Outcome;

const TOL: f64 = 0.01;

type R<T> = std::result::Result<T, String>;

fn e<E: std::fmt::Debug>(x: E) -> String {
    format!("{x:?}")
}

/// Volume from a fine tessellation.
fn volume(s: &Solid) -> f64 {
    s.triangulation(0.002).to_polygon().volume()
}

/// Outward, whatever way the building turned it.
fn outward(mut s: Solid) -> Solid {
    if volume(&s) < 0.0 {
        s.not();
    }
    s
}

fn cube(lo: [f64; 3], hi: [f64; 3]) -> Solid {
    let v = builder::vertex(Point3::from(lo));
    let e = builder::extrude(&v, Vector3::unit_x() * (hi[0] - lo[0]));
    let f = builder::extrude(&e, Vector3::unit_y() * (hi[1] - lo[1]));
    outward(builder::extrude(&f, Vector3::unit_z() * (hi[2] - lo[2])))
}

/// A cylinder along z of radius `r` from z0 for `len`, then turned so z
/// becomes `dir` about the origin and moved to `at`.
fn cyl(at: [f64; 3], dir: [f64; 3], r: f64, z0: f64, len: f64) -> R<Solid> {
    let v = builder::vertex(Point3::new(r, 0.0, z0));
    let w: Wire = builder::revolve(
        &v,
        Point3::new(0.0, 0.0, z0),
        Vector3::unit_z(),
        builder::SweepAngle::Closed,
        4,
    );
    let f: Face = builder::try_attach_plane(&[w]).map_err(e)?;
    let mut s: Solid = builder::extrude(&f, Vector3::unit_z() * len);
    let d = Vector3::from(dir).normalize();
    let axis = Vector3::unit_z().cross(d);
    if axis.magnitude() > 1e-12 {
        let angle = Vector3::unit_z().dot(d).clamp(-1.0, 1.0).acos();
        s = builder::rotated(&s, Point3::origin(), axis.normalize(), Rad(angle));
    }
    Ok(outward(builder::translated(&s, Vector3::from(at))))
}

fn ball(centre: [f64; 3], r: f64) -> R<Solid> {
    let v0 = builder::vertex(Point3::new(0.0, 0.0, -r));
    let v1 = builder::vertex(Point3::new(0.0, 0.0, r));
    let arc = builder::circle_arc(&v0, &v1, Point3::new(r, 0.0, 0.0));
    let line = builder::line(&v1, &v0);
    let f: Face = builder::try_attach_plane(&[wire![arc, line]]).map_err(e)?;
    let s: Solid = builder::revolve(
        &f,
        Point3::origin(),
        Vector3::unit_z(),
        builder::SweepAngle::Closed,
        4,
    );
    Ok(outward(builder::translated(&s, Vector3::from(centre))))
}

fn torus(big: f64, small: f64) -> R<Solid> {
    let w: Wire = primitive::circle(
        Point3::new(big + small, 0.0, 0.0),
        Point3::new(big, 0.0, 0.0),
        Vector3::unit_y(),
        4,
    );
    let f: Face = builder::try_attach_plane(&[w]).map_err(e)?;
    Ok(outward(builder::revolve(
        &f,
        Point3::origin(),
        Vector3::unit_z(),
        builder::SweepAngle::Closed,
        4,
    )))
}

fn solid(s: &Solid) -> Outcome {
    if s.boundaries()
        .iter()
        .any(|b| b.shell_condition() != ShellCondition::Closed)
    {
        return Outcome::Refused("result is not closed".into());
    }
    Outcome::Solid(volume(s))
}

fn case(k: usize) -> R<Outcome> {
    use monstertruck::solid::{and, difference, or};
    let s = match k {
        0 => difference(&cube([-1.0; 3], [1.0; 3]), &cube([0.0; 3], [2.0; 3]), TOL).map_err(e)?,
        1 => {
            let t = cyl([0.0; 3], [0.0, 0.0, 1.0], 0.5, -2.0, 4.0)?;
            difference(&cube([-1.0; 3], [1.0; 3]), &t, TOL).map_err(e)?
        }
        2 => {
            let (s, c) = (PI / 6.0).sin_cos();
            let t = cyl([0.0; 3], [s, 0.0, c], 0.4, -4.0, 8.0)?;
            difference(&cube([-1.0; 3], [1.0; 3]), &t, TOL).map_err(e)?
        }
        3 | 5 => {
            let a = cyl([0.0; 3], [1.0, 0.0, 0.0], 1.0, -3.0, 6.0)?;
            let b = cyl([0.0; 3], [0.0, 1.0, 0.0], 1.0, -3.0, 6.0)?;
            if k == 3 {
                and(&a, &b, TOL).map_err(e)?
            } else {
                or(&a, &b, TOL).map_err(e)?
            }
        }
        4 => {
            let a = cyl([0.0; 3], [1.0, 0.0, 0.0], 1.0, -3.0, 6.0)?;
            let b = cyl([0.0; 3], [0.0, 1.0, 0.0], 0.6, -3.0, 6.0)?;
            and(&a, &b, TOL).map_err(e)?
        }
        6 => and(
            &ball([0.0; 3], 1.0)?,
            &cube([-2.0, -2.0, 0.0], [2.0, 2.0, 2.0]),
            TOL,
        )
        .map_err(e)?,
        7 => or(&ball([0.0; 3], 1.0)?, &ball([1.0, 0.0, 0.0], 1.0)?, TOL).map_err(e)?,
        8 => {
            let t = cyl([0.3, 0.0, 0.0], [0.0, 0.0, 1.0], 0.4, -2.0, 4.0)?;
            difference(&ball([0.0; 3], 1.0)?, &t, TOL).map_err(e)?
        }
        9 => difference(
            &torus(2.0, 0.5)?,
            &cube([-3.0, 0.0, -1.0], [3.0, 3.0, 1.0]),
            TOL,
        )
        .map_err(e)?,
        10 | 11 => {
            let c = cube([-1.0; 3], [1.0; 3]);
            let mut shell = c.boundaries()[0].clone();
            let mut edges: Vec<Edge> = Vec::new();
            for ed in shell.edge_iter() {
                if !edges.iter().any(|x| x.id() == ed.id()) {
                    edges.push(ed);
                }
            }
            let pick = if k == 10 { &edges[..1] } else { &edges[..] };
            fillet_edges(&mut shell, pick, Some(&FilletOptions::constant(0.3))).map_err(e)?;
            Solid::try_new(vec![shell]).map_err(e)?
        }
        _ => return Err("no such case".into()),
    };
    Ok(solid(&s))
}

pub fn run(k: usize) -> Outcome {
    case(k).unwrap_or_else(Outcome::Refused)
}

pub fn random_cut(tilt: f64, turn: f64, off: [f64; 2], radius: f64) -> Outcome {
    let go = || -> R<Outcome> {
        let t = cyl([0.0; 3], [0.0, 0.0, 1.0], radius, -4.0, 8.0)?;
        let t = builder::rotated(&t, Point3::origin(), Vector3::unit_x(), Rad(tilt));
        let t = builder::rotated(&t, Point3::origin(), Vector3::unit_z(), Rad(turn));
        let t = builder::translated(&t, Vector3::new(off[0], off[1], 0.0));
        let s = monstertruck::solid::difference(&cube([-1.0; 3], [1.0; 3]), &t, TOL).map_err(e)?;
        Ok(solid(&s))
    };
    go().unwrap_or_else(Outcome::Refused)
}
