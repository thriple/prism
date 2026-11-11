//! Where a moving face first meets another face, when arcs or curved
//! faces are involved (Exact Curves 6c-2). `reach` uses it for a flat face
//! pulled along its normal and for a cylinder face taking a new radius.
//!
//! The moving face reaches a point `q` of the other face after a distance
//! `t` (its "foot" is where on the moving face `q` is met). The first
//! meeting is the smallest `t` over the points of the other face whose
//! foot is inside the moving face. It is found among:
//! - the other face's edges, sampled, with each local minimum refined and
//!   each place where the foot leaves the moving face found by bisection;
//! - rays from the moving face's outline, along the way it moves, hitting
//!   the other face (refined the same way);
//! - points inside the other face where it faces straight back at the
//!   moving face: exact for spheres and tori under a slide; a grid search
//!   for a curved face meeting a growing cylinder.

use std::f64::consts::{FRAC_PI_2, TAU};

use prism_geom::{Curve, Profile, Revolution, Surface};
use prism_math::{DVec2, DVec3, Tol};
use prism_topo::{FaceId, Model};

use crate::crossing::Laid;
use crate::shell::Planar;

/// An edge as its two ends and its curve.
type Edge3 = (DVec3, DVec3, Curve);

fn edges_of(m: &Model, faces: &[FaceId]) -> Vec<Edge3> {
    faces
        .iter()
        .filter_map(|f| m.face(*f))
        .flat_map(|f| f.sides().collect::<Vec<_>>())
        .filter_map(|s| {
            let e = m.edge(s.edge)?;
            Some((m.vertex(e.a), m.vertex(e.b), e.curve))
        })
        .collect()
}

/// What moves.
pub(crate) enum Mover {
    /// A flat face moving along `dir` (a unit vector).
    Slide {
        face: Planar,
        origin: DVec3,
        dir: DVec3,
        edges: Vec<Edge3>,
    },
    /// Faces on one turned surface taking a new size: their points move
    /// square to it, along the faces' normal (`sign` 1) or against it (-1).
    Grow {
        faces: Vec<Laid>,
        surface: Surface,
        sign: f64,
        edges: Vec<Edge3>,
    },
}

impl Mover {
    pub(crate) fn slide(m: &Model, face: FaceId, dir: DVec3) -> Option<Mover> {
        let planar = Planar::of(m, face)?;
        Some(Mover::Slide {
            origin: planar.origin,
            face: planar,
            dir,
            edges: edges_of(m, &[face]),
        })
    }

    pub(crate) fn grow(m: &Model, faces: &[FaceId], surface: Surface, sign: f64) -> Option<Mover> {
        let laid: Option<Vec<Laid>> = faces.iter().map(|f| Laid::new(m, *f)).collect();
        Some(Mover::Grow {
            faces: laid?,
            surface,
            sign,
            edges: edges_of(m, faces),
        })
    }

    /// How far the face moves before it reaches `q`, if `q`'s foot is
    /// inside it (not within tolerance of its outline).
    fn foot(&self, q: DVec3, tol: &Tol) -> Option<f64> {
        match self {
            Mover::Slide {
                face, origin, dir, ..
            } => {
                let t = (q - *origin).dot(*dir);
                (face.classify(q - *dir * t, tol.dist) == Some(true)).then_some(t)
            }
            Mover::Grow {
                faces,
                surface,
                sign,
                ..
            } => {
                let foot = surface.project(q);
                faces
                    .iter()
                    .any(|l| l.classify(foot, tol.dist) == Some(true))
                    .then_some(surface.distance(q) * sign)
            }
        }
    }

    /// The way a point `p` of the moving face goes.
    fn way(&self, p: DVec3) -> DVec3 {
        match self {
            Mover::Slide { dir, .. } => *dir,
            Mover::Grow { surface, sign, .. } => {
                let n = surface.normal_at(p) * *sign;
                if n.is_finite() { n } else { DVec3::ZERO }
            }
        }
    }

