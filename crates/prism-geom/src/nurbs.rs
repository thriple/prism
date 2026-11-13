//! NURBS curves (Freeform NURBS part 1). Prism owns the type, its
//! evaluation and its file layout; the hard constructions (skinning,
//! fitting) go through the kernel (PLAN 2.3).
//!
//! A curve is its degree, knots, control points and weights. A closed
//! smooth curve is periodic: its last `degree` control points repeat its
//! first, and its knots are evenly spaced, so it has no corner at the seam.

use std::sync::Arc;

use prism_math::{DAffine3, DVec3, Plane, newell};
use serde::{Deserialize, Serialize};

use crate::SEGMENTS_PER_TURN;

/// The highest degree a curve or surface may have.
pub const MAX_DEGREE: usize = 7;

/// A weighted point: `p` is the point times its weight.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct H {
    pub p: DVec3,
    pub w: f64,
}

impl H {
    pub fn of(p: DVec3, w: f64) -> H {
        H { p: p * w, w }
    }
    fn lerp(self, o: H, t: f64) -> H {
        H {
            p: self.p.lerp(o.p, t),
            w: self.w + (o.w - self.w) * t,
        }
    }
    fn diff(self, o: H, k: f64) -> H {
        H {
            p: (self.p - o.p) * k,
            w: (self.w - o.w) * k,
        }
    }
    /// The point, and the derivative given this value's derivative `d`.
    pub fn split(self, d: H) -> (DVec3, DVec3) {
        let c = self.p / self.w;
        (c, (d.p - c * d.w) / self.w)
    }
}

/// The knot span holding `t`: `k` with `knots[k] <= t < knots[k + 1]`,
/// kept inside the curve (`n` control points).
pub(crate) fn span(p: usize, knots: &[f64], n: usize, t: f64) -> usize {
    let mut k = p;
    while k + 1 < n && t >= knots[k + 1] {
        k += 1;
    }
    k
}

/// de Boor's algorithm in span `k`: the value and the derivative, both
/// weighted. `ctrl(i)` is control point `i`.
pub(crate) fn de_boor(
    p: usize,
    knots: &[f64],
    k: usize,
    t: f64,
    ctrl: impl Fn(usize) -> H,
) -> (H, H) {
    let mut d = [H::default(); MAX_DEGREE + 1];
    for (j, x) in d.iter_mut().enumerate().take(p + 1) {
        *x = ctrl(k - p + j);
    }
    let mut der = H::default();
    for r in 1..=p {
        if r == p {
            der = d[p].diff(d[p - 1], p as f64 / (knots[k + 1] - knots[k]));
        }
        for j in (r..=p).rev() {
            let i = k - p + j;
            let den = knots[i + p - r + 1] - knots[i];
            let a = if den > 0.0 { (t - knots[i]) / den } else { 0.0 };
            d[j] = d[j - 1].lerp(d[j], a);
        }
    }
    (d[p], der)
}

/// The `p + 1` basis functions that are not zero in span `k`, at `t`:
/// entry `r` belongs to control point `k - p + r`.
pub(crate) fn basis(p: usize, knots: &[f64], k: usize, t: f64) -> [f64; MAX_DEGREE + 1] {
    let mut n = [0.0; MAX_DEGREE + 1];
    let mut left = [0.0; MAX_DEGREE + 1];
    let mut right = [0.0; MAX_DEGREE + 1];
    n[0] = 1.0;
    for j in 1..=p {
        left[j] = t - knots[k + 1 - j];
        right[j] = knots[k + j] - t;
        let mut saved = 0.0;
        for r in 0..j {
            let den = right[r + 1] + left[j - r];
            let temp = if den > 0.0 { n[r] / den } else { 0.0 };
            n[r] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }
        n[j] = saved;
    }
    n
}

/// Whether `knots` suit `n` control points of degree `p`.
pub(crate) fn knots_ok(p: usize, knots: &[f64], n: usize) -> bool {
    (1..=MAX_DEGREE).contains(&p)
        && n > p
        && knots.len() == n + p + 1
        && knots.iter().all(|k| k.is_finite())
        && knots.windows(2).all(|w| w[0] <= w[1])
        && knots[p] < knots[n]
        && knots.windows(p + 2).all(|w| w[0] < w[p + 1])
}

/// Clamped, evenly spaced knots over 0 to 1.
pub(crate) fn clamped_knots(p: usize, n: usize) -> Vec<f64> {
    let spans = n - p;
    (0..n + p + 1)
        .map(|i| {
            if i <= p {
                0.0
            } else if i >= n {
                1.0
            } else {
                (i - p) as f64 / spans as f64
            }
        })
        .collect()
}

/// Solve `a x = b` (rows of `a` are `n` long) by elimination with
/// pivoting. None if the system has no single answer.
pub(crate) fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<DVec3>) -> Option<Vec<DVec3>> {
    let n = b.len();
    for c in 0..n {
        let piv = (c..n).max_by(|x, y| a[*x][c].abs().total_cmp(&a[*y][c].abs()))?;
        if a[piv][c].abs() < 1e-12 {
            return None;
        }
        a.swap(c, piv);
        b.swap(c, piv);
        for r in c + 1..n {
            let f = a[r][c] / a[c][c];
            if f != 0.0 {
                let (top, rest) = a.split_at_mut(r);
                for (x, y) in rest[0][c..].iter_mut().zip(&top[c][c..]) {
                    *x -= f * y;
                }
                b[r] = b[r] - b[c] * f;
            }
        }
    }
    for c in (0..n).rev() {
        let mut s = b[c];
        for k in c + 1..n {
            s -= b[k] * a[c][k];
        }
        b[c] = s / a[c][c];
    }
    Some(b)
}

