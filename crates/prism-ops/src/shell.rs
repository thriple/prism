//! The faces of solids for the inside test (DECISIONS #31), exact with
//! arcs and curved faces (Exact Curves 6c).
//!
//! A ray from a point counts the faces it crosses: +1 leaving through a
//! face, -1 entering. A flat face is a plane and an outline of lines and
//! exact arcs; a curved face is its surface and its outline in (θ, s). A
//! ray meets a curved surface where the distance to it changes sign along
//! the ray (found in small steps, then halved down to the crossing), so a
//! torus needs no quartic; a ray that only grazes the surface between two
//! steps crosses it twice or not at all, which counts the same. A crossing
//! within a tolerance of a face's outline, or along a face, is unsure: the
//! next ray is tried.

use prism_geom::Curve;
use prism_math::predicates::orient2d;
use prism_math::{DVec2, DVec3, Tol};
use prism_topo::{FaceId, Model};

use crate::crossing::Laid;

/// One side of a flat face's outline in its plane's frame.
#[derive(Clone, Debug)]
enum Side2 {
    Line(DVec2, DVec2),
    /// From `a` to `b` on the circle about `c`; `mid` is on the arc.
    Arc {
        a: DVec2,
        b: DVec2,
        c: DVec2,
        r: f64,
        mid: DVec2,
    },
}

/// A flat face: its plane and its outline with exact arcs.
#[derive(Clone, Debug)]
pub(crate) struct Planar {
    pub(crate) origin: DVec3,
    pub(crate) u: DVec3,
    pub(crate) v: DVec3,
    pub(crate) normal: DVec3,
    sides: Vec<Side2>,
    pub(crate) lo: DVec3,
    pub(crate) hi: DVec3,
}

impl Planar {
    pub(crate) fn of(m: &Model, id: FaceId) -> Option<Planar> {
        let f = m.face(id)?;
        let plane = *f.plane()?;
        let fr = plane.frame();
        let mut sides = Vec::new();
        for s in f.sides() {
            let e = m.edge(s.edge)?;
            let (pa, pb) = (m.vertex(e.a), m.vertex(e.b));
            let (a, b) = (fr.to_2d(pa), fr.to_2d(pb));
            match e.curve {
                // A spline as the straight steps it is drawn with.
                Curve::Spline(_) => {
                    let mut pts = vec![a];
                    pts.extend(e.curve.inner_points(pa, pb).iter().map(|p| fr.to_2d(*p)));
                    pts.push(b);
                    sides.extend(pts.windows(2).map(|w| Side2::Line(w[0], w[1])));
                }
                Curve::Line => sides.push(Side2::Line(a, b)),
                Curve::Arc { centre, radius, .. } => sides.push(Side2::Arc {
                    a,
                    b,
                    c: fr.to_2d(centre),
                    r: radius,
                    mid: fr.to_2d(e.curve.point(pa, pb, 0.5)),
                }),
            }
        }
        let pts: Vec<DVec3> = m.face_loop_points(f).into_iter().flatten().collect();
        let first = *pts.first()?;
        let (lo, hi) = pts
            .iter()
            .fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        // Arcs bulge past their drawn points by at most their sag.
        let sag = sides
            .iter()
            .map(|s| match s {
                Side2::Arc { r, .. } => r * 0.01,
                Side2::Line(..) => 0.0,
            })
            .fold(0.0, f64::max);
        let pad = DVec3::splat(sag);
        Some(Planar {
            origin: fr.origin,
            u: fr.u,
            v: fr.v,
            normal: plane.normal,
            sides,
            lo: lo - pad,
            hi: hi + pad,
        })
    }

    pub(crate) fn to_2d(&self, p: DVec3) -> DVec2 {
        let d = p - self.origin;
        DVec2::new(d.dot(self.u), d.dot(self.v))
    }

    /// Whether `q` is inside, exactly: the outline with each arc's chord,
    /// counted along a ray, then each arc's piece of its circle beyond the
    /// chord added or taken away.
    pub(crate) fn contains(&self, q: DVec2) -> bool {
        let mut inside = false;
        for s in &self.sides {
            let (a, b) = match s {
                Side2::Line(a, b) => (*a, *b),
                Side2::Arc { a, b, .. } => (*a, *b),
            };
            if (a.y > q.y) != (b.y > q.y) {
                let x = a.x + (q.y - a.y) * (b.x - a.x) / (b.y - a.y);
                if x > q.x {
                    inside = !inside;
                }
            }
            if let Side2::Arc { a, b, c, r, mid } = s
                && q.distance(*c) < *r
                && orient2d(*a, *b, q).signum() == orient2d(*a, *b, *mid).signum()
                && orient2d(*a, *b, q) != 0.0
            {
                inside = !inside;
            }
        }
        inside
    }