    fn edges(&self) -> &[Edge3] {
        match self {
            Mover::Slide { edges, .. } | Mover::Grow { edges, .. } => edges,
        }
    }
}

/// The face met.
enum Target {
    Flat(Planar),
    Curved { laid: Laid, lo: DVec3, hi: DVec3 },
}

impl Target {
    fn of(m: &Model, id: FaceId) -> Option<Target> {
        let f = m.face(id)?;
        if f.plane().is_some() {
            return Planar::of(m, id).map(Target::Flat);
        }
        let (lo, hi) = crate::cut::face_box(m, f)?;
        Some(Target::Curved {
            laid: Laid::new(m, id)?,
            lo,
            hi,
        })
    }

    fn holds(&self, p: DVec3, tol: &Tol) -> bool {
        match self {
            Target::Flat(f) => f.classify(p, tol.dist) == Some(true),
            Target::Curved { laid, .. } => laid.classify(p, tol.dist) == Some(true),
        }
    }

    /// The nearest point where the ray `o + d t` (0 < t ≤ `reach`) enters
    /// or leaves the face, inside it.
    fn first_hit(&self, o: DVec3, d: DVec3, reach: f64, tol: &Tol) -> Option<f64> {
        match self {
            Target::Flat(f) => {
                let den = f.normal.dot(d);
                if den.abs() < 1e-9 {
                    return None;
                }
                let t = (f.origin - o).dot(f.normal) / den;
                (t > tol.dist && t <= reach && self.holds(o + d * t, tol)).then_some(t)
            }
            Target::Curved { laid, lo, hi } => {
                let (t0, t1) = crate::shell::ray_span(o, d, *lo, *hi)?;
                let t1 = t1.min(reach + tol.dist);
                if t0 >= t1 {
                    return None;
                }
                let s = &laid.surface;
                let g = |t: f64| s.distance(o + d * t);
                let size = laid.scale.x.min(laid.scale.y).max(tol.dist * 10.0);
                let n = (((t1 - t0) / (size / 40.0)).ceil() as usize).clamp(8, 4000);
                let (mut ta, mut ga) = (t0, g(t0));
                for i in 1..=n {
                    let tb = t0 + (t1 - t0) * i as f64 / n as f64;
                    let gb = g(tb);
                    // A real change of side: a ray running along the
                    // surface (a seam of a cylinder it slides along) only
                    // gives noise near zero.
                    if (ga < 0.0) != (gb < 0.0) && ga.abs().max(gb.abs()) > tol.dist {
                        let (mut a, mut b, mut gl) = (ta, tb, ga);
                        for _ in 0..80 {
                            let mid = (a + b) / 2.0;
                            let gm = g(mid);
                            if (gm < 0.0) == (gl < 0.0) {
                                a = mid;
                                gl = gm;
                            } else {
                                b = mid;
                            }
                        }
                        let t = (a + b) / 2.0;
                        if t > tol.dist && t <= reach && self.holds(o + d * t, tol) {
                            return Some(t);
                        }
                    }
                    ta = tb;
                    ga = gb;
                }
                None
            }
        }
    }
}

/// Keep `t` if it is ahead and nearer than `best`.
fn take(best: &mut f64, t: f64, eps: f64) {
    if t > eps && t < *best {
        *best = t;
    }
}