/// A NURBS curve.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Nurbs {
    degree: usize,
    knots: Vec<f64>,
    points: Vec<DVec3>,
    weights: Vec<f64>,
    /// Closed and smooth at the seam: the last `degree` control points
    /// repeat the first.
    #[serde(default)]
    periodic: bool,
}

impl Nurbs {
    /// A checked curve. None if the parts do not fit together.
    pub fn new(
        degree: usize,
        knots: Vec<f64>,
        points: Vec<DVec3>,
        weights: Vec<f64>,
        periodic: bool,
    ) -> Option<Nurbs> {
        let n = points.len();
        let ok = knots_ok(degree, &knots, n)
            && weights.len() == n
            && points.iter().all(|p| p.is_finite())
            && weights.iter().all(|w| w.is_finite() && *w > 0.0)
            && (!periodic
                || n > 2 * degree
                    && (0..degree).all(|i| {
                        points[i] == points[n - degree + i] && weights[i] == weights[n - degree + i]
                    }));
        ok.then_some(Nurbs {
            degree,
            knots,
            points,
            weights,
            periodic,
        })
    }

    /// An open curve shaped by `points` (its control points): it starts at
    /// the first and ends at the last. The degree is lowered for few points.
    pub fn open(points: &[DVec3], degree: usize) -> Option<Nurbs> {
        let n = points.len();
        if n < 2 {
            return None;
        }
        let p = degree.clamp(1, MAX_DEGREE).min(n - 1);
        Nurbs::new(p, clamped_knots(p, n), points.to_vec(), vec![1.0; n], false)
    }

    /// A closed smooth curve shaped by `points` (three or more).
    pub fn closed(points: &[DVec3], degree: usize) -> Option<Nurbs> {
        let n = points.len();
        if n < 3 {
            return None;
        }
        let p = degree.clamp(1, MAX_DEGREE).min(n - 1);
        let all: Vec<DVec3> = points.iter().chain(&points[..p]).copied().collect();
        let knots = (0..n + 2 * p + 1)
            .map(|i| (i as f64 - p as f64) / n as f64)
            .collect();
        Nurbs::new(p, knots, all, vec![1.0; n + p], true)
    }

    /// A cubic curve through `points`, open or closed.
    pub fn through(points: &[DVec3], closed: bool) -> Option<Nurbs> {
        let n = points.len();
        if closed {
            let mut c = Nurbs::closed(points, 3)?;
            let p = c.degree;
            let rows: Vec<Vec<f64>> = (0..n)
                .map(|i| {
                    let t = i as f64 / n as f64;
                    let k = span(p, &c.knots, n + p, t);
                    let b = basis(p, &c.knots, k, t);
                    let mut row = vec![0.0; n];
                    for (r, v) in b.iter().enumerate().take(p + 1) {
                        row[(k - p + r) % n] += v;
                    }
                    row
                })
                .collect();
            let q = solve(rows, points.to_vec())?;
            for (i, x) in q.iter().enumerate() {
                c.set_control(i, *x);
            }
            return Some(c);
        }
        if n < 3 {
            return Nurbs::open(points, 1);
        }
        let p = 3.min(n - 1);
        // Chord-length parameters, and knots from their averages.
        let total: f64 = points.windows(2).map(|w| w[0].distance(w[1])).sum();
        if total <= 0.0 {
            return None;
        }
        let mut ts = vec![0.0];
        for w in points.windows(2) {
            let last = ts[ts.len() - 1];
            ts.push(last + w[0].distance(w[1]) / total);
        }
        ts[n - 1] = 1.0;
        let mut knots = vec![0.0; n + p + 1];
        for j in 1..n - p {
            knots[j + p] = ts[j..j + p].iter().sum::<f64>() / p as f64;
        }
        for k in knots.iter_mut().skip(n) {
            *k = 1.0;
        }
        if !knots_ok(p, &knots, n) {
            return None;
        }
        let rows: Vec<Vec<f64>> = ts
            .iter()
            .map(|&t| {
                let k = span(p, &knots, n, t);
                let b = basis(p, &knots, k, t);
                let mut row = vec![0.0; n];
                row[k - p..=k].copy_from_slice(&b[..=p]);
                row
            })
            .collect();
        let q = solve(rows, points.to_vec())?;
        Nurbs::new(p, knots, q, vec![1.0; n], false)
    }

    pub fn degree(&self) -> usize {
        self.degree
    }

    pub fn knots(&self) -> &[f64] {
        &self.knots
    }

    /// Every control point, a periodic curve's repeats included.
    pub fn points(&self) -> &[DVec3] {
        &self.points
    }

    pub fn weights(&self) -> &[f64] {
        &self.weights
    }

    pub fn is_periodic(&self) -> bool {
        self.periodic
    }

    /// The control points that can be moved on their own (a periodic
    /// curve's repeats left out).
    pub fn cage(&self) -> &[DVec3] {
        let n = self.points.len();
        &self.points[..if self.periodic { n - self.degree } else { n }]
    }

    /// Move control point `i` of the cage (and its repeat).
    pub fn set_control(&mut self, i: usize, p: DVec3) {
        let n = self.cage().len();
        if i >= n {
            return;
        }
        self.points[i] = p;
        if self.periodic && i < self.degree {
            self.points[n + i] = p;
        }
    }

    /// The first and last parameter.
    pub fn domain(&self) -> (f64, f64) {
        (self.knots[self.degree], self.knots[self.points.len()])
    }

    /// Whether the curve ends where it starts.
    pub fn is_closed(&self) -> bool {
        let n = self.points.len();
        self.periodic
            || (self.knots[0] == self.knots[self.degree]
                && self.knots[n] == self.knots[n + self.degree]
                && self.points[0] == self.points[n - 1])
    }