    /// Whether `q` is within `d` of the outline.
    pub(crate) fn near(&self, q: DVec2, d: f64) -> bool {
        self.sides.iter().any(|s| match s {
            Side2::Line(a, b) => {
                let e = *b - *a;
                let t =
                    ((q - *a).dot(e) / e.length_squared().max(f64::MIN_POSITIVE)).clamp(0.0, 1.0);
                q.distance(*a + e * t) < d
            }
            Side2::Arc { a, b, c, r, mid } => {
                // On the arc's side of its chord (or at its ends): the
                // distance to the circle; else to the nearer end.
                let toward = q - *c;
                let on_circle = *c + toward.normalize_or_zero() * *r;
                let within =
                    orient2d(*a, *b, on_circle).signum() == orient2d(*a, *b, *mid).signum();
                if within {
                    (q.distance(*c) - r).abs() < d
                } else {
                    q.distance(*a) < d || q.distance(*b) < d
                }
            }
        })
    }

    /// Whether a point of the plane is in the face: None near its outline.
    pub(crate) fn classify(&self, p: DVec3, d: f64) -> Option<bool> {
        let q = self.to_2d(p);
        if self.near(q, d) {
            return None;
        }
        Some(self.contains(q))
    }
}

/// A curved face: its surface and its outline in (θ, s).
pub(crate) struct Bent {
    laid: Laid,
    lo: DVec3,
    hi: DVec3,
}

/// A face of a solid, for the inside test.
pub(crate) enum ShellFace {
    Flat(Planar),
    Curved(Bent),
}

impl ShellFace {
    pub(crate) fn of(m: &Model, id: FaceId) -> Option<ShellFace> {
        let f = m.face(id)?;
        if f.plane().is_some() {
            return Planar::of(m, id).map(ShellFace::Flat);
        }
        let laid = Laid::new(m, id)?;
        let (lo, hi) = crate::cut::face_box(m, f)?;
        Some(ShellFace::Curved(Bent { laid, lo, hi }))
    }

    pub(crate) fn bounds(&self) -> (DVec3, DVec3) {
        match self {
            ShellFace::Flat(p) => (p.lo, p.hi),
            ShellFace::Curved(b) => (b.lo, b.hi),
        }
    }

    /// The signed crossings of the ray `o + d t` (t > 0) with the face: +1
    /// leaving, -1 entering. None if one is too close to tell.
    fn crossings(&self, o: DVec3, d: DVec3, tol: &Tol) -> Option<i32> {
        match self {
            ShellFace::Flat(f) => {
                let den = f.normal.dot(d);
                let t = (f.origin - o).dot(f.normal);
                if den.abs() < 1e-9 {
                    // Along the plane: unsure if in it.
                    return if t.abs() <= tol.dist { None } else { Some(0) };
                }
                let hit = o + d * (t / den);
                if t.abs() <= tol.dist {
                    // Starting on the plane: on the face itself, no ray
                    // can tell the side.
                    return match f.classify(hit, tol.dist) {
                        Some(false) => Some(0),
                        _ => None,
                    };
                }
                if t / den <= 0.0 {
                    return Some(0);
                }
                Some(match f.classify(hit, tol.dist)? {
                    true if den > 0.0 => 1,
                    true => -1,
                    false => 0,
                })
            }
            ShellFace::Curved(b) => {
                let (t0, t1) = ray_span(o, d, b.lo, b.hi)?;
                let s = &b.laid.surface;
                let g = |t: f64| s.distance(o + d * t);
                // Steps of a twentieth of the surface's size.
                let size = b
                    .laid
                    .scale
                    .x
                    .min(b.laid.scale.y.max(1e-300))
                    .max(tol.dist * 10.0);
                let n = (((t1 - t0) / (size / 20.0)).ceil() as usize).clamp(8, 4000);
                let mut w = 0;
                let (mut ta, mut ga) = (t0, g(t0));
                if ga.abs() <= tol.dist && t0 <= tol.dist {
                    // Starting on the surface.
                    if b.laid.classify(o, tol.dist) != Some(false) {
                        return None;
                    }
                }
                for i in 1..=n {
                    let tb = t0 + (t1 - t0) * i as f64 / n as f64;
                    let gb = g(tb);
                    if (ga < 0.0) != (gb < 0.0) {
                        let (mut lo, mut hi, mut glo) = (ta, tb, ga);
                        for _ in 0..80 {
                            let m = (lo + hi) / 2.0;
                            let gm = g(m);
                            if (gm < 0.0) == (glo < 0.0) {
                                lo = m;
                                glo = gm;
                            } else {
                                hi = m;
                            }
                        }
                        let t = (lo + hi) / 2.0;
                        if t > tol.dist {
                            let p = o + d * t;
                            if b.laid.classify(p, tol.dist)? {
                                let nn = s.normal_at(p);
                                let c = nn.dot(d);
                                if c.abs() < 1e-6 {
                                    return None;
                                }
                                w += if c > 0.0 { 1 } else { -1 };
                            }
                        }
                    }
                    ta = tb;
                    ga = gb;
                }
                Some(w)
            }
        }
    }
}

