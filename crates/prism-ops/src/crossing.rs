//! Where a curved face crosses another face (Exact Curves 6b).
//!
//! The crossing is traced in the curved face's (θ, s): where the other
//! face's plane or surface is reached, found on a fine grid and each point
//! then put on it exactly (Newton steps). It is cut where it leaves either
//! face, the ends put exactly on the edge they cross. A crossing that is a
//! line or a circle (`docs/research/chunks-ahead.md`, part 6: a plane
//! square to the axis or through it, any plane through a sphere, two
//! surfaces on one axis) is kept exact; any other (an ellipse, a crossing
//! of two cylinders) becomes fine segments marked approximate (DECISIONS
//! #32). Exact or not is decided by the points themselves: a curve whose
//! exact points all lie on one line or circle is that line or circle.

use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::TAU;

use prism_geom::{Curve, Profile, Revolution, Surface, arc_through, circle_distance};
use prism_math::predicates::{Inside, point_in_polygon};
use prism_math::{DVec2, DVec3, Plane, Tol};
use prism_topo::{EdgeId, FaceId, Model};

use crate::cut::layout;
use crate::intersect::Flat;

/// What a crossing is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Line,
    /// An exact arc: `sweep` counter-clockwise about `normal` from the
    /// first point to the last.
    Arc {
        centre: DVec3,
        normal: DVec3,
        radius: f64,
        sweep: f64,
    },
    /// Fine segments through every point.
    Approx,
}

/// One crossing: its shape and points (its two ends for a line or an
/// arc, every point for fine segments).
#[derive(Clone, Debug)]
pub struct Piece {
    pub shape: Shape,
    pub points: Vec<DVec3>,
}

impl Piece {
    pub fn start(&self) -> DVec3 {
        self.points[0]
    }

    pub fn end(&self) -> DVec3 {
        self.points[self.points.len() - 1]
    }
}

/// The largest turn one grid step covers (half the chord rule's step, so
/// the traced points are dense enough to choose fine segments from).
fn grid_step() -> f64 {
    (1.0 - 1e-3f64).acos()
}

/// A curved face laid out in (θ, s).
pub(crate) struct Laid {
    pub(crate) face: FaceId,
    pub(crate) surface: Surface,
    pub(crate) rev: Revolution,
    pub(crate) polys: Vec<Vec<DVec2>>,
    circle: bool,
    /// How long a step in θ and in s is, near the face (to measure
    /// distances in (θ, s) as lengths).
    pub(crate) scale: DVec2,
}

impl Laid {
    pub(crate) fn new(m: &Model, face: FaceId) -> Option<Laid> {
        let (rev, polys) = layout(m, face)?;
        let surface = m.face(face)?.surface.clone();
        let circle = matches!(rev.profile, Profile::Circle { .. });
        let (lo, hi) = bounds(&polys[0]);
        let r = |s: f64| rev.at(s).0.abs();
        let scale = match rev.profile {
            Profile::Line { slope, .. } => {
                DVec2::new(r(lo.y).max(r(hi.y)), (1.0 + slope * slope).sqrt())
            }
            Profile::Circle { radius, .. } => {
                let rr = r(lo.y).max(r(hi.y)).max(r((lo.y + hi.y) / 2.0));
                DVec2::new(rr, radius)
            }
        };
        Some(Laid {
            face,
            surface,
            rev,
            polys,
            circle,
            scale,
        })
    }

    /// The curve's size near (θ, s), for the chord rule.
    fn size_at(&self, q: DVec2) -> f64 {
        match self.rev.profile {
            Profile::Line { .. } => self.rev.at(q.y).0.abs(),
            Profile::Circle { radius, .. } => radius,
        }
    }

    /// Whether (θ, s), as laid out, is in the face or within `d` of its
    /// outline.
    pub(crate) fn holds_2d(&self, q: DVec2, d: f64) -> bool {
        inside(q, &self.polys) || self.near_outline(q, d)
    }