    /// `t` brought into the domain: wrapped round a closed curve, held at
    /// the ends of an open one.
    pub fn wrap(&self, t: f64) -> f64 {
        let (lo, hi) = self.domain();
        if (lo..=hi).contains(&t) {
            t
        } else if self.is_closed() {
            lo + (t - lo).rem_euclid(hi - lo)
        } else {
            t.clamp(lo, hi)
        }
    }

    fn weighted(&self, t: f64) -> (H, H) {
        let t = self.wrap(t);
        let k = span(self.degree, &self.knots, self.points.len(), t);
        de_boor(self.degree, &self.knots, k, t, |i| {
            H::of(self.points[i], self.weights[i])
        })
    }

    /// The point at `t`.
    pub fn eval(&self, t: f64) -> DVec3 {
        let (c, _) = self.weighted(t);
        c.p / c.w
    }

    /// The point and the first derivative at `t`.
    pub fn eval_d(&self, t: f64) -> (DVec3, DVec3) {
        let (c, d) = self.weighted(t);
        c.split(d)
    }

    /// The curvature vector at `t`: towards the centre of the bend, as long
    /// as one over the bend's radius.
    pub fn curvature(&self, t: f64) -> DVec3 {
        let (lo, hi) = self.domain();
        let h = (hi - lo) * 1e-4;
        let (t0, t1) = if self.is_closed() {
            (t - h, t + h)
        } else {
            ((t - h).max(lo), (t + h).min(hi))
        };
        let (p0, d0) = self.eval_d(t0);
        let (p1, d1) = self.eval_d(t1);
        let ds = p0.distance(p1);
        if ds <= 0.0 {
            return DVec3::ZERO;
        }
        (d1.normalize_or_zero() - d0.normalize_or_zero()) / ds
    }

    /// The curve after `m`. Any affine map keeps a NURBS curve exact.
    pub fn transformed(&self, m: &DAffine3) -> Nurbs {
        let mut c = self.clone();
        for p in &mut c.points {
            *p = m.transform_point3(*p);
        }
        c
    }

    /// The same curve with one more knot at `t` (so one more control
    /// point). None for a periodic curve or a `t` outside it.
    pub fn insert_knot(&self, t: f64) -> Option<Nurbs> {
        let (lo, hi) = self.domain();
        if self.periodic || t <= lo || t >= hi {
            return None;
        }
        let (p, n) = (self.degree, self.points.len());
        let k = span(p, &self.knots, n, t);
        let old = |i: usize| H::of(self.points[i], self.weights[i]);
        let mut q = Vec::with_capacity(n + 1);
        for i in 0..=n {
            q.push(if i + p <= k {
                old(i)
            } else if i <= k {
                let a = (t - self.knots[i]) / (self.knots[i + p] - self.knots[i]);
                old(i - 1).lerp(old(i), a)
            } else {
                old(i - 1)
            });
        }
        let mut knots = self.knots.clone();
        knots.insert(k + 1, t);
        Nurbs::new(
            p,
            knots,
            q.iter().map(|h| h.p / h.w).collect(),
            q.iter().map(|h| h.w).collect(),
            false,
        )
    }

    /// A straight line as a curve.
    pub fn line(a: DVec3, b: DVec3) -> Option<Nurbs> {
        Nurbs::open(&[a, b], 1)
    }

    /// An arc as an exact (rational) curve: from `start`, counter-clockwise
    /// about `axis` through `centre` by `sweep` radians (a full turn at
    /// most, which closes it).
    pub fn arc(centre: DVec3, axis: DVec3, start: DVec3, sweep: f64) -> Option<Nurbs> {
        let n = axis.try_normalize()?;
        let r = start - centre;
        let u = r - n * r.dot(n);
        let radius = u.length();
        let sweep = sweep.min(std::f64::consts::TAU);
        if radius <= 0.0 || sweep.is_nan() || sweep <= 0.0 {
            return None;
        }
        let (u, v) = (u / radius, n.cross(u / radius));
        let base = centre + n * r.dot(n);
        let pieces = (sweep / std::f64::consts::FRAC_PI_2 - 1e-9).ceil().max(1.0) as usize;
        let step = sweep / pieces as f64;
        let w = (step / 2.0).cos();
        let at = |t: f64, k: f64| base + (u * t.cos() + v * t.sin()) * radius * k;
        let mut points = vec![at(0.0, 1.0)];
        let mut weights = vec![1.0];
        let mut knots = vec![0.0; 3];
        for i in 0..pieces {
            let t = step * i as f64;
            points.push(at(t + step / 2.0, 1.0 / w));
            weights.push(w);
            points.push(at(t + step, 1.0));
            weights.push(1.0);
            let k = (i + 1) as f64 / pieces as f64;
            knots.extend([k, k]);
        }
        knots.push(1.0);
        if sweep >= std::f64::consts::TAU {
            // Closed exactly.
            let last = points.len() - 1;
            points[last] = points[0];
        }
        Nurbs::new(2, knots, points, weights, false)
    }

    /// The same curve walked the other way.
    pub fn reversed(&self) -> Nurbs {
        let (lo, hi) = (self.knots[0], self.knots[self.knots.len() - 1]);
        Nurbs {
            degree: self.degree,
            knots: self.knots.iter().rev().map(|k| lo + hi - k).collect(),
            points: self.points.iter().rev().copied().collect(),
            weights: self.weights.iter().rev().copied().collect(),
            periodic: self.periodic,
        }
    }

    /// The same curve with its parameter running from 0 to 1.
    pub fn normalized(&self) -> Nurbs {
        let (lo, hi) = self.domain();
        let mut c = self.clone();
        for k in &mut c.knots {
            *k = (*k - lo) / (hi - lo);
        }
        c
    }

