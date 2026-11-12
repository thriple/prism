//! NURBS surfaces (Freeform NURBS part 2). A face on one is a [`Patch`]:
//! the shared surface, the rectangle of it the face covers, and the side
//! it faces.

use std::sync::Arc;

use prism_math::{DAffine3, DVec3};
use serde::{Deserialize, Serialize};

use crate::SEGMENTS_PER_TURN;
use crate::nurbs::{H, Nurbs, basis, de_boor, knots_ok, span};

/// A NURBS surface: a net of control points, `count[0]` along u by
/// `count[1]` along v, stored row by row along u (`points[i * count[1] + j]`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NurbsSurface {
    degree: [usize; 2],
    knots: [Vec<f64>; 2],
    count: [usize; 2],
    points: Vec<DVec3>,
    weights: Vec<f64>,
    /// Closed and smooth round that direction: the last `degree` rows (or
    /// columns) repeat the first.
    #[serde(default)]
    periodic: [bool; 2],
}

impl NurbsSurface {
    /// A checked surface. None if the parts do not fit together.
    pub fn new(
        degree: [usize; 2],
        knots: [Vec<f64>; 2],
        count: [usize; 2],
        points: Vec<DVec3>,
        weights: Vec<f64>,
        periodic: [bool; 2],
    ) -> Option<NurbsSurface> {
        let ok = (0..2).all(|d| knots_ok(degree[d], &knots[d], count[d]))
            && points.len() == count[0] * count[1]
            && weights.len() == points.len()
            && points.iter().all(|p| p.is_finite())
            && weights.iter().all(|w| w.is_finite() && *w > 0.0)
            && (0..2).all(|d| !periodic[d] || count[d] > 2 * degree[d]);
        let s = NurbsSurface {
            degree,
            knots,
            count,
            points,
            weights,
            periodic,
        };
        (ok && s.repeats_hold()).then_some(s)
    }

    /// Whether a periodic direction's last rows repeat its first.
    fn repeats_hold(&self) -> bool {
        let [nu, nv] = self.count;
        let same = |a: usize, b: usize| {
            self.points[a] == self.points[b] && self.weights[a] == self.weights[b]
        };
        let u_ok = !self.periodic[0]
            || (0..self.degree[0])
                .all(|i| (0..nv).all(|j| same(i * nv + j, (nu - self.degree[0] + i) * nv + j)));
        let v_ok = !self.periodic[1]
            || (0..self.degree[1])
                .all(|j| (0..nu).all(|i| same(i * nv + j, i * nv + nv - self.degree[1] + j)));
        u_ok && v_ok
    }

    /// The surface swept by `curve` (its u) moved along `shift` (its v,
    /// from 0 to 1).
    pub fn extrusion(curve: &Nurbs, shift: DVec3) -> Option<NurbsSurface> {
        let n = curve.points().len();
        let points = curve
            .points()
            .iter()
            .flat_map(|p| [*p, *p + shift])
            .collect();
        let weights = curve.weights().iter().flat_map(|w| [*w, *w]).collect();
        NurbsSurface::new(
            [curve.degree(), 1],
            [curve.knots().to_vec(), vec![0.0, 0.0, 1.0, 1.0]],
            [n, 2],
            points,
            weights,
            [curve.is_periodic(), false],
        )
    }

    pub fn degree(&self) -> [usize; 2] {
        self.degree
    }

    pub fn knots(&self) -> [&[f64]; 2] {
        [&self.knots[0], &self.knots[1]]
    }

    /// How many control points along u and v, a periodic direction's
    /// repeats included.
    pub fn count(&self) -> [usize; 2] {
        self.count
    }

    pub fn points(&self) -> &[DVec3] {
        &self.points
    }

    pub fn weights(&self) -> &[f64] {
        &self.weights
    }

    pub fn periodic(&self) -> [bool; 2] {
        self.periodic
    }