    pub(crate) fn near_outline(&self, q: DVec2, d: f64) -> bool {
        let k = self.scale;
        let m = |p: DVec2| DVec2::new(p.x * k.x, p.y * k.y);
        let q = m(q);
        self.polys.iter().any(|l| {
            (0..l.len()).any(|i| {
                let (a, b) = (m(l[i]), m(l[(i + 1) % l.len()]));
                let e = b - a;
                let t =
                    ((q - a).dot(e) / e.length_squared().max(f64::MIN_POSITIVE)).clamp(0.0, 1.0);
                q.distance(a + e * t) < d
            })
        })
    }

    /// Where a point of the surface falls in the layout: its (θ, s), moved
    /// by whole turns into the face, if it is in it.
    /// Whether a point of the surface is in the face: None within `d` of
    /// its outline (too close to tell).
    pub(crate) fn classify(&self, p: DVec3, d: f64) -> Option<bool> {
        let (t, s) = self.rev.param(p);
        let s_turns: &[f64] = if self.circle {
            &[0.0, -1.0, 1.0]
        } else {
            &[0.0]
        };
        for kt in [0.0, -1.0, 1.0, -2.0, 2.0] {
            for ks in s_turns {
                let q = DVec2::new(t + kt * TAU, s + ks * TAU);
                if self.near_outline(q, d) {
                    return None;
                }
                if self.holds_2d(q, 0.0) {
                    return Some(true);
                }
            }
        }
        Some(false)
    }

    pub(crate) fn place(&self, p: DVec3, d: f64) -> Option<DVec2> {
        let (t, s) = self.rev.param(p);
        let s_turns: &[f64] = if self.circle {
            &[0.0, -1.0, 1.0]
        } else {
            &[0.0]
        };
        [0.0, -1.0, 1.0, -2.0, 2.0].iter().find_map(|kt| {
            s_turns.iter().find_map(|ks| {
                let q = DVec2::new(t + kt * TAU, s + ks * TAU);
                self.holds_2d(q, d).then_some(q)
            })
        })
    }
}

fn bounds(poly: &[DVec2]) -> (DVec2, DVec2) {
    poly.iter()
        .fold((DVec2::INFINITY, DVec2::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(*p), hi.max(*p))
        })
}

fn inside(q: DVec2, polys: &[Vec<DVec2>]) -> bool {
    point_in_polygon(q, &polys[0]) == Inside::Inside
        && polys[1..]
            .iter()
            .all(|h| point_in_polygon(q, h) == Inside::Outside)
}

/// The face the curved face crosses.
enum Other {
    Flat(FaceId, Flat, Plane),
    Curved(Laid),
}

impl Other {
    fn face(&self) -> FaceId {
        match self {
            Other::Flat(id, _, _) => *id,
            Other::Curved(l) => l.face,
        }
    }

    /// Signed distance to its plane or surface.
    fn value(&self, p: DVec3) -> f64 {
        match self {
            Other::Flat(_, _, pl) => pl.distance(p),
            Other::Curved(l) => l.surface.distance(p),
        }
    }

    fn holds(&self, p: DVec3, d: f64) -> bool {
        match self {
            Other::Flat(_, f, _) => f.holds(p, d),
            Other::Curved(l) => l.place(p, d).is_some(),
        }
    }

    fn on_outline(&self, p: DVec3, d: f64) -> bool {
        match self {
            Other::Flat(_, f, _) => f.on_outline_3d(p, d),
            Other::Curved(l) => {
                let (t, s) = l.rev.param(p);
                [0.0, -1.0, 1.0, -2.0, 2.0]
                    .iter()
                    .any(|k| l.near_outline(DVec2::new(t + k * TAU, s), d))
            }
        }
    }

    fn size_at(&self, p: DVec3) -> f64 {
        match self {
            Other::Flat(..) => f64::INFINITY,
            Other::Curved(l) => {
                let (t, s) = l.rev.param(p);
                l.size_at(DVec2::new(t, s))
            }
        }
    }
}