/// The smallest value of `f` over [0, 1] into `best`: `f` is sampled at
/// `n` steps, each local minimum is refined (golden section), and each end
/// of a run of values is found by bisection.
fn scan(n: usize, f: &dyn Fn(f64) -> Option<f64>, best: &mut f64, eps: f64) {
    let us: Vec<f64> = (0..=n).map(|i| i as f64 / n as f64).collect();
    let vals: Vec<Option<f64>> = us.iter().map(|u| f(*u)).collect();
    let big = |u: f64| f(u).unwrap_or(f64::INFINITY);
    for i in 0..=n {
        if let Some(v) = vals[i] {
            take(best, v, eps);
            let lower = |j: usize| vals[j].is_none_or(|w| w >= v);
            if i > 0 && i < n && lower(i - 1) && lower(i + 1) {
                let w = golden(&big, us[i - 1], us[i + 1]);
                if w.is_finite() {
                    take(best, w, eps);
                }
            }
        }
        if i < n && vals[i].is_some() != vals[i + 1].is_some() {
            // Where the run of values ends.
            let (mut a, mut b) = if vals[i].is_some() {
                (us[i], us[i + 1])
            } else {
                (us[i + 1], us[i])
            };
            for _ in 0..60 {
                let mid = (a + b) / 2.0;
                if f(mid).is_some() {
                    a = mid;
                } else {
                    b = mid;
                }
            }
            if let Some(v) = f(a) {
                take(best, v, eps);
            }
        }
    }
}