    /// How many control points along u and v can be moved on their own.
    pub fn cage(&self) -> [usize; 2] {
        [0, 1].map(|d| {
            if self.periodic[d] {
                self.count[d] - self.degree[d]
            } else {
                self.count[d]
            }
        })
    }

    pub fn control(&self, i: usize, j: usize) -> DVec3 {
        self.points[i * self.count[1] + j]
    }

    /// Move control point (`i`, `j`) of the cage, and its repeats.
    pub fn set_control(&mut self, i: usize, j: usize, p: DVec3) {
        let [cu, cv] = self.cage();
        if i >= cu || j >= cv {
            return;
        }
        let nv = self.count[1];
        let mut is = vec![i];
        if self.periodic[0] && i < self.degree[0] {
            is.push(cu + i);
        }
        let mut js = vec![j];
        if self.periodic[1] && j < self.degree[1] {
            js.push(cv + j);
        }
        for &a in &is {
            for &b in &js {
                self.points[a * nv + b] = p;
            }
        }
    }

    /// The first and last parameter along u and along v.
    pub fn domain(&self) -> [(f64, f64); 2] {
        [0, 1].map(|d| (self.knots[d][self.degree[d]], self.knots[d][self.count[d]]))
    }

    /// Whether direction `d` comes back to where it starts.
    pub fn is_closed(&self, d: usize) -> bool {
        self.periodic[d]
    }

    fn wrap(&self, d: usize, t: f64) -> f64 {
        let (lo, hi) = self.domain()[d];
        if (lo..=hi).contains(&t) {
            t
        } else if self.periodic[d] {
            lo + (t - lo).rem_euclid(hi - lo)
        } else {
            t.clamp(lo, hi)
        }
    }

    fn at(&self, i: usize, j: usize) -> H {
        let k = i * self.count[1] + j;
        H::of(self.points[k], self.weights[k])
    }

    /// The weighted value and its two derivatives.
    fn weighted(&self, u: f64, v: f64) -> (H, H, H) {
        let (u, v) = (self.wrap(0, u), self.wrap(1, v));
        let [p, q] = self.degree;
        let ku = span(p, &self.knots[0], self.count[0], u);
        let kv = span(q, &self.knots[1], self.count[1], v);
        // Each of the rows that matter, collapsed along v.
        let mut rows = [(H::default(), H::default()); crate::MAX_DEGREE + 1];
        for (r, row) in rows.iter_mut().enumerate().take(p + 1) {
            let i = ku - p + r;
            *row = de_boor(q, &self.knots[1], kv, v, |j| self.at(i, j));
        }
        let (s, du) = de_boor(p, &self.knots[0], ku, u, |i| rows[i + p - ku].0);
        let (dv, _) = de_boor(p, &self.knots[0], ku, u, |i| rows[i + p - ku].1);
        (s, du, dv)
    }

    /// The point at (`u`, `v`).
    pub fn eval(&self, u: f64, v: f64) -> DVec3 {
        let (s, _, _) = self.weighted(u, v);
        s.p / s.w
    }

    /// The point and the derivatives along u and v.
    pub fn eval_d(&self, u: f64, v: f64) -> (DVec3, DVec3, DVec3) {
        let (s, du, dv) = self.weighted(u, v);
        let (p, a) = s.split(du);
        let (_, b) = s.split(dv);
        (p, a, b)
    }

    /// The unit normal at (`u`, `v`): along u crossed with along v. Zero
    /// where the surface comes to a point.
    pub fn normal(&self, u: f64, v: f64) -> DVec3 {
        let (_, a, b) = self.eval_d(u, v);
        let n = a.cross(b).normalize_or_zero();
        if n != DVec3::ZERO {
            return n;
        }
        // At a pole: the normal just inside.
        let [(u0, u1), (v0, v1)] = self.domain();
        let (cu, cv) = ((u0 + u1) / 2.0, (v0 + v1) / 2.0);
        let (_, a, b) = self.eval_d(u + (cu - u) * 1e-6, v + (cv - v) * 1e-6);
        a.cross(b).normalize_or_zero()
    }