/// Lines of the grid where `f` changes sign (marching squares), each a
/// polyline and whether it closes.
fn contours(
    f: &dyn Fn(DVec2) -> f64,
    lo: DVec2,
    step: DVec2,
    nx: usize,
    ny: usize,
) -> Vec<(Vec<DVec2>, bool)> {
    let at = |i: usize, j: usize| lo + DVec2::new(i as f64 * step.x, j as f64 * step.y);
    let v: Vec<Vec<f64>> = (0..=nx)
        .map(|i| (0..=ny).map(|j| f(at(i, j))).collect())
        .collect();
    let pos = |x: f64| x >= 0.0;
    // Edge keys: (0, i, j) from (i, j) to (i + 1, j); (1, i, j) from (i, j)
    // to (i, j + 1).
    type Key = (u8, usize, usize);
    let point = |k: Key| -> DVec2 {
        let (i, j) = (k.1, k.2);
        let (p, q, a, b) = if k.0 == 0 {
            (at(i, j), at(i + 1, j), v[i][j], v[i + 1][j])
        } else {
            (at(i, j), at(i, j + 1), v[i][j], v[i][j + 1])
        };
        p.lerp(q, a / (a - b))
    };
    let mut links: BTreeMap<Key, Vec<Key>> = BTreeMap::new();
    let mut join = |a: Key, b: Key| {
        links.entry(a).or_default().push(b);
        links.entry(b).or_default().push(a);
    };
    for i in 0..nx {
        for j in 0..ny {
            let c = [v[i][j], v[i + 1][j], v[i + 1][j + 1], v[i][j + 1]];
            let e: [Key; 4] = [(0, i, j), (1, i + 1, j), (0, i, j + 1), (1, i, j)];
            let cuts: Vec<usize> = (0..4)
                .filter(|k| pos(c[*k]) != pos(c[(k + 1) % 4]))
                .collect();
            match cuts.len() {
                2 => join(e[cuts[0]], e[cuts[1]]),
                4 => {
                    let mid = (c[0] + c[1] + c[2] + c[3]) / 4.0;
                    if pos(mid) == pos(c[0]) {
                        join(e[0], e[1]);
                        join(e[2], e[3]);
                    } else {
                        join(e[0], e[3]);
                        join(e[1], e[2]);
                    }
                }
                _ => {}
            }
        }
    }
    let mut seen: BTreeSet<Key> = BTreeSet::new();
    let mut out = Vec::new();
    let walk = |start: Key, seen: &mut BTreeSet<Key>| -> (Vec<Key>, bool) {
        let mut path = vec![start];
        seen.insert(start);
        let mut cur = start;
        loop {
            let next = links[&cur].iter().find(|k| !seen.contains(*k)).copied();
            match next {
                Some(k) => {
                    seen.insert(k);
                    path.push(k);
                    cur = k;
                }
                None => {
                    let closed = path.len() > 2 && links[&cur].contains(&start);
                    return (path, closed);
                }
            }
        }
    };
    let ends: Vec<Key> = links
        .iter()
        .filter(|(_, l)| l.len() == 1)
        .map(|(k, _)| *k)
        .collect();
    for k in ends {
        if !seen.contains(&k) {
            let (path, _) = walk(k, &mut seen);
            out.push((path.into_iter().map(point).collect(), false));
        }
    }
    let rest: Vec<Key> = links.keys().copied().collect();
    for k in rest {
        if !seen.contains(&k) {
            let (path, closed) = walk(k, &mut seen);
            out.push((path.into_iter().map(point).collect(), closed));
        }
    }
    out
}

/// The crossing of a curved face with another face.
struct Tracer<'a> {
    model: &'a Model,
    a: Laid,
    b: Other,
    tol: Tol,
}