/// The minimum of `f` on [a, b] by golden section.
fn golden(f: &dyn Fn(f64) -> f64, mut a: f64, mut b: f64) -> f64 {
    let k = (5f64.sqrt() - 1.0) / 2.0;
    let mut c = b - (b - a) * k;
    let mut d = a + (b - a) * k;
    let (mut fc, mut fd) = (f(c), f(d));
    for _ in 0..80 {
        if fc <= fd {
            b = d;
            d = c;
            fd = fc;
            c = b - (b - a) * k;
            fc = f(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + (b - a) * k;
            fd = f(d);
        }
    }
    f((a + b) / 2.0)
}

/// How many samples along an edge.
fn samples(curve: &Curve) -> usize {
    if curve.is_arc() { 48 } else { 16 }
}

/// Points of a turned surface whose normal points along `dir` or against
/// it: on a sphere or torus, where it faces a sliding face head on.
fn facing_points(rev: &Revolution, dir: DVec3) -> Vec<DVec3> {
    let Profile::Circle { r0, h0, radius } = rev.profile else {
        // Cones and cylinders face along whole lines, which end on edges.
        return Vec::new();
    };
    let a = dir.dot(rev.axis);
    let perp = dir - rev.axis * a;
    let b = perp.length();
    let mut out = Vec::new();
    if b > 1e-9 {
        let e = perp / b;
        for (e, sigma) in [(e, 1.0), (-e, -1.0)] {
            for sg in [1.0, -1.0] {
                // The normal is e cos s + axis sin s.
                let (cs, sn) = (sg * b * sigma, sg * a);
                out.push(rev.origin + rev.axis * (h0 + radius * sn) + e * (r0 + radius * cs));
            }
        }
    } else {
        // Along the axis: the top and bottom circles (or poles).
        for k in 0..16 {
            let th = TAU * k as f64 / 16.0;
            for s in [FRAC_PI_2, -FRAC_PI_2] {
                out.push(rev.point(th, s));
            }
        }
    }
    out
}

/// The least distance (no more than `reach`) the mover goes before it
/// meets face `g`, from the candidates above. `reach` if it does not.
pub(crate) fn first_meeting(m: &Model, mover: &Mover, g: FaceId, reach: f64) -> f64 {
    let tol = m.tol();
    let mut best = reach;
    let Some(target) = Target::of(m, g) else {
        return best;
    };
    let eps = tol.dist;
    // The other face's edges.
    for (a, b, curve) in edges_of(m, &[g]) {
        let f = |u: f64| mover.foot(curve.point(a, b, u), &tol);
        scan(samples(&curve), &f, &mut best, eps);
    }
    // Rays from the moving face's outline.
    for (a, b, curve) in mover.edges() {
        let f = |u: f64| {
            let p = curve.point(*a, *b, u);
            target.first_hit(p, mover.way(p), reach, &tol)
        };
        scan(samples(curve), &f, &mut best, eps);
    }
    // Inside the other face.
    if let Target::Curved { laid, .. } = &target {
        match mover {
            Mover::Slide { dir, .. } => {
                for q in facing_points(&laid.rev, *dir) {
                    if target.holds(q, &tol)
                        && let Some(t) = mover.foot(q, &tol)
                    {
                        take(&mut best, t, eps);
                    }
                }
            }
            Mover::Grow { .. } => grid_search(laid, mover, &tol, &mut best),
        }
    }
    // A flat face met head on by a growing sphere or torus: from where the
    // surface faces it, straight out.
    if let (Target::Flat(plane), Mover::Grow { faces, surface, .. }) = (&target, mover)
        && let Some(rev) = surface.revolution()
    {
        for p in facing_points(&rev, plane.normal) {
            if faces.iter().any(|l| l.classify(p, tol.dist) == Some(true))
                && let Some(t) = target.first_hit(p, mover.way(p), best, &tol)
            {
                take(&mut best, t, eps);
            }
        }
    }
    best
}

/// The least distance at which a growing surface meets a point inside a
/// curved face: a grid over the face's layout, each local minimum refined
/// by a compass search.
fn grid_search(laid: &Laid, mover: &Mover, tol: &Tol, best: &mut f64) {
    let Some(outer) = laid.polys.first() else {
        return;
    };
    let first = outer[0];
    let (lo, hi) = outer
        .iter()
        .fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
    let f = |q: DVec2| -> f64 {
        if !laid.holds_2d(q, 0.0) {
            return f64::INFINITY;
        }
        match mover.foot(laid.rev.point(q.x, q.y), tol) {
            Some(t) if t > tol.dist => t,
            _ => f64::INFINITY,
        }
    };
    const N: usize = 16;
    let cell = (hi - lo) / N as f64;
    let at = |i: usize, j: usize| lo + DVec2::new(cell.x * i as f64, cell.y * j as f64);
    let vals: Vec<Vec<f64>> = (0..=N)
        .map(|i| (0..=N).map(|j| f(at(i, j))).collect())
        .collect();
    for i in 0..=N {
        for j in 0..=N {
            let v = vals[i][j];
            if !v.is_finite() {
                continue;
            }
            take(best, v, tol.dist);
            let lower = [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)]
                .iter()
                .all(|(di, dj)| {
                    let (x, y) = (i as i64 + di, j as i64 + dj);
                    x < 0
                        || y < 0
                        || x > N as i64
                        || y > N as i64
                        || vals[x as usize][y as usize] >= v
                });
            if !lower {
                continue;
            }
            // Compass search from the grid point.
            let (mut q, mut fq) = (at(i, j), v);
            let mut step = cell * 0.5;
            let stop = cell.length() * 1e-9;
            while step.length() > stop {
                let mut moved = false;
                for d in [
                    DVec2::new(step.x, 0.0),
                    DVec2::new(-step.x, 0.0),
                    DVec2::new(0.0, step.y),
                    DVec2::new(0.0, -step.y),
                ] {
                    let w = f(q + d);
                    if w < fq {
                        q += d;
                        fq = w;
                        moved = true;
                        break;
                    }
                }
                if !moved {
                    step *= 0.5;
                }
            }
            take(best, fq, tol.dist);
        }
    }
}

/// Whether a face has arcs on its outline.
pub(crate) fn has_arcs(m: &Model, id: FaceId) -> bool {
    m.face(id).is_some_and(|f| {
        f.edge_ids()
            .any(|e| m.edge(e).is_some_and(|x| x.curve.is_arc()))
    })
}

/// The box round a face, curved bulges included.
pub(crate) fn face_bounds(m: &Model, id: FaceId) -> Option<(DVec3, DVec3)> {
    let f = m.face(id)?;
    if f.is_curved() {
        return crate::cut::face_box(m, f);
    }
    let pts: Vec<DVec3> = m.face_loop_points(f).into_iter().flatten().collect();
    let first = *pts.first()?;
    Some(
        pts.iter()
            .fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p))),
    )
}