    /// The surface after `m`. Any affine map keeps a NURBS surface exact.
    pub fn transformed(&self, m: &DAffine3) -> NurbsSurface {
        let mut s = self.clone();
        for p in &mut s.points {
            *p = m.transform_point3(*p);
        }
        s
    }

    /// The surface with every point sent through `f`.
    pub fn mapped(&self, f: impl Fn(DVec3) -> DVec3) -> NurbsSurface {
        let mut s = self.clone();
        for p in &mut s.points {
            *p = f(*p);
        }
        s
    }

    /// The curve along direction `d` where the other parameter is `t`:
    /// along u at v = `t` for `d` 0, along v at u = `t` for `d` 1.
    pub fn iso(&self, d: usize, t: f64) -> Option<Nurbs> {
        let o = 1 - d;
        let t = self.wrap(o, t);
        let q = self.degree[o];
        let k = span(q, &self.knots[o], self.count[o], t);
        let b = basis(q, &self.knots[o], k, t);
        let pts: Vec<H> = (0..self.count[d])
            .map(|a| {
                let mut s = H::default();
                for (r, w) in b.iter().enumerate().take(q + 1) {
                    let c = k - q + r;
                    let h = if d == 0 { self.at(a, c) } else { self.at(c, a) };
                    s.p += h.p * *w;
                    s.w += h.w * *w;
                }
                s
            })
            .collect();
        Nurbs::new(
            self.degree[d],
            self.knots[d].clone(),
            pts.iter().map(|h| h.p / h.w).collect(),
            pts.iter().map(|h| h.w).collect(),
            self.periodic[d],
        )
    }

    /// The same surface with one more knot at `t` along direction `d` (so
    /// one more row of control points). None for a periodic direction.
    pub fn insert_knot(&self, d: usize, t: f64) -> Option<NurbsSurface> {
        if self.periodic[d] {
            return None;
        }
        let o = 1 - d;
        let lines: Vec<Nurbs> = (0..self.count[o])
            .map(|c| {
                let (pts, ws): (Vec<DVec3>, Vec<f64>) = (0..self.count[d])
                    .map(|a| {
                        let k = if d == 0 {
                            a * self.count[1] + c
                        } else {
                            c * self.count[1] + a
                        };
                        (self.points[k], self.weights[k])
                    })
                    .unzip();
                Nurbs::new(self.degree[d], self.knots[d].clone(), pts, ws, false)?.insert_knot(t)
            })
            .collect::<Option<_>>()?;
        let n = self.count[d] + 1;
        let mut count = self.count;
        count[d] = n;
        let mut knots = self.knots.clone();
        knots[d] = lines[0].knots().to_vec();
        let mut points = vec![DVec3::ZERO; count[0] * count[1]];
        let mut weights = vec![1.0; points.len()];
        for (c, line) in lines.iter().enumerate() {
            for a in 0..n {
                let k = if d == 0 {
                    a * count[1] + c
                } else {
                    c * count[1] + a
                };
                points[k] = line.points()[a];
                weights[k] = line.weights()[a];
            }
        }
        NurbsSurface::new(self.degree, knots, count, points, weights, self.periodic)
    }

