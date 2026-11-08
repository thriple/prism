//! Where planar faces cross (DECISIONS #29), and Intersect Faces.
//!
//! Two faces cross along the line where their planes meet, where that line
//! is inside both faces. A crossing that runs along both faces' outlines is
//! the edge they share, so it does not count. One along one face's outline
//! but inside the other does (a wall standing on a floor with no edge under
//! it). Connected or not makes no difference.

use std::collections::BTreeSet;

use prism_math::predicates::{Inside, point_in_polygon};
use prism_math::{DVec2, DVec3, Tol};
use prism_topo::{FaceId, Model};

/// A planar face in its own 2D frame (the viewport's scene faces and the
/// model's faces both give one).
#[derive(Clone, Debug)]
pub struct Flat {
    pub origin: DVec3,
    pub u: DVec3,
    pub v: DVec3,
    pub normal: DVec3,
    pub outer: Vec<DVec2>,
    pub holes: Vec<Vec<DVec2>>,
    pub lo: DVec3,
    pub hi: DVec3,
}

impl Flat {
    /// A model face's flat form.
    pub fn of(m: &Model, id: FaceId) -> Option<Flat> {
        let f = m.face(id)?;
        let plane = *f.plane();
        let frame = plane.frame();
        let to2 = |l: &[prism_topo::VertexId]| -> Vec<DVec2> {
            l.iter().map(|v| frame.to_2d(m.vertex(*v))).collect()
        };
        let pts: Vec<DVec3> = f.loops().flatten().map(|v| m.vertex(*v)).collect();
        let (lo, hi) = pts
            .iter()
            .fold((pts[0], pts[0]), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        Some(Flat {
            origin: frame.origin,
            u: frame.u,
            v: frame.v,
            normal: plane.normal,
            outer: to2(&f.outer),
            holes: f.holes.iter().map(|h| to2(h)).collect(),
            lo,
            hi,
        })
    }

    fn to_2d(&self, p: DVec3) -> DVec2 {
        let d = p - self.origin;
        DVec2::new(d.dot(self.u), d.dot(self.v))
    }

    fn inside(&self, q: DVec2) -> bool {
        point_in_polygon(q, &self.outer) == Inside::Inside
            && self
                .holes
                .iter()
                .all(|h| point_in_polygon(q, h) == Inside::Outside)
    }

    /// Within `d` of the outline or a hole's outline.
    fn on_outline(&self, q: DVec2, d: f64) -> bool {
        std::iter::once(&self.outer)
            .chain(self.holes.iter())
            .any(|l| {
                (0..l.len()).any(|i| {
                    let (a, b) = (l[i], l[(i + 1) % l.len()]);
                    let e = b - a;
                    let t = ((q - a).dot(e) / e.length_squared().max(f64::MIN_POSITIVE))
                        .clamp(0.0, 1.0);
                    q.distance(a + e * t) < d
                })
            })
    }

    /// The parts of the line `p + dir t` inside the face (on its outline
    /// counts as inside), as (t0, t1).
    fn clip(&self, p: DVec3, dir: DVec3, tol: &Tol) -> Vec<(f64, f64)> {
        let q = self.to_2d(p);
        let d = DVec2::new(dir.dot(self.u), dir.dot(self.v));
        let mut ts = Vec::new();
        for l in std::iter::once(&self.outer).chain(self.holes.iter()) {
            for i in 0..l.len() {
                let (a, b) = (l[i], l[(i + 1) % l.len()]);
                let e = b - a;
                let den = d.perp_dot(e);
                if den.abs() <= 1e-14 * e.length() {
                    // Along the line: its ends bound the parts.
                    if (a - q).perp_dot(d).abs() <= tol.dist {
                        ts.push((a - q).dot(d));
                        ts.push((b - q).dot(d));
                    }
                    continue;
                }
                let s = (a - q).perp_dot(d) / den;
                if (-1e-12..=1.0 + 1e-12).contains(&s) {
                    ts.push((a - q).perp_dot(e) / den);
                }
            }
        }
        ts.sort_by(f64::total_cmp);
        ts.dedup_by(|x, y| (*x - *y).abs() <= tol.dist);
        let mut out: Vec<(f64, f64)> = Vec::new();
        for w in ts.windows(2) {
            let mid = q + d * (0.5 * (w[0] + w[1]));
            if self.inside(mid) || self.on_outline(mid, tol.dist) {
                match out.last_mut() {
                    Some(last) if (last.1 - w[0]).abs() <= tol.dist => last.1 = w[1],
                    _ => out.push((w[0], w[1])),
                }
            }
        }
        out
    }
}

/// The segments where faces `a` and `b` cross.
pub fn crossings(a: &Flat, b: &Flat, tol: &Tol) -> Vec<(DVec3, DVec3)> {
    let pad = DVec3::splat(tol.dist);
    if (a.hi + pad).cmplt(b.lo).any() || (b.hi + pad).cmplt(a.lo).any() {
        return Vec::new();
    }
    let d = a.normal.cross(b.normal);
    let len2 = d.length_squared();
    if len2 <= 1e-18 {
        return Vec::new();
    }
    // A point on both planes.
    let (ca, cb) = (a.normal.dot(a.origin), b.normal.dot(b.origin));
    let p = (b.normal.cross(d) * ca + d.cross(a.normal) * cb) / len2;
    let dir = d / len2.sqrt();
    let (ia, ib) = (a.clip(p, dir, tol), b.clip(p, dir, tol));
    // Corners of either face on the line: a crossing can be a shared edge
    // for part of its length only, so each piece between them is tested.
    let mut stops: Vec<f64> = [a, b]
        .iter()
        .flat_map(|f| {
            std::iter::once(&f.outer)
                .chain(f.holes.iter())
                .flatten()
                .map(move |q| f.origin + f.u * q.x + f.v * q.y)
        })
        .filter(|x| (*x - p).cross(dir).length() <= tol.dist)
        .map(|x| (x - p).dot(dir))
        .collect();
    stops.sort_by(f64::total_cmp);
    let mut out: Vec<(DVec3, DVec3)> = Vec::new();
    for (a0, a1) in &ia {
        for (b0, b1) in &ib {
            let (t0, t1) = (a0.max(*b0), a1.min(*b1));
            if t1 - t0 <= tol.dist * 10.0 {
                continue;
            }
            let mut cuts: Vec<f64> = std::iter::once(t0)
                .chain(
                    stops
                        .iter()
                        .copied()
                        .filter(|t| *t > t0 + tol.dist && *t < t1 - tol.dist),
                )
                .chain(std::iter::once(t1))
                .collect();
            cuts.dedup_by(|x, y| (*x - *y).abs() <= tol.dist);
            let mut run: Option<(f64, f64)> = None;
            for w in cuts.windows(2) {
                let mid = p + dir * (0.5 * (w[0] + w[1]));
                // Along both outlines: the edge they share.
                let shared =
                    a.on_outline(a.to_2d(mid), tol.dist) && b.on_outline(b.to_2d(mid), tol.dist);
                match (&mut run, shared) {
                    (Some(r), false) => r.1 = w[1],
                    (None, false) => run = Some((w[0], w[1])),
                    (Some(r), true) => {
                        out.push((p + dir * r.0, p + dir * r.1));
                        run = None;
                    }
                    (None, true) => {}
                }
            }
            if let Some(r) = run {
                out.push((p + dir * r.0, p + dir * r.1));
            }
        }
    }
    out
}

/// Every crossing in the model, or only those involving `only`.
pub fn all_crossings(m: &Model, only: Option<&BTreeSet<FaceId>>) -> Vec<(DVec3, DVec3)> {
    let tol = m.tol();
    let faces: Vec<(FaceId, Flat)> = m
        .faces()
        .filter_map(|(id, _)| Flat::of(m, id).map(|f| (id, f)))
        .collect();
    let mut out = Vec::new();
    for i in 0..faces.len() {
        for j in i + 1..faces.len() {
            if let Some(s) = only
                && !s.contains(&faces[i].0)
                && !s.contains(&faces[j].0)
            {
                continue;
            }
            out.extend(crossings(&faces[i].1, &faces[j].1, &tol));
        }
    }
    out
}

/// Intersect Faces: every crossing (or those involving `only`) becomes an
/// edge, and the faces split along them. Returns how many segments were
/// added.
pub fn intersect_faces(m: &mut Model, only: Option<&BTreeSet<FaceId>>) -> usize {
    let segs = all_crossings(m, only);
    for (a, b) in &segs {
        crate::add_segment(m, *a, *b);
    }
    segs.len()
}