    /// A closed smooth curve started `k` control points later (the same
    /// shape; its seam moves).
    pub fn started_at(&self, k: usize) -> Nurbs {
        if !self.periodic {
            return self.clone();
        }
        let n = self.cage().len();
        let turn =
            |v: &[f64]| -> Vec<f64> { (0..n + self.degree).map(|i| v[(i + k) % n]).collect() };
        let xs: Vec<f64> = self.weights[..n].to_vec();
        let pts: Vec<DVec3> = (0..n + self.degree)
            .map(|i| self.points[(i + k) % n])
            .collect();
        Nurbs {
            points: pts,
            weights: turn(&xs),
            ..self.clone()
        }
    }

    /// How many times `t` is a knot.
    fn multiplicity(&self, t: f64) -> usize {
        self.knots.iter().filter(|k| **k == t).count()
    }

    /// The distinct knots strictly inside the domain.
    fn breaks(&self) -> Vec<f64> {
        let (lo, hi) = self.domain();
        let mut out: Vec<f64> = self
            .knots
            .iter()
            .copied()
            .filter(|k| *k > lo && *k < hi)
            .collect();
        out.dedup();
        out
    }

    /// The same curve, one degree higher (exact). None for a periodic
    /// curve or one already at the highest degree.
    pub fn elevated(&self) -> Option<Nurbs> {
        if self.periodic || self.degree >= MAX_DEGREE {
            return None;
        }
        let p = self.degree;
        // Cut into Bézier pieces: every inside knot `p` times.
        let mut c = self.clone();
        for t in self.breaks() {
            for _ in c.multiplicity(t)..p {
                c = c.insert_knot(t)?;
            }
        }
        let breaks = c.breaks();
        let pieces = breaks.len() + 1;
        let h = |i: usize| H::of(c.points[i], c.weights[i]);
        let mut q: Vec<H> = vec![h(0)];
        for s in 0..pieces {
            let at = |i: usize| h(s * p + i);
            for i in 1..=p {
                let a = i as f64 / (p + 1) as f64;
                q.push(at(i).lerp(at(i - 1), a));
            }
            q.push(at(p));
        }
        let (lo, hi) = c.domain();
        let mut knots = vec![lo; p + 2];
        for t in breaks {
            knots.extend(std::iter::repeat_n(t, p + 1));
        }
        knots.extend(std::iter::repeat_n(hi, p + 2));
        Nurbs::new(
            p + 1,
            knots,
            q.iter().map(|x| x.p / x.w).collect(),
            q.iter().map(|x| x.w).collect(),
            false,
        )
    }

    /// The curves made alike, so a surface can run through them: the same
    /// degree, knots and number of control points, each still its own
    /// shape exactly, over 0 to 1. Open curves are raised to the highest
    /// degree and share every knot. Closed smooth curves must already
    /// match (the same degree and number of control points). None if they
    /// cannot be made alike.
    pub fn alike(curves: &[Nurbs]) -> Option<Vec<Nurbs>> {
        let first = curves.first()?;
        if curves.iter().any(|c| c.periodic) {
            let same = curves.iter().all(|c| {
                c.periodic && c.degree == first.degree && c.points.len() == first.points.len()
            });
            let out: Vec<Nurbs> = curves.iter().map(|c| c.normalized()).collect();
            let knots_match = out.iter().all(|c| {
                c.knots
                    .iter()
                    .zip(&out[0].knots)
                    .all(|(a, b)| (a - b).abs() < 1e-12)
            });
            return (same && knots_match).then(|| {
                let k = out[0].knots.clone();
                out.into_iter()
                    .map(|c| Nurbs {
                        knots: k.clone(),
                        ..c
                    })
                    .collect()
            });
        }
        let degree = curves.iter().map(|c| c.degree).max()?;
        let mut out = Vec::with_capacity(curves.len());
        for c in curves {
            let mut c = c.normalized();
            while c.degree < degree {
                c = c.elevated()?;
            }
            out.push(c);
        }
        // Every inside knot, as many times as the curve that has it most.
        // Knots a hair apart count as one.
        let mut all: Vec<(f64, usize)> = Vec::new();
        for c in &out {
            for t in c.breaks() {
                let m = c.multiplicity(t);
                match all.iter_mut().find(|x| (x.0 - t).abs() < 1e-9) {
                    Some(x) => x.1 = x.1.max(m),
                    None => all.push((t, m)),
                }
            }
        }
        all.sort_by(|a, b| a.0.total_cmp(&b.0));
        for c in &mut out {
            // Its own knots onto the shared values first.
            for k in &mut c.knots {
                if let Some(x) = all.iter().find(|x| (x.0 - *k).abs() < 1e-9) {
                    *k = x.0;
                }
            }
            for (t, m) in &all {
                for _ in c.multiplicity(*t)..*m {
                    *c = c.insert_knot(*t)?;
                }
            }
        }
        Some(out)
    }

    /// The part of an open curve from `t0` to `t1` (`t0` below `t1`, both
    /// in the domain) as a curve of its own, exactly. None for a periodic
    /// curve or an empty range.
    pub fn between(&self, t0: f64, t1: f64) -> Option<Nurbs> {
        let (lo, hi) = self.domain();
        if self.periodic || t0.is_nan() || t1.is_nan() || t0 < lo || t1 > hi || t1 <= t0 {
            return None;
        }
        let p = self.degree;
        let mut c = self.clone();
        for t in [t0, t1] {
            if t > lo && t < hi {
                for _ in c.multiplicity(t)..p {
                    c = c.insert_knot(t)?;
                }
            }
        }
        // With `p` knots at a value, the curve passes through one control
        // point there: at `t0` the one before the run's last `p` knots, at
        // `t1` the one before the run's first.
        let last0 = c.knots.iter().rposition(|k| *k == t0)?;
        let first1 = c.knots.iter().position(|k| *k == t1)?;
        let a = last0 + 1 - p;
        let (i0, i1) = (a - 1, first1 - 1);
        if i1 <= i0 {
            return None;
        }
        let mut knots = vec![t0; p + 1];
        knots.extend(c.knots[last0 + 1..first1].iter().copied());
        knots.extend(std::iter::repeat_n(t1, p + 1));
        Nurbs::new(
            p,
            knots,
            c.points[i0..=i1].to_vec(),
            c.weights[i0..=i1].to_vec(),
            false,
        )
    }