    /// The (u, v) inside `range` whose point is closest to `p`.
    pub fn closest(&self, p: DVec3, range: [(f64, f64); 2]) -> (f64, f64) {
        let dom = self.domain();
        let n = [0, 1].map(|d| {
            let spans = (self.count[d] - self.degree[d]) as f64;
            let part = ((range[d].1 - range[d].0) / (dom[d].1 - dom[d].0)).abs();
            ((spans * part * (self.degree[d] + 1) as f64).ceil() as usize).clamp(4, 64)
        });
        let at =
            |d: usize, i: usize| range[d].0 + (range[d].1 - range[d].0) * i as f64 / n[d] as f64;
        let mut best = (f64::INFINITY, 0, 0);
        for i in 0..=n[0] {
            for j in 0..=n[1] {
                let d = self.eval(at(0, i), at(1, j)).distance_squared(p);
                if d < best.0 {
                    best = (d, i, j);
                }
            }
        }
        let lo = [range[0].0.min(range[0].1), range[1].0.min(range[1].1)];
        let hi = [range[0].0.max(range[0].1), range[1].0.max(range[1].1)];
        let (mut u, mut v) = (at(0, best.1), at(1, best.2));
        for _ in 0..30 {
            let (s, a, b) = self.eval_d(u, v);
            let r = p - s;
            let (aa, ab, bb) = (a.dot(a), a.dot(b), b.dot(b));
            let det = aa * bb - ab * ab;
            let (ra, rb) = (r.dot(a), r.dot(b));
            let (du, dv) = if det.abs() > 1e-14 * aa.max(bb).powi(2) {
                ((ra * bb - rb * ab) / det, (rb * aa - ra * ab) / det)
            } else if aa > 0.0 {
                (ra / aa, 0.0)
            } else if bb > 0.0 {
                (0.0, rb / bb)
            } else {
                break;
            };
            let (nu, nv) = ((u + du).clamp(lo[0], hi[0]), (v + dv).clamp(lo[1], hi[1]));
            let moved =
                (nu - u).abs() / (dom[0].1 - dom[0].0) + (nv - v).abs() / (dom[1].1 - dom[1].0);
            u = nu;
            v = nv;
            if moved <= 1e-14 {
                break;
            }
        }
        (u, v)
    }

    /// The parameters along direction `d` to draw the surface with, between
    /// `lo` and `hi`: every knot, and between knots as many steps as the
    /// surface bends there (as fine as an arc that turns as much).
    pub fn grid(&self, d: usize, lo: f64, hi: f64, other: (f64, f64)) -> Vec<f64> {
        let (a, b) = (lo.min(hi), lo.max(hi));
        let dom = self.domain()[d];
        let period = dom.1 - dom.0;
        // The knots inside, a closed direction's repeated past its end.
        let mut cuts = vec![a];
        let turns = if self.periodic[d] { 1 } else { 0 };
        for turn in 0..=turns {
            for k in self.degree[d]..=self.count[d] {
                let t = self.knots[d][k] + period * turn as f64;
                if t > a && t < b && cuts.last().is_none_or(|l| t > *l) {
                    cuts.push(t);
                }
            }
        }
        cuts.push(b);
        cuts.sort_by(f64::total_cmp);
        cuts.dedup();
        let dir = |t: f64, s: f64| {
            let (_, du, dv) = if d == 0 {
                self.eval_d(t, s)
            } else {
                self.eval_d(s, t)
            };
            (if d == 0 { du } else { dv }).normalize_or_zero()
        };
        let mut out = vec![a];
        for w in cuts.windows(2) {
            let mut n = 1usize;
            if self.degree[d] > 1 {
                // The most it turns across this span, at a few places
                // along the other direction.
                let sub = self.degree[d] + 1;
                let mut most: f64 = 0.0;
                for i in 0..=4 {
                    let s = other.0 + (other.1 - other.0) * i as f64 / 4.0;
                    let mut last = dir(w[0], s);
                    let mut sum = 0.0;
                    for k in 1..=sub {
                        let x = dir(w[0] + (w[1] - w[0]) * k as f64 / sub as f64, s);
                        sum += last.dot(x).clamp(-1.0, 1.0).acos();
                        last = x;
                    }
                    most = most.max(sum);
                }
                n = ((most / std::f64::consts::TAU * SEGMENTS_PER_TURN as f64).ceil() as usize)
                    .clamp(2, 96);
            }
            for k in 1..=n {
                out.push(w[0] + (w[1] - w[0]) * k as f64 / n as f64);
            }
        }
        if hi < lo {
            out.reverse();
        }
        out
    }
}