impl Tracer<'_> {
    fn point(&self, q: DVec2) -> DVec3 {
        self.a.rev.point(q.x, q.y)
    }

    fn f(&self, q: DVec2) -> f64 {
        self.b.value(self.point(q))
    }

    /// Newton steps onto the crossing, shortest in length on the surface.
    fn refine(&self, q: DVec2) -> Option<DVec2> {
        let mut q = q;
        let k = self.a.scale;
        // Small steps for the slope, and the largest step taken at once.
        let s_unit = if self.a.circle {
            1.0
        } else {
            k.x.max(self.tol.dist)
        };
        let (ht, hs) = (1e-7, 1e-7 * s_unit);
        let lim = DVec2::new(0.5, 0.5 * s_unit);
        let close = self.tol.dist * 1e-4;
        for _ in 0..40 {
            let f0 = self.f(q);
            if f0.abs() <= close {
                return Some(q);
            }
            let ft =
                (self.f(q + DVec2::new(ht, 0.0)) - self.f(q - DVec2::new(ht, 0.0))) / (2.0 * ht);
            let fs =
                (self.f(q + DVec2::new(0.0, hs)) - self.f(q - DVec2::new(0.0, hs))) / (2.0 * hs);
            // Lengths of a unit step in θ and in s here.
            let (r, _, dr, dh) = self.a.rev.at(q.y);
            let wt = (r * r).max((k.x * 1e-6).powi(2));
            let ws = (dr * dr + dh * dh).max(f64::MIN_POSITIVE);
            let den = ft * ft / wt + fs * fs / ws;
            if den <= f64::MIN_POSITIVE {
                return None;
            }
            let mut d = DVec2::new(ft / wt, fs / ws) * (-f0 / den);
            let big = (d.x.abs() / lim.x).max(d.y.abs() / lim.y);
            if big > 1.0 {
                d /= big;
            }
            q += d;
        }
        (self.f(q).abs() <= self.tol.dist * 1e-2).then_some(q)
    }

    /// Whether (θ, s) of the crossing is in both faces (outlines count).
    fn ok(&self, q: DVec2) -> bool {
        let d = self.tol.dist * 10.0;
        self.a.holds_2d(q, d) && self.b.holds(self.point(q), d)
    }

    /// Between a place in both faces and one outside, the last place in.
    fn bisect(&self, mut qi: DVec2, mut qo: DVec2) -> DVec2 {
        for _ in 0..60 {
            let Some(m) = self.refine(qi.lerp(qo, 0.5)) else {
                break;
            };
            if self.ok(m) {
                qi = m;
            } else {
                qo = m;
            }
            if self.point(qi).distance(self.point(qo)) < self.tol.dist * 0.1 {
                break;
            }
        }
        qi
    }

    /// A crossing's end, put exactly on the edge of either face it ends at:
    /// on that edge and on the other face's plane or surface.
    fn snap(&self, x: DVec3) -> DVec3 {
        let m = self.model;
        let mine: BTreeSet<EdgeId> = m
            .face(self.a.face)
            .map(|f| f.edge_ids().collect())
            .unwrap_or_default();
        let theirs: BTreeSet<EdgeId> = m
            .face(self.b.face())
            .map(|f| f.edge_ids().collect())
            .unwrap_or_default();
        let best = mine
            .iter()
            .chain(theirs.iter())
            .filter_map(|id| {
                let e = m.edge(*id)?;
                let (pa, pb) = (m.vertex(e.a), m.vertex(e.b));
                let c = e.curve.closest_point(pa, pb, x);
                Some((*id, c.distance(x), c))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        let Some((id, d, c)) = best else { return x };
        if d > self.tol.dist * 1000.0 {
            return x;
        }
        let Some(e) = m.edge(id).cloned() else {
            return x;
        };
        if e.approx {
            return c;
        }
        let (pa, pb) = (m.vertex(e.a), m.vertex(e.b));
        // On my edge, the other face's plane or surface is reached; on
        // theirs, mine.
        let h = |t: f64| {
            let p = e.curve.point(pa, pb, t);
            if mine.contains(&id) {
                self.b.value(p)
            } else {
                self.a.surface.distance(p)
            }
        };
        let t_of = |p: DVec3| match e.curve {
            Curve::Spline(ref s) => s.closest(p),
            Curve::Line => {
                let dd = pb - pa;
                (p - pa).dot(dd) / dd.length_squared().max(f64::MIN_POSITIVE)
            }
            Curve::Arc { .. } => {
                let s = e.curve.sweep(pa, pb);
                if s > 0.0 {
                    e.curve.sweep(pa, p).min(s) / s
                } else {
                    0.0
                }
            }
        };
        // Secant steps along the edge.
        let mut t0 = t_of(c);
        let mut t1 = (t0 + 1e-6).min(1.0);
        if t1 == t0 {
            t1 = t0 - 1e-6;
        }
        let (mut h0, mut h1) = (h(t0), h(t1));
        for _ in 0..50 {
            if h1.abs() <= self.tol.dist * 1e-6 || h1 == h0 {
                break;
            }
            let t2 = (t1 - h1 * (t1 - t0) / (h1 - h0)).clamp(0.0, 1.0);
            t0 = t1;
            h0 = h1;
            t1 = t2;
            h1 = h(t1);
        }
        let p = e.curve.point(pa, pb, t1);
        if h1.abs() <= self.tol.dist * 1e-2 && p.distance(x) < self.tol.dist * 1000.0 {
            p
        } else {
            c
        }
    }

    /// The pieces of the crossing.
    fn pieces(&self) -> Vec<Piece> {
        let (lo, hi) = bounds(&self.a.polys[0]);
        let g = grid_step();
        let k = self.a.scale;
        let step_t = g;
        let step_s = if self.a.circle {
            g
        } else {
            g * k.x.max(self.tol.dist) / k.y
        };
        let count = |w: f64, st: f64| ((w / st).ceil() as usize).clamp(2, 600);
        let (nx, ny) = (count(hi.x - lo.x, step_t), count(hi.y - lo.y, step_s));
        let step = DVec2::new((hi.x - lo.x) / nx as f64, (hi.y - lo.y) / ny as f64);
        // On the same surface (two faces of one torus, say): no crossing,
        // only touching everywhere.
        let probe = (0..5)
            .flat_map(|i| (0..5).map(move |j| (i, j)))
            .all(|(i, j)| {
                let q = lo + DVec2::new((hi.x - lo.x) * i as f64, (hi.y - lo.y) * j as f64) / 4.0;
                self.f(q).abs() <= self.tol.dist
            });
        if probe {
            return Vec::new();
        }
        // Half a step past the face on every side.
        let start = lo - step * 0.5;
        let f = |q: DVec2| self.f(q);
        let mut out = Vec::new();
        for (line, closed) in contours(&f, start, step, nx + 1, ny + 1) {
            let pts: Vec<DVec2> = line.into_iter().filter_map(|q| self.refine(q)).collect();
            if pts.len() < 2 {
                continue;
            }
            for run in self.runs(pts, closed) {
                out.extend(self.shape(run));
            }
        }
        out
    }

    /// Cut a traced line into the runs inside both faces, each as places
    /// with its ends on the faces' outlines; and whether a run closes.
    fn runs(&self, mut pts: Vec<DVec2>, closed: bool) -> Vec<(Vec<DVec3>, bool)> {
        let flags: Vec<bool> = pts.iter().map(|q| self.ok(*q)).collect();
        if closed {
            match flags.iter().position(|x| !x) {
                None => {
                    let p: Vec<DVec3> = pts.iter().map(|q| self.point(*q)).collect();
                    return vec![(p, true)];
                }
                Some(i) => pts.rotate_left(i),
            }
            pts.push(pts[0]);
        }
        let flags: Vec<bool> = pts.iter().map(|q| self.ok(*q)).collect();
        let mut out = Vec::new();
        let mut i = 0;
        while i < pts.len() {
            if !flags[i] {
                i += 1;
                continue;
            }
            let mut run: Vec<DVec3> = Vec::new();
            if i > 0 {
                let b = self.bisect(pts[i], pts[i - 1]);
                run.push(self.snap(self.point(b)));
            }
            let mut j = i;
            while j < pts.len() && flags[j] {
                run.push(self.point(pts[j]));
                j += 1;
            }
            if j < pts.len() {
                let b = self.bisect(pts[j - 1], pts[j]);
                run.push(self.snap(self.point(b)));
            }
            out.push((run, false));
            i = j;
        }
        out
    }

    /// A run as one or two pieces (a closed run in two halves): a line, a
    /// circle's arc, or fine segments.
    fn shape(&self, (run, closed): (Vec<DVec3>, bool)) -> Vec<Piece> {
        let mut run = run;
        // Drop repeats (an end snapped onto its neighbour).
        run.dedup_by(|a, b| a.distance(*b) <= self.tol.dist);
        if run.len() < 2 {
            return Vec::new();
        }
        let halves: Vec<Vec<DVec3>> = if closed {
            let n = run.len();
            if n < 4 {
                return Vec::new();
            }
            let h = n / 2;
            let mut second = run[h..].to_vec();
            second.push(run[0]);
            vec![run[..=h].to_vec(), second]
        } else {
            vec![run]
        };
        let mut out = Vec::new();
        for mut pts in halves {
            // Only its ends traced: put a point between them on the
            // crossing, so a short arc is not taken for a line.
            if pts.len() == 2
                && let (Some(qa), Some(qb)) = (
                    self.a.place(pts[0], f64::INFINITY),
                    self.a.place(pts[1], f64::INFINITY),
                )
                && let Some(m) = self.refine(qa.lerp(qb, 0.5))
            {
                pts.insert(1, self.point(m));
            }
            let len: f64 = pts.windows(2).map(|w| w[0].distance(w[1])).sum();
            if len <= self.tol.dist * 10.0 {
                continue;
            }
            // Along both faces' outlines: the edge they share.
            let mid = pts[pts.len() / 2];
            let d = self.tol.dist * 10.0;
            let on_a = self
                .a
                .place(mid, d)
                .is_some_and(|q| self.a.near_outline(q, d));
            if (on_a && self.b.on_outline(mid, d)) || self.along_shared(mid) {
                continue;
            }
            out.push(self.fit(pts));
        }
        out
    }

    /// Whether `p` is on an edge both faces have: the crossing is that
    /// edge. Fine segments are judged by the curve they stand for, so
    /// within the chord rule's stray of their chord.
    fn along_shared(&self, p: DVec3) -> bool {
        let m = self.model;
        let edges = |f: FaceId| -> BTreeSet<EdgeId> {
            m.face(f)
                .map(|f| f.edge_ids().collect())
                .unwrap_or_default()
        };
        let theirs = edges(self.b.face());
        let stray = self
            .a
            .place(p, f64::INFINITY)
            .map_or(self.a.scale.x, |q| self.a.size_at(q))
            * 2e-3;
        edges(self.a.face)
            .intersection(&theirs)
            .filter_map(|id| m.edge(*id))
            .any(|e| {
                let (pa, pb) = (m.vertex(e.a), m.vertex(e.b));
                let d = e.curve.closest_point(pa, pb, p).distance(p);
                d <= self.tol.dist * 10.0 || (e.approx && d <= stray)
            })
    }

    fn fit(&self, pts: Vec<DVec3>) -> Piece {
        let eps = self.tol.dist;
        let (p, q) = (pts[0], pts[pts.len() - 1]);
        let line = p.distance(q) > eps
            && pts
                .iter()
                .all(|x| Curve::Line.closest_point(p, q, *x).distance(*x) <= eps);
        if line {
            return Piece {
                shape: Shape::Line,
                points: vec![p, q],
            };
        }
        if pts.len() >= 3
            && let Some(spec) = arc_through(p, pts[pts.len() / 2], q, eps)
            && pts
                .iter()
                .all(|x| circle_distance(spec.centre, spec.normal, spec.radius, *x) <= eps)
        {
            return Piece {
                shape: Shape::Arc {
                    centre: spec.centre,
                    normal: spec.normal,
                    radius: spec.radius,
                    sweep: spec.sweep,
                },
                points: vec![p, q],
            };
        }
        // Fine segments: the chord rule on the traced points.
        let size = pts
            .iter()
            .map(|x| {
                let s = self
                    .a
                    .place(*x, f64::INFINITY)
                    .map_or(self.a.scale.x, |qq| self.a.size_at(qq));
                s.min(self.b.size_at(*x))
            })
            .fold(f64::INFINITY, f64::min);
        let dev = (size * 1e-3).max(eps);
        let mut keep = vec![false; pts.len()];
        keep[0] = true;
        keep[pts.len() - 1] = true;
        simplify(&pts, 0, pts.len() - 1, dev, &mut keep);
        Piece {
            shape: Shape::Approx,
            points: pts
                .into_iter()
                .zip(keep)
                .filter(|(_, k)| *k)
                .map(|(x, _)| x)
                .collect(),
        }
    }
}

/// Douglas–Peucker: keep the points a chord would stray from by more
/// than `dev`.
fn simplify(pts: &[DVec3], i: usize, j: usize, dev: f64, keep: &mut [bool]) {
    if j <= i + 1 {
        return;
    }
    let (a, b) = (pts[i], pts[j]);
    let (k, d) = (i + 1..j)
        .map(|k| (k, Curve::Line.closest_point(a, b, pts[k]).distance(pts[k])))
        .max_by(|x, y| x.1.total_cmp(&y.1))
        .unwrap_or((i, 0.0));
    if d > dev {
        keep[k] = true;
        simplify(pts, i, k, dev, keep);
        simplify(pts, k, j, dev, keep);
    }
}

/// A face's box (a curved face's surface sampled, so its bulge is in).
fn face_box(m: &Model, id: FaceId) -> Option<(DVec3, DVec3)> {
    crate::cut::face_box(m, m.face(id)?)
}

/// Where curved face `a` crosses face `b` (flat or curved).
pub fn crossings_with(m: &Model, a: FaceId, b: FaceId) -> Vec<Piece> {
    if a == b {
        return Vec::new();
    }
    let (Some(ba), Some(bb)) = (face_box(m, a), face_box(m, b)) else {
        return Vec::new();
    };
    if ba.1.cmplt(bb.0).any() || bb.1.cmplt(ba.0).any() {
        return Vec::new();
    }
    let Some(laid) = Laid::new(m, a) else {
        return Vec::new();
    };
    let Some(fb) = m.face(b) else {
        return Vec::new();
    };
    let other = match fb.plane() {
        Some(pl) => match Flat::drawn(m, b) {
            Some(f) => Other::Flat(b, f, *pl),
            None => return Vec::new(),
        },
        None => match Laid::new(m, b) {
            Some(l) => Other::Curved(l),
            None => return Vec::new(),
        },
    };
    Tracer {
        model: m,
        a: laid,
        b: other,
        tol: m.tol(),
    }
    .pieces()
}

/// Every crossing that involves a curved face (or only those with a face
/// in `only`).
pub fn curved_crossings(m: &Model, only: Option<&BTreeSet<FaceId>>) -> Vec<Piece> {
    let curved: Vec<FaceId> = m
        .faces()
        .filter(|(_, f)| f.is_curved())
        .map(|(id, _)| id)
        .collect();
    let all: Vec<FaceId> = m.faces().map(|(id, _)| id).collect();
    let mut out = Vec::new();
    for &a in &curved {
        for &b in &all {
            let b_curved = m.face(b).is_some_and(|f| f.is_curved());
            // Each pair of curved faces once.
            if b == a || (b_curved && b < a) {
                continue;
            }
            if let Some(s) = only
                && !s.contains(&a)
                && !s.contains(&b)
            {
                continue;
            }
            out.extend(crossings_with(m, a, b));
        }
    }
    out
}

/// Add a crossing to the model as edges: a line or an arc as drawing one
/// does (cutting what it crosses and splitting flat faces), fine segments
/// marked approximate and sharing a curve id. Returns the edges added.
pub fn add_piece(m: &mut Model, p: &Piece) -> Vec<EdgeId> {
    match p.shape {
        Shape::Line => crate::add_segment(m, p.start(), p.end()).edges,
        Shape::Arc {
            centre,
            normal,
            radius,
            sweep,
        } => crate::add_arc(m, centre, normal, radius, p.start(), sweep).edges,
        Shape::Approx => {
            let tol = crate::sticky::tol_with(m, &p.points);
            let flat = Plane::fit(&p.points, &tol)
                .filter(|pl| p.points.iter().all(|x| pl.contains(*x, &tol)));
            let mut edges = Vec::new();
            if flat.is_some() || p.points.len() == 2 {
                for w in p.points.windows(2) {
                    edges.extend(crate::add_segment(m, w[0], w[1]).edges);
                }
            } else {
                let n = p.points.len();
                let vs: Vec<_> = p
                    .points
                    .iter()
                    .enumerate()
                    .map(|(i, x)| {
                        if i == 0 || i == n - 1 {
                            crate::insert_point(m, *x, &tol)
                        } else {
                            m.add_vertex(*x)
                        }
                    })
                    .collect();
                for w in vs.windows(2) {
                    if w[0] != w[1]
                        && let Some(e) = m.add_edge(w[0], w[1])
                    {
                        edges.push(e);
                    }
                }
            }
            let cid = m.new_curve_id();
            for e in &edges {
                m.set_approx(*e, true);
                m.set_curve_id(*e, Some(cid));
            }
            edges
        }
    }
}