    /// The same open curve with weight 1 at both ends (so curves that
    /// share an end agree there in homogeneous coordinates, as a Coons
    /// patch needs). Exact: all weights are scaled, then the last span is
    /// made a Bézier piece and reparametrised.
    pub fn unit_ends(&self) -> Option<Nurbs> {
        if self.periodic {
            return None;
        }
        let mut c = self.clone();
        let w0 = c.weights[0];
        for w in &mut c.weights {
            *w /= w0;
        }
        let p = c.degree;
        let wl = c.weights[c.weights.len() - 1];
        if (wl - 1.0).abs() <= 1e-15 {
            return Some(c);
        }
        if let Some(t) = c.breaks().last().copied() {
            for _ in c.multiplicity(t)..p {
                c = c.insert_knot(t)?;
            }
        }
        let wl = c.weights[c.weights.len() - 1];
        let rho = (1.0 / wl).powf(1.0 / p as f64);
        let n = c.weights.len();
        for k in 0..=p {
            c.weights[n - 1 - p + k] *= rho.powi(k as i32);
        }
        c.weights[n - 1] = 1.0;
        Some(c)
    }

    /// The same open curve with weight `w` at its end, the rest of it
    /// untouched: the last span is made a Bézier piece of its own (split
    /// first if it is the only one) and reparametrised. Exact.
    pub fn reweighted_end(&self, w: f64) -> Option<Nurbs> {
        if self.periodic || w.is_nan() || w <= 0.0 {
            return None;
        }
        let mut c = self.clone();
        let p = c.degree;
        if c.breaks().is_empty() {
            let (lo, hi) = c.domain();
            c = c.insert_knot((lo + hi) / 2.0)?;
        }
        if let Some(t) = c.breaks().last().copied() {
            for _ in c.multiplicity(t)..p {
                c = c.insert_knot(t)?;
            }
        }
        let n = c.weights.len();
        let rho = (w / c.weights[n - 1]).powf(1.0 / p as f64);
        for k in 0..=p {
            c.weights[n - 1 - p + k] *= rho.powi(k as i32);
        }
        c.weights[n - 1] = w;
        Some(c)
    }

    /// The same curve with every weight times `k` (the same shape).
    pub fn scaled_weights(&self, k: f64) -> Nurbs {
        let mut c = self.clone();
        for w in &mut c.weights {
            *w *= k;
        }
        c
    }

    /// Open curves joined end to end (each starts where the one before
    /// ends) as one curve, exactly: raised to one degree, with a corner
    /// allowed at each join. Each takes a share of the parameter by the
    /// length of its control polygon. None if they do not meet end to end.
    pub fn joined(curves: &[Nurbs]) -> Option<Nurbs> {
        let first = curves.first()?;
        if curves.len() == 1 {
            return Some(first.normalized());
        }
        if curves.iter().any(|c| c.periodic) {
            return None;
        }
        let degree = curves.iter().map(|c| c.degree).max()?;
        let mut parts = Vec::with_capacity(curves.len());
        for c in curves {
            let mut c = c.normalized();
            while c.degree < degree {
                c = c.elevated()?;
            }
            parts.push(c);
        }
        let size = |c: &Nurbs| {
            c.points
                .windows(2)
                .map(|w| w[0].distance(w[1]))
                .sum::<f64>()
                .max(1e-300)
        };
        let total: f64 = parts.iter().map(size).sum();
        let scale = parts
            .iter()
            .map(|c| {
                c.points
                    .iter()
                    .fold(0.0f64, |m, p| m.max(p.abs().max_element()))
            })
            .fold(1.0f64, f64::max);
        let mut knots = vec![0.0; degree + 1];
        let mut points = parts[0].points.clone();
        let mut weights = parts[0].weights.clone();
        let mut at = 0.0;
        for (k, c) in parts.iter().enumerate() {
            let len = size(c) / total;
            if k > 0 {
                let (prev, start) = (points[points.len() - 1], c.points[0]);
                if prev.distance(start) > scale * 1e-7 {
                    return None;
                }
                // The same weight at the shared point (a rational curve's
                // shape does not change when all its weights are scaled).
                let f = weights[weights.len() - 1] / c.weights[0];
                points.extend(c.points[1..].iter().copied());
                weights.extend(c.weights[1..].iter().map(|w| w * f));
                knots.extend(std::iter::repeat_n(at, degree));
            }
            let n = c.knots.len();
            knots.extend(
                c.knots[degree + 1..n - degree - 1]
                    .iter()
                    .map(|t| at + t * len),
            );
            at += len;
        }
        knots.extend(std::iter::repeat_n(1.0, degree + 1));
        Nurbs::new(degree, knots, points, weights, false)
    }

    /// The number of knot spans of non-zero length in the domain.
    pub fn spans(&self) -> usize {
        let (p, n) = (self.degree, self.points.len());
        (p..n)
            .filter(|&k| self.knots[k] < self.knots[k + 1])
            .count()
    }