/// The part of a NURBS surface a face covers, and the side it faces.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Patch {
    pub surface: Arc<NurbsSurface>,
    /// From and to along u.
    pub u: (f64, f64),
    /// From and to along v.
    pub v: (f64, f64),
    /// Facing against the surface's own normal (along u crossed with
    /// along v).
    pub flipped: bool,
}

impl Patch {
    /// All of a surface.
    pub fn whole(surface: NurbsSurface) -> Patch {
        let [u, v] = surface.domain();
        Patch {
            surface: Arc::new(surface),
            u,
            v,
            flipped: false,
        }
    }

    pub fn range(&self) -> [(f64, f64); 2] {
        [self.u, self.v]
    }

    /// The (u, v) of the patch's point closest to `p`.
    pub fn param(&self, p: DVec3) -> (f64, f64) {
        self.surface.closest(p, self.range())
    }

    pub fn project(&self, p: DVec3) -> DVec3 {
        let (u, v) = self.param(p);
        self.surface.eval(u, v)
    }

    /// Which way the face points at (`u`, `v`).
    pub fn normal(&self, u: f64, v: f64) -> DVec3 {
        let n = self.surface.normal(u, v);
        if self.flipped { -n } else { n }
    }

    pub fn normal_at(&self, p: DVec3) -> DVec3 {
        let (u, v) = self.param(p);
        self.normal(u, v)
    }

    /// How far `p` is from the patch, signed by the face's side.
    pub fn distance(&self, p: DVec3) -> f64 {
        let (u, v) = self.param(p);
        let d = p - self.surface.eval(u, v);
        d.length()
            * if d.dot(self.normal(u, v)) < 0.0 {
                -1.0
            } else {
                1.0
            }
    }

    pub fn flipped(&self) -> Patch {
        Patch {
            flipped: !self.flipped,
            ..self.clone()
        }
    }

    /// The patch after `m`; a mirror turns the face's loops round, so it
    /// then faces the other way along the surface's own normal.
    pub fn transformed(&self, m: &DAffine3) -> Patch {
        Patch {
            surface: Arc::new(self.surface.transformed(m)),
            flipped: self.flipped != (m.matrix3.determinant() < 0.0),
            ..self.clone()
        }
    }

    /// One of the patch's four sides as the curve an edge follows: 0 along
    /// u at the start of v, 1 along v at the end of u, 2 back along u at
    /// the end of v, 3 back along v at the start of u.
    pub fn side(&self, k: usize) -> Option<crate::Spline> {
        let (d, at, from, to) = match k {
            0 => (0, self.v.0, self.u.0, self.u.1),
            1 => (1, self.u.1, self.v.0, self.v.1),
            2 => (0, self.v.1, self.u.1, self.u.0),
            _ => (1, self.u.0, self.v.1, self.v.0),
        };
        Some(crate::Spline {
            curve: Arc::new(self.surface.iso(d, at)?),
            t0: from,
            t1: to,
        })
    }