/// Where the ray `o + d t` (t ≥ 0) is inside the box, padded.
pub(crate) fn ray_span(o: DVec3, d: DVec3, lo: DVec3, hi: DVec3) -> Option<(f64, f64)> {
    let pad = DVec3::splat((hi - lo).length() * 1e-6 + 1e-12);
    let (lo, hi) = (lo - pad, hi + pad);
    let (mut t0, mut t1) = (0.0f64, f64::INFINITY);
    for k in 0..3 {
        if d[k].abs() < 1e-300 {
            if o[k] < lo[k] || o[k] > hi[k] {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo[k] - o[k]) / d[k], (hi[k] - o[k]) / d[k]);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
        if t0 > t1 {
            return None;
        }
    }
    Some((t0, t1))
}

/// Ray directions, tried in turn until one passes cleanly. Fixed, so the
/// answer is the same every run.
const RAYS: [DVec3; 4] = [
    DVec3::new(0.284_7, 0.539_1, 0.792_6),
    DVec3::new(-0.613_2, 0.217_9, 0.759_3),
    DVec3::new(0.471_5, -0.802_3, 0.366_1),
    DVec3::new(-0.152_8, -0.433_6, -0.888_1),
];

/// How many closed shells `p` is inside. None if every ray is unsure.
pub(crate) fn winding(faces: &[ShellFace], p: DVec3, tol: &Tol) -> Option<i32> {
    'ray: for d in RAYS {
        let d = d.normalize();
        let mut w = 0;
        for f in faces {
            let (lo, hi) = f.bounds();
            if ray_span(p, d, lo, hi).is_none() {
                continue;
            }
            match f.crossings(p, d, tol) {
                Some(k) => w += k,
                None => continue 'ray,
            }
        }
        return Some(w);
    }
    None
}

/// A point inside a face and which way it faces there (for testing its
/// two sides).
pub(crate) fn interior(m: &Model, id: FaceId) -> Vec<(DVec3, DVec3)> {
    let Some(f) = m.face(id) else {
        return Vec::new();
    };
    if let Some(pl) = f.plane() {
        let fr = pl.frame();
        let loops: Vec<Vec<DVec2>> = m
            .face_loop_points(f)
            .iter()
            .map(|l| l.iter().map(|p| fr.to_2d(*p)).collect())
            .collect();
        return crate::regions::interior_points(&loops)
            .into_iter()
            .map(|q| (fr.to_3d(q), pl.normal))
            .collect();
    }
    let Some((rev, polys)) = crate::cut::layout(m, id) else {
        return Vec::new();
    };
    crate::regions::interior_points(&polys)
        .into_iter()
        .map(|q| {
            let p = rev.point(q.x, q.y);
            (p, f.surface.normal_at(p))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// A solid of revolution by Follow Me of `profile` (drawn first, as
    /// segments and arcs) round a circle about z at height 5.
    fn turned(draw: impl Fn(&mut Model)) -> (Model, Vec<ShellFace>) {
        let mut m = Model::new();
        draw(&mut m);
        let prof = m.faces().next().map(|(id, _)| id).expect("a profile");
        let before: BTreeSet<_> = m.edges().map(|(id, _)| id).collect();
        crate::add_circle(&mut m, DVec3::new(0.0, 0.0, 5.0), DVec3::Z, 0.5);
        let path: BTreeSet<_> = m
            .edges()
            .map(|(id, _)| id)
            .filter(|id| !before.contains(id))
            .collect();
        assert!(crate::follow::follow(&mut m, &path, prof));
        let faces = crate::solid::closed_faces(&m)
            .into_iter()
            .filter_map(|f| ShellFace::of(&m, f))
            .collect();
        (m, faces)
    }

    #[test]
    fn points_inside_a_sphere_and_a_torus_count_once() {
        let (m, sphere) = turned(|m| {
            crate::add_segment(m, DVec3::new(0.0, 0.0, -1.0), DVec3::new(0.0, 0.0, 1.0));
            crate::add_arc(
                m,
                DVec3::ZERO,
                -DVec3::Y,
                1.0,
                DVec3::new(0.0, 0.0, -1.0),
                std::f64::consts::PI,
            );
        });
        let tol = m.tol();
        for (p, want) in [
            (DVec3::ZERO, 1),
            (DVec3::new(0.3, -0.5, 0.6), 1),
            (DVec3::new(0.0, 0.0, 0.999), 1),
            (DVec3::new(0.8, 0.8, 0.0), 0),
            (DVec3::new(0.0, 0.0, 1.001), 0),
        ] {
            assert_eq!(winding(&sphere, p, &tol), Some(want), "{p}");
        }
        let (m, torus) = turned(|m| {
            crate::add_circle(m, DVec3::new(2.0, 0.0, 0.0), DVec3::Y, 0.5);
        });
        let tol = m.tol();
        for (p, want) in [
            (DVec3::new(2.0, 0.0, 0.0), 1),
            (DVec3::new(-1.7, 0.6, 0.1), 1),
            (DVec3::ZERO, 0),
            (DVec3::new(2.0, 0.0, 0.6), 0),
            (DVec3::new(3.0, 0.0, 0.0), 0),
        ] {
            assert_eq!(winding(&torus, p, &tol), Some(want), "{p}");
        }
    }
}