    /// The parameter in `lo..=hi` whose point is closest to `p`. The range
    /// may run past the end of a closed curve.
    pub fn closest(&self, p: DVec3, lo: f64, hi: f64) -> f64 {
        let (d0, d1) = self.domain();
        let n = ((self.spans() * 8) as f64 * ((hi - lo) / (d1 - d0)).abs().max(0.05)).ceil()
            as usize
            + 8;
        let at = |i: usize| lo + (hi - lo) * i as f64 / n as f64;
        let mut best = (f64::INFINITY, 0);
        for i in 0..=n {
            let d = self.eval(at(i)).distance_squared(p);
            if d < best.0 {
                best = (d, i);
            }
        }
        let (a, b) = (at(best.1.saturating_sub(1)), at((best.1 + 1).min(n)));
        let mut t = at(best.1);
        for _ in 0..24 {
            let (c, d) = self.eval_d(t);
            let dd = d.length_squared();
            if dd == 0.0 {
                break;
            }
            let step = (p - c).dot(d) / dd;
            let next = (t + step).clamp(a.min(b), a.max(b));
            if (next - t).abs() <= 1e-15 * (d1 - d0) {
                t = next;
                break;
            }
            t = next;
        }
        t
    }

    /// How far the direction of travel turns between `t0` and `t1`, in
    /// radians (measured at a few points per span).
    pub fn turning(&self, t0: f64, t1: f64) -> f64 {
        let (d0, d1) = self.domain();
        let n = (((t1 - t0).abs() / (d1 - d0) * self.spans() as f64).ceil() as usize).max(1)
            * (self.degree + 1);
        let mut last = self.eval_d(t0).1.normalize_or_zero();
        let mut sum = 0.0;
        for i in 1..=n {
            let d = self
                .eval_d(t0 + (t1 - t0) * i as f64 / n as f64)
                .1
                .normalize_or_zero();
            sum += last.dot(d).clamp(-1.0, 1.0).acos();
            last = d;
        }
        sum
    }

    /// How many straight steps draw the part from `t0` to `t1`: as fine as
    /// an arc that turns as much.
    pub fn steps(&self, t0: f64, t1: f64) -> usize {
        let (d0, d1) = self.domain();
        let spans = ((t1 - t0).abs() / (d1 - d0) * self.spans() as f64).ceil() as usize;
        if self.degree == 1 {
            return spans.max(1);
        }
        let by_turn = (self.turning(t0, t1) / std::f64::consts::TAU * SEGMENTS_PER_TURN as f64)
            .ceil() as usize;
        by_turn.clamp((spans * 2).max(2), 1024)
    }

    /// The plane the curve lies in, if it lies in one.
    pub fn plane(&self, eps: f64) -> Option<Plane> {
        let n = newell(&self.points).normalize_or_zero();
        if n == DVec3::ZERO {
            return None;
        }
        let pl = Plane::new(self.points[0], n);
        self.points
            .iter()
            .all(|p| pl.distance(*p).abs() <= eps)
            .then_some(pl)
    }
}

/// The part of a NURBS curve an edge runs along: from `t0` to `t1`, which
/// is below `t0` for an edge walked against the curve, and may pass the end
/// of a closed curve.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spline {
    pub curve: Arc<Nurbs>,
    pub t0: f64,
    pub t1: f64,
}

impl Spline {
    /// All of an open curve.
    pub fn whole(curve: Nurbs) -> Spline {
        let (t0, t1) = curve.domain();
        Spline {
            curve: Arc::new(curve),
            t0,
            t1,
        }
    }

    /// The parameter at fraction `s` of the way.
    pub fn at(&self, s: f64) -> f64 {
        self.t0 + (self.t1 - self.t0) * s
    }

    pub fn point(&self, s: f64) -> DVec3 {
        self.curve.eval(self.at(s))
    }

    /// The unit direction of travel at fraction `s`.
    pub fn tangent(&self, s: f64) -> DVec3 {
        let d = self.curve.eval_d(self.at(s)).1.normalize_or_zero();
        if self.t1 < self.t0 { -d } else { d }
    }

    pub fn reversed(&self) -> Spline {
        Spline {
            curve: self.curve.clone(),
            t0: self.t1,
            t1: self.t0,
        }
    }

    /// The fraction of the way whose point is closest to `p`.
    pub fn closest(&self, p: DVec3) -> f64 {
        let (lo, hi) = (self.t0.min(self.t1), self.t0.max(self.t1));
        let t = self.curve.closest(p, lo, hi);
        if self.t1 == self.t0 {
            0.0
        } else {
            (t - self.t0) / (self.t1 - self.t0)
        }
    }

    /// The two parts either side of the point closest to `p`.
    pub fn split(&self, p: DVec3) -> (Spline, Spline) {
        let t = self.at(self.closest(p));
        (
            Spline {
                curve: self.curve.clone(),
                t0: self.t0,
                t1: t,
            },
            Spline {
                curve: self.curve.clone(),
                t0: t,
                t1: self.t1,
            },
        )
    }

    /// The points strictly between the ends that it is drawn through.
    pub fn inner_points(&self) -> Vec<DVec3> {
        let n = self.curve.steps(self.t0, self.t1);
        (1..n).map(|i| self.point(i as f64 / n as f64)).collect()
    }

    pub fn length(&self) -> f64 {
        let n = self.curve.steps(self.t0, self.t1) * 4;
        let mut last = self.point(0.0);
        let mut sum = 0.0;
        for i in 1..=n {
            let p = self.point(i as f64 / n as f64);
            sum += last.distance(p);
            last = p;
        }
        sum
    }

    pub fn transformed(&self, m: &DAffine3) -> Spline {
        Spline {
            curve: Arc::new(self.curve.transformed(m)),
            t0: self.t0,
            t1: self.t1,
        }
    }