    /// The patch's four corners, in the order its sides join them.
    pub fn corners(&self) -> [DVec3; 4] {
        [
            self.surface.eval(self.u.0, self.v.0),
            self.surface.eval(self.u.1, self.v.0),
            self.surface.eval(self.u.1, self.v.1),
            self.surface.eval(self.u.0, self.v.1),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wave() -> Nurbs {
        Nurbs::open(
            &[
                DVec3::ZERO,
                DVec3::new(1.0, 2.0, 0.0),
                DVec3::new(3.0, -2.0, 0.0),
                DVec3::new(4.0, 0.0, 0.0),
            ],
            3,
        )
        .unwrap()
    }

    #[test]
    fn an_extrusion_follows_its_curve() {
        let c = wave();
        let s = NurbsSurface::extrusion(&c, DVec3::new(0.0, 0.0, 2.0)).unwrap();
        for i in 0..=10 {
            let u = i as f64 / 10.0;
            let (p, du, dv) = s.eval_d(u, 0.25);
            assert!(p.distance(c.eval(u) + DVec3::new(0.0, 0.0, 0.5)) < 1e-13);
            assert!(du.distance(c.eval_d(u).1) < 1e-12);
            assert!(dv.distance(DVec3::new(0.0, 0.0, 2.0)) < 1e-12);
            assert!(s.normal(u, 0.25).z.abs() < 1e-12);
        }
        // Its sides are the curve, a line up, the curve back, a line down.
        let patch = Patch::whole(s);
        let top = patch.side(2).unwrap();
        assert!(
            top.point(0.0)
                .distance(c.eval(1.0) + DVec3::new(0.0, 0.0, 2.0))
                < 1e-13
        );
        assert!(top.point(1.0).distance(DVec3::new(0.0, 0.0, 2.0)) < 1e-13);
        assert!(patch.corners()[2].distance(DVec3::new(4.0, 0.0, 2.0)) < 1e-13);
    }

    #[test]
    fn the_closest_point_and_the_signed_distance() {
        let patch =
            Patch::whole(NurbsSurface::extrusion(&wave(), DVec3::new(0.0, 0.0, 2.0)).unwrap());
        let (u, v) = (0.3, 0.6);
        let on = patch.surface.eval(u, v);
        let n = patch.normal(u, v);
        let p = on + n * 0.05;
        let (cu, cv) = patch.param(p);
        assert!((cu - u).abs() < 1e-8 && (cv - v).abs() < 1e-8, "{cu} {cv}");
        assert!((patch.distance(p) - 0.05).abs() < 1e-9);
        assert!((patch.flipped().distance(p) + 0.05).abs() < 1e-9);
        // A mirror: the face still points the way the mirrored normal does.
        let m = DAffine3::from_scale(DVec3::new(-1.0, 1.0, 1.0));
        let q = patch.transformed(&m);
        let mirrored = DVec3::new(-n.x, n.y, n.z);
        assert!(q.normal(u, v).distance(mirrored) < 1e-9);
    }

    #[test]
    fn inserting_a_row_keeps_the_shape_and_the_grid_follows_the_bend() {
        let s = NurbsSurface::extrusion(&wave(), DVec3::new(0.0, 0.5, 2.0)).unwrap();
        let t = s.insert_knot(0, 0.4).unwrap();
        assert_eq!(t.count(), [5, 2]);
        for i in 0..=8 {
            for j in 0..=4 {
                let (u, v) = (i as f64 / 8.0, j as f64 / 4.0);
                assert!(s.eval(u, v).distance(t.eval(u, v)) < 1e-13);
            }
        }
        assert_eq!(
            s.grid(1, 0.0, 1.0, (0.0, 1.0)),
            vec![0.0, 1.0],
            "straight along v"
        );
        assert!(s.grid(0, 0.0, 1.0, (0.0, 1.0)).len() > 8, "curved along u");
    }

    #[test]
    fn a_closed_curve_makes_a_tube_without_a_seam() {
        let ring = Nurbs::closed(
            &[
                DVec3::ZERO,
                DVec3::new(2.0, 0.0, 0.0),
                DVec3::new(2.0, 2.0, 0.0),
                DVec3::new(0.0, 2.0, 0.0),
            ],
            3,
        )
        .unwrap();
        let s = NurbsSurface::extrusion(&ring, DVec3::Z).unwrap();
        assert!(s.is_closed(0) && !s.is_closed(1));
        assert_eq!(s.cage(), [4, 2]);
        assert!(s.eval(0.0, 0.5).distance(s.eval(1.0, 0.5)) < 1e-12);
        assert!(s.normal(0.0, 0.5).distance(s.normal(1.0 - 1e-12, 0.5)) < 1e-6);
        // A part that passes the seam.
        assert!(s.eval(1.25, 0.5).distance(s.eval(0.25, 0.5)) < 1e-12);
        assert!(s.grid(0, 0.75, 1.25, (0.0, 1.0)).len() > 3);
    }
}