    /// Whether this is all of its curve (so its ends are the curve's ends).
    pub fn is_whole(&self) -> bool {
        let (lo, hi) = self.curve.domain();
        (self.t0 == lo && self.t1 == hi) || (self.t0 == hi && self.t1 == lo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quarter() -> Nurbs {
        let w = std::f64::consts::FRAC_1_SQRT_2;
        Nurbs::new(
            2,
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            vec![DVec3::X, DVec3::new(1.0, 1.0, 0.0), DVec3::Y],
            vec![1.0, w, 1.0],
            false,
        )
        .unwrap()
    }

    #[test]
    fn a_rational_quadratic_is_a_circle() {
        let c = quarter();
        for i in 0..=20 {
            let t = i as f64 / 20.0;
            let (p, d) = c.eval_d(t);
            assert!((p.length() - 1.0).abs() < 1e-14);
            assert!(
                p.dot(d).abs() < 1e-13,
                "the tangent is square to the radius"
            );
            if i > 0 && i < 20 {
                assert!((c.curvature(t).length() - 1.0).abs() < 1e-5);
            }
        }
        assert_eq!(c.eval(0.0), DVec3::X);
        assert_eq!(c.eval(1.0), DVec3::Y);
    }

    #[test]
    fn a_part_of_a_curve_is_the_same_shape_and_parts_join_back() {
        let pts = [
            DVec3::ZERO,
            DVec3::new(1.0, 2.0, 0.0),
            DVec3::new(3.0, -1.0, 1.0),
            DVec3::new(4.0, 0.5, 0.0),
            DVec3::new(6.0, 0.0, 2.0),
        ];
        let arc = Nurbs::arc(DVec3::ZERO, DVec3::Z, DVec3::X, 4.0).unwrap();
        for c in [Nurbs::open(&pts, 3).unwrap(), quarter(), arc] {
            let (lo, hi) = c.domain();
            let m = lo + (hi - lo) * 0.41;
            let a = c.between(lo, m).unwrap();
            let b = c.between(m, hi).unwrap();
            let mid = c
                .between(lo + (hi - lo) * 0.2, lo + (hi - lo) * 0.7)
                .unwrap();
            for i in 0..=40 {
                let s = i as f64 / 40.0;
                assert!(
                    a.eval(lo + (m - lo) * s)
                        .distance(c.eval(lo + (m - lo) * s))
                        < 1e-12
                );
                assert!(b.eval(m + (hi - m) * s).distance(c.eval(m + (hi - m) * s)) < 1e-12);
                let t = lo + (hi - lo) * (0.2 + 0.5 * s);
                assert!(mid.eval(t).distance(c.eval(t)) < 1e-12);
            }
            // Joined back: every point of the whole is on the join.
            let j = Nurbs::joined(&[a, b]).unwrap();
            assert!(j.eval(0.0).distance(c.eval(lo)) < 1e-12);
            assert!(j.eval(1.0).distance(c.eval(hi)) < 1e-12);
            for i in 0..=40 {
                let p = c.eval(lo + (hi - lo) * i as f64 / 40.0);
                let t = j.closest(p, 0.0, 1.0);
                assert!(j.eval(t).distance(p) < 1e-9);
            }
        }
        assert!(quarter().between(0.5, 0.5).is_none());
        // Weight 1 at both ends, the same shape.
        let half = quarter().between(0.0, 0.3).unwrap();
        assert!(half.weights()[half.weights().len() - 1] != 1.0);
        let u = half.unit_ends().unwrap();
        assert_eq!(
            (u.weights()[0], u.weights()[u.weights().len() - 1]),
            (1.0, 1.0)
        );
        let r = half.reweighted_end(1.0).unwrap();
        assert_eq!(r.weights()[r.weights().len() - 1], 1.0);
        for i in 0..=20 {
            let t = i as f64 / 20.0 * 0.3;
            let p = r.eval(t);
            assert!((p.length() - 1.0).abs() < 1e-14);
            if t < 0.15 {
                assert!(
                    p.distance(half.eval(t)) < 1e-14,
                    "the first span is untouched"
                );
            }
        }
        for i in 0..=20 {
            let p = u.eval(i as f64 / 20.0 * 0.3);
            assert!((p.length() - 1.0).abs() < 1e-14);
            assert!(half.eval(half.closest(p, 0.0, 0.3)).distance(p) < 1e-12);
        }
    }

    #[test]
    fn a_line_and_an_arc_join_with_a_corner() {
        let line = Nurbs::line(DVec3::new(-2.0, 1.0, 0.0), DVec3::Y).unwrap();
        let arc = Nurbs::arc(DVec3::ZERO, DVec3::Z, DVec3::Y, 1.0).unwrap();
        let j = Nurbs::joined(&[line.clone(), arc.clone()]).unwrap();
        assert_eq!(j.degree(), 2);
        assert!(j.eval(0.0).distance(line.eval(0.0)) < 1e-12);
        assert!(j.eval(1.0).distance(arc.eval(1.0)) < 1e-12);
        // Each part keeps its own parameter, scaled.
        let k = j.knots()[3];
        for i in 0..=20 {
            let s = i as f64 / 20.0;
            assert!(j.eval(k + (1.0 - k) * s).distance(arc.eval(s)) < 1e-12);
            assert!(j.eval(k * s).distance(line.eval(s)) < 1e-12);
        }
        assert!(Nurbs::joined(&[arc, line]).is_none(), "they do not meet");
    }

    #[test]
    fn inserting_a_knot_keeps_the_shape() {
        let pts = [
            DVec3::ZERO,
            DVec3::new(1.0, 2.0, 0.0),
            DVec3::new(3.0, -1.0, 1.0),
            DVec3::new(4.0, 0.5, 0.0),
            DVec3::new(6.0, 0.0, 2.0),
        ];
        for c in [Nurbs::open(&pts, 3).unwrap(), quarter()] {
            let d = c.insert_knot(0.37).unwrap();
            assert_eq!(d.points().len(), c.points().len() + 1);
            for i in 0..=50 {
                let t = i as f64 / 50.0;
                assert!(c.eval(t).distance(d.eval(t)) < 1e-13);
            }
        }
    }

    #[test]
    fn a_curve_through_points_passes_through_them() {
        let pts = [
            DVec3::ZERO,
            DVec3::new(1.0, 2.0, 0.0),
            DVec3::new(3.0, -1.0, 1.0),
            DVec3::new(4.0, 0.5, 0.0),
            DVec3::new(6.0, 0.0, 2.0),
        ];
        let c = Nurbs::through(&pts, false).unwrap();
        for p in pts {
            assert!(c.eval(c.closest(p, 0.0, 1.0)).distance(p) < 1e-10);
        }
        let c = Nurbs::through(&pts, true).unwrap();
        assert!(c.is_closed());
        for (i, p) in pts.iter().enumerate() {
            assert!(c.eval(i as f64 / 5.0).distance(*p) < 1e-10);
        }
    }

    #[test]
    fn a_closed_curve_is_smooth_at_the_seam() {
        let pts = [
            DVec3::ZERO,
            DVec3::new(2.0, 0.0, 0.0),
            DVec3::new(2.0, 2.0, 0.0),
            DVec3::new(0.0, 2.0, 0.0),
        ];
        let c = Nurbs::closed(&pts, 3).unwrap();
        let (a, da) = c.eval_d(0.0);
        let (b, db) = c.eval_d(1.0 - 1e-12);
        assert!(a.distance(b) < 1e-9);
        assert!(da.distance(db) < 1e-6);
        assert_eq!(c.cage().len(), 4);
        assert_eq!(c.wrap(1.25), 0.25);
        assert!(c.plane(1e-9).is_some());
    }

    #[test]
    fn the_closest_point_and_the_split() {
        let c = Nurbs::open(
            &[
                DVec3::ZERO,
                DVec3::new(1.0, 2.0, 0.0),
                DVec3::new(3.0, 2.0, 0.0),
                DVec3::new(4.0, 0.0, 0.0),
            ],
            3,
        )
        .unwrap();
        let s = Spline::whole(c);
        let mid = s.point(0.4);
        assert!((s.closest(mid) - 0.4).abs() < 1e-9);
        let (a, b) = s.split(mid);
        assert!(a.point(1.0).distance(mid) < 1e-12 && b.point(0.0).distance(mid) < 1e-12);
        assert!((a.length() + b.length() - s.length()).abs() < 1e-3);
        let r = s.reversed();
        assert!(r.point(0.0).distance(s.point(1.0)) < 1e-15);
        assert!(r.tangent(0.0).distance(-s.tangent(1.0)) < 1e-12);
    }
    #[test]
    fn an_arc_is_exact_and_a_full_turn_closes() {
        let c = Nurbs::arc(
            DVec3::new(1.0, 2.0, 3.0),
            DVec3::Z,
            DVec3::new(3.0, 2.0, 3.0),
            4.0,
        )
        .unwrap();
        for i in 0..=40 {
            let p = c.eval(i as f64 / 40.0);
            assert!((p.distance(DVec3::new(1.0, 2.0, 3.0)) - 2.0).abs() < 1e-13);
        }
        let end = DVec3::new(1.0 + 2.0 * 4f64.cos(), 2.0 + 2.0 * 4f64.sin(), 3.0);
        assert!(c.eval(1.0).distance(end) < 1e-13);
        let full = Nurbs::arc(DVec3::ZERO, DVec3::Z, DVec3::X, 7.0).unwrap();
        assert!(full.is_closed() && !full.is_periodic());
    }

    #[test]
    fn raising_the_degree_and_making_curves_alike_keep_their_shapes() {
        let line = Nurbs::line(DVec3::ZERO, DVec3::new(4.0, 0.0, 0.0)).unwrap();
        let arc = Nurbs::arc(
            DVec3::new(2.0, 0.0, 2.0),
            DVec3::Y,
            DVec3::new(0.0, 0.0, 2.0),
            3.0,
        )
        .unwrap();
        let wave = Nurbs::open(
            &[
                DVec3::new(0.0, 0.0, 4.0),
                DVec3::new(1.0, 2.0, 4.0),
                DVec3::new(2.0, -2.0, 4.0),
                DVec3::new(3.0, 1.0, 4.0),
                DVec3::new(4.0, 0.0, 4.0),
            ],
            3,
        )
        .unwrap();
        let up = arc.elevated().unwrap();
        assert_eq!(up.degree(), 3);
        for i in 0..=30 {
            let t = i as f64 / 30.0;
            assert!(up.eval(t).distance(arc.eval(t)) < 1e-13);
        }
        let all = [line.clone(), arc.clone(), wave.clone()];
        let alike = Nurbs::alike(&all).unwrap();
        for (a, b) in alike.iter().zip(&all) {
            assert_eq!(a.degree(), 3);
            assert_eq!(a.knots(), alike[0].knots());
            assert_eq!(a.points().len(), alike[0].points().len());
            for i in 0..=30 {
                let t = i as f64 / 30.0;
                assert!(a.eval(t).distance(b.eval(t)) < 1e-12);
            }
        }
        let back = wave.reversed();
        assert!(back.eval(0.25).distance(wave.eval(0.75)) < 1e-13);
        // Closed curves of different sizes cannot be made alike.
        let sq = |n: usize| {
            let pts: Vec<DVec3> = (0..n)
                .map(|i| {
                    let a = i as f64 / n as f64 * std::f64::consts::TAU;
                    DVec3::new(a.cos(), a.sin(), 0.0)
                })
                .collect();
            Nurbs::closed(&pts, 3).unwrap()
        };
        assert!(Nurbs::alike(&[sq(5), sq(6)]).is_none());
        assert!(Nurbs::alike(&[sq(5), sq(5).started_at(2)]).is_some());
        assert!(sq(5).started_at(2).eval(0.0).distance(sq(5).eval(0.4)) < 1e-12);
    }
}
